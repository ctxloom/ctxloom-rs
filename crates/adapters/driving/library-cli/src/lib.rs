//! The library commands as a driving adapter (S1): arguments in, answers out,
//! no logic. `config show`, `bundle list|show|view`, `fragment|command|skill
//! list|show`, `fragment premises`, `profile list|show`, `agent list|show`,
//! `llm list`, `mcp server list|show`, `companion list`, `search`, `context
//! assemble` and `run --dry-run`.
//!
//! The whole command line is parsed by the program's own command line (one
//! behaviour for help, usage errors and the exit ladder); this crate gives it
//! the library's [`nouns`] and, given the matches of one, [`run`]s it: the
//! arguments become a request of the application, and its answer is written
//! as json or as text, with what it noticed said on the error stream.

mod commands;
pub mod inert;
mod text;
mod write;

use std::io::{self, Write};

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{Catalog, Format};
use clap::ArgMatches;
use cli_support::{flag, many, one, say_notices};
use library_application::{
    AppError, AssembleRequest, ConfigShown, Flags, ItemKind, Library, Notice, RunRequest,
    SearchRequest,
};
use serde::Serialize;

pub use commands::nouns;

/// The program's name, for the warnings.
const PROG: &str = "ctxloom";

pub use cli_support::Failure;

/// One command, with the global flags the library's use cases take.
pub type Call<'a> = cli_support::Call<'a, Flags>;

/// An application error as the failure it ends the command in.
trait IntoFailure {
    fn into_failure(self) -> Failure;
}

impl IntoFailure for AppError {
    fn into_failure(self) -> Failure {
        let exit = self.exit_code();
        Failure::new(self, exit)
    }
}

fn failed(error: AppError) -> Failure {
    error.into_failure()
}

/// Everything a command needs to answer.
struct Context<'a> {
    library: &'a Library,
    flags: Flags,
    format: Format,
    explicit: bool,
    catalog: &'a Catalog,
}

/// Run `noun` (given the matches of the whole line).
pub fn run(
    library: &Library,
    noun: &str,
    call: &Call<'_>,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let flags = call.globals.clone();
    let format = call.explicit_format.unwrap_or_else(|| {
        if call.stdout_is_terminal {
            Format::Text
        } else {
            match library.preferred_format(&flags).as_deref() {
                Some("text") => Format::Text,
                _ => call.default_format,
            }
        }
    });
    let cx = Context {
        library,
        flags,
        format,
        explicit: call.explicit_format.is_some(),
        catalog,
    };
    let (_, sub) = call
        .matches
        .subcommand()
        .filter(|(name, _)| *name == noun)
        .ok_or_else(|| failed(AppError::SearchNothing))?;
    dispatch(&cx, noun, sub, out, err)
}

/// The verb of a noun's matches, or its default.
fn verb<'a>(sub: &'a ArgMatches, default: &'a str) -> (&'a str, &'a ArgMatches) {
    match sub.subcommand() {
        Some((name, matches)) => (name, matches),
        None => (default, sub),
    }
}

fn dispatch(
    cx: &Context<'_>,
    noun: &str,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    match noun {
        "config" => written(write::config(cx, verb(sub, "show"), out, err), || {
            config(cx, verb(sub, "show").1, out, err)
        }),
        "bundle" => written(write::bundle(cx, verb(sub, "list"), out, err), || {
            bundle(cx, verb(sub, "list"), out, err)
        }),
        "fragment" | "command" => {
            let kind = if noun == "fragment" {
                ItemKind::Fragment
            } else {
                ItemKind::Command
            };
            written(write::item(cx, kind, verb(sub, "list"), out, err), || {
                items(cx, noun, verb(sub, "list"), out, err)
            })
        }
        "skill" => written(write::skill(cx, verb(sub, "list"), out, err), || {
            skill(cx, verb(sub, "list"), out, err)
        }),
        "profile" => written(write::profile(cx, verb(sub, "list"), out, err), || {
            profile(cx, verb(sub, "list"), out, err)
        }),
        "agent" => written(write::agent(cx, verb(sub, "list"), out, err), || {
            agent(cx, verb(sub, "list"), out, err)
        }),
        "llm" => written(write::llm(cx, verb(sub, "list"), out, err), || {
            llm(cx, out, err)
        }),
        "mcp" => mcp(cx, sub, out, err),
        "companion" => written(write::companion(cx, verb(sub, "list"), out, err), || {
            companion(cx, out, err)
        }),
        "loadout" => write::loadout(sub, out),
        "search" => search(cx, sub, out, err),
        "context" => context(cx, verb(sub, "assemble").1, out, err),
        _ => run_command(cx, sub, out, err),
    }
}

/// A write verb's answer, or (it was not one) the read verb's.
fn written(
    write: Result<Option<u8>, Failure>,
    read: impl FnOnce() -> Result<u8, Failure>,
) -> Result<u8, Failure> {
    match write? {
        Some(code) => Ok(code),
        None => read(),
    }
}

/// The notices, said; then the answer's error or its output.
fn answer<T>(
    cx: &Context<'_>,
    outcome: library_application::Outcome<T>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    write: impl FnOnce(T, &mut dyn Write, &mut Vec<String>) -> io::Result<u8>,
) -> Result<u8, Failure> {
    say_notices(&outcome.notices, PROG, cx.format, cx.catalog, err);
    let value = outcome.result.map_err(failed)?;
    let mut inert = Vec::new();
    let code = write(value, out, &mut inert).map_err(output_failure)?;
    let said: Vec<Notice> = inert.into_iter().map(Notice::text).collect();
    say_notices(&said, PROG, cx.format, cx.catalog, err);
    Ok(code)
}

fn output_failure(error: io::Error) -> Failure {
    Failure {
        cause: Box::new(OutputFailure(error.to_string())),
        exit: 1,
    }
}

/// The answer could not be written.
#[derive(Debug)]
struct OutputFailure(String);

impl std::fmt::Display for OutputFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for OutputFailure {
    fn code(&self) -> &str {
        "ctxloom-cli-output"
    }

    fn args(&self) -> Vec<(&str, angzarr_diagnostic::Arg)> {
        vec![("ext_err_txt", self.0.as_str().into())]
    }
}

/// Write `value` as json, or as `text` says; the exit code is 0.
fn emit<T: Serialize>(
    format: Format,
    out: &mut dyn Write,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<u8> {
    cli_support::emit(format, out, value, text)?;
    Ok(0)
}

fn text_arg(matches: &ArgMatches, name: &str) -> String {
    one(matches, name).unwrap_or_default()
}

fn config(
    cx: &Context<'_>,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let section = text_arg(sub, "section");
    let outcome = cx.library.config_show(
        &cx.flags,
        (!section.is_empty()).then_some(section.as_str()),
        flag(sub, "raw"),
    );
    answer(cx, outcome, out, err, |shown: ConfigShown, out, _| {
        emit(cx.format, out, &shown.json, |out| {
            write!(out, "{}", shown.text)
        })
    })
}

fn bundle(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    match verb {
        "show" => {
            let outcome = cx.library.bundle_show(&cx.flags, &text_arg(sub, "name"));
            answer(cx, outcome, out, err, |show, out, notes| {
                emit(cx.format, out, &show, |out| {
                    let mut said = text::Notes(Vec::new());
                    let written = text::bundle_show(out, &mut said, &show);
                    notes.extend(said.0);
                    written
                })
            })
        }
        "view" => {
            let ask = text_arg(sub, "name");
            let outcome = cx
                .library
                .bundle_view(&cx.flags, &ask, flag(sub, "distilled"));
            answer(cx, outcome, out, err, |view, out, notes| {
                emit(cx.format, out, &view, |out| {
                    let mut said = text::Notes(Vec::new());
                    let written = text::bundle_view(out, &mut said, &view, &ask);
                    notes.extend(said.0);
                    written
                })
            })
        }
        _ => {
            let outcome = cx.library.bundle_list(&cx.flags);
            answer(cx, outcome, out, err, |list, out, _| {
                emit(cx.format, out, &list.rows, |out| {
                    text::bundle_list(out, &list)
                })
            })
        }
    }
}

fn items(
    cx: &Context<'_>,
    kind: &str,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let kind_of = if kind == "fragment" {
        ItemKind::Fragment
    } else {
        ItemKind::Command
    };
    match verb {
        "show" => {
            if cx.explicit && cx.format == Format::Json {
                return Err(AppError::UnsupportedFormat {
                    command: format!("{kind} show"),
                    format: "json".to_string(),
                }
                .into_failure());
            }
            let outcome = cx.library.item_show(
                &cx.flags,
                kind_of,
                &text_arg(sub, "ref"),
                flag(sub, "distilled"),
            );
            answer(cx, outcome, out, err, |show, out, notes| {
                let mut said = text::Notes(Vec::new());
                let written = text::item_show(out, &mut said, &show);
                notes.extend(said.0);
                written.map(|()| 0)
            })
        }
        "premises" => {
            let outcome = cx.library.premises(&cx.flags);
            answer(cx, outcome, out, err, |premises, out, _| {
                emit(cx.format, out, &premises, |out| {
                    text::premises(out, &premises)
                })
            })
        }
        _ => {
            let outcome = cx
                .library
                .item_list(&cx.flags, kind_of, &text_arg(sub, "bundle"));
            answer(cx, outcome, out, err, |list, out, _| {
                emit(cx.format, out, &list.rows, |out| {
                    text::item_list(out, &list)
                })
            })
        }
    }
}

fn skill(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    if verb == "show" {
        let outcome = cx.library.skill_show(&cx.flags, &text_arg(sub, "ref"));
        return answer(cx, outcome, out, err, |show, out, notes| {
            emit(cx.format, out, &show, |out| {
                let mut said = text::Notes(Vec::new());
                let written = text::skill_show(out, &mut said, &show);
                notes.extend(said.0);
                written
            })
        });
    }
    let outcome = cx.library.skill_list(&cx.flags, &text_arg(sub, "bundle"));
    answer(cx, outcome, out, err, |list, out, _| {
        emit(cx.format, out, &list.rows, |out| {
            text::skill_list(out, &list)
        })
    })
}

fn profile(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    if verb == "show" {
        let outcome = cx.library.profile_show(&cx.flags, &text_arg(sub, "name"));
        return answer(cx, outcome, out, err, |show, out, _| {
            emit(cx.format, out, &show, |out| text::profile_show(out, &show))
        });
    }
    let outcome = cx.library.profile_list(&cx.flags);
    answer(cx, outcome, out, err, |list, out, _| {
        emit(cx.format, out, &list.rows, |out| {
            text::profile_list(out, &list)
        })
    })
}

fn agent(
    cx: &Context<'_>,
    (verb, sub): (&str, &ArgMatches),
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    if verb == "show" {
        let catalog = cx.catalog;
        let say = |d: &dyn Diagnostic| catalog.message(d);
        let outcome = cx
            .library
            .agent_show(&cx.flags, &text_arg(sub, "name"), &say);
        return answer(cx, outcome, out, err, |show, out, _| {
            emit(cx.format, out, &show, |out| text::agent_show(out, &show))
        });
    }
    let outcome = cx.library.agent_list(&cx.flags);
    answer(cx, outcome, out, err, |rows, out, _| {
        emit(cx.format, out, &rows, |out| text::agent_list(out, &rows))
    })
}

fn llm(cx: &Context<'_>, out: &mut dyn Write, err: &mut dyn Write) -> Result<u8, Failure> {
    let outcome = cx.library.llm_list(&cx.flags);
    answer(cx, outcome, out, err, |rows, out, _| {
        emit(cx.format, out, &rows, |out| text::llm_list(out, &rows))
    })
}

fn companion(cx: &Context<'_>, out: &mut dyn Write, err: &mut dyn Write) -> Result<u8, Failure> {
    let outcome = cx.library.companion_list(&cx.flags);
    answer(cx, outcome, out, err, |rows, out, _| {
        emit(cx.format, out, &rows, |out| {
            text::companion_list(out, &rows)
        })
    })
}

fn mcp(
    cx: &Context<'_>,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let Some(("server", server)) = sub.subcommand() else {
        return Err(write::mcp_bare(cx));
    };
    let (name, args) = verb(server, "list");
    if let Some(code) = write::mcp(cx, (name, args), out, err)? {
        return Ok(code);
    }
    if name == "show" {
        let outcome = cx.library.mcp_show(&cx.flags, &text_arg(args, "name"));
        return answer(cx, outcome, out, err, |show, out, _| {
            emit(cx.format, out, &show, |out| text::mcp_show(out, &show))
        });
    }
    let outcome = cx.library.mcp_list(&cx.flags);
    answer(cx, outcome, out, err, |list, out, _| {
        emit(cx.format, out, &list, |out| text::mcp_list(out, &list))
    })
}

fn search(
    cx: &Context<'_>,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let request = SearchRequest {
        query: text_arg(sub, "query"),
        tags: many(sub, "tag"),
        kind: text_arg(sub, "type"),
        local: flag(sub, "local"),
        remote: flag(sub, "remote"),
    };
    let outcome = cx.library.search(&cx.flags, &request);
    answer(cx, outcome, out, err, |results, out, _| {
        emit(cx.format, out, &results, |out| text::search(out, &results))
    })
}

fn context(
    cx: &Context<'_>,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let request = AssembleRequest {
        profiles: many(sub, "profile"),
        fragments: many(sub, "fragment"),
        tags: many(sub, "tag"),
    };
    let outcome = cx.library.assemble(&cx.flags, &request);
    answer(cx, outcome, out, err, |assembled, out, _| {
        emit(cx.format, out, &assembled, |out| {
            writeln!(out, "{}", assembled.context)
        })
    })
}

fn run_command(
    cx: &Context<'_>,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    if !flag(sub, "dry-run") {
        return Err(AppError::RunNeedsDryRun.into_failure());
    }
    let mut prompt = text_arg(sub, "prompt");
    if prompt.is_empty() {
        prompt = many(sub, "prompt_words").join(" ");
    }
    let request = RunRequest {
        agent: text_arg(sub, "agent"),
        profile: text_arg(sub, "profile"),
        fragments: many(sub, "fragment"),
        tags: many(sub, "tag"),
        llm: text_arg(sub, "llm"),
        prompt,
        saved_command: text_arg(sub, "command"),
        workspace: text_arg(sub, "workspace"),
    };
    let outcome = cx.library.dry_run(&cx.flags, &request);
    say_notices(&outcome.notices, PROG, cx.format, cx.catalog, err);
    let previewed = outcome.result.map_err(failed)?;
    emit(cx.format, out, &previewed.plan, |out| {
        text::dry_run(out, &previewed.plan)
    })
    .map_err(output_failure)?;
    if previewed.fatal.is_empty() {
        return Ok(0);
    }
    Err(AppError::aborted(&previewed.fatal).into_failure())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
