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
mod notices;
mod text;

use std::io::{self, Write};

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{write_json, Catalog, Format};
use clap::ArgMatches;
use library_application::{
    AppError, AssembleRequest, ConfigShown, Flags, ItemKind, Library, Notice, RunRequest,
    SearchRequest,
};
use serde::Serialize;

pub use commands::nouns;

/// The program's name, for the warnings.
const PROG: &str = "ctxloom";

/// One command, as this crate is asked to run it.
pub struct Call<'a> {
    /// The matches of the whole command line.
    pub matches: &'a ArgMatches,
    /// The `--format` the line gave, if any.
    pub explicit_format: Option<Format>,
    /// Text on a terminal, json off one.
    pub default_format: Format,
    pub stdout_is_terminal: bool,
    pub degraded: bool,
    pub no_companions: bool,
    pub config_set: Vec<String>,
}

/// Why a command did not complete, and its exit code.
#[derive(Debug)]
pub struct Failure {
    pub cause: Box<dyn Diagnostic>,
    pub exit: u8,
}

impl From<AppError> for Failure {
    fn from(error: AppError) -> Failure {
        let exit = error.exit_code();
        Failure {
            cause: Box::new(error),
            exit,
        }
    }
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
    let flags = Flags {
        degraded: call.degraded,
        config_set: call.config_set.clone(),
        no_companions: call.no_companions,
    };
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
        .ok_or_else(|| Failure::from(AppError::SearchNothing))?;
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
        "config" => config(cx, verb(sub, "show").1, out, err),
        "bundle" => bundle(cx, verb(sub, "list"), out, err),
        "fragment" => items(cx, "fragment", verb(sub, "list"), out, err),
        "command" => items(cx, "command", verb(sub, "list"), out, err),
        "skill" => skill(cx, verb(sub, "list"), out, err),
        "profile" => profile(cx, verb(sub, "list"), out, err),
        "agent" => agent(cx, verb(sub, "list"), out, err),
        "llm" => llm(cx, out, err),
        "mcp" => mcp(cx, sub, out, err),
        "companion" => companion(cx, out, err),
        "search" => search(cx, sub, out, err),
        "context" => context(cx, verb(sub, "assemble").1, out, err),
        _ => run_command(cx, sub, out, err),
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
    notices::say(&outcome.notices, PROG, cx.format, cx.catalog, err);
    let value = outcome.result.map_err(Failure::from)?;
    let mut inert = Vec::new();
    let code = write(value, out, &mut inert).map_err(output_failure)?;
    let said: Vec<Notice> = inert.into_iter().map(Notice::text).collect();
    notices::say(&said, PROG, cx.format, cx.catalog, err);
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

/// Write `value` as json, or as `text` says.
fn emit<T: Serialize>(
    format: Format,
    out: &mut dyn Write,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<u8> {
    match format {
        Format::Json => write_json(out, value)?,
        Format::Text => text(out)?,
    }
    Ok(0)
}

fn flag(matches: &ArgMatches, name: &str) -> bool {
    matches
        .try_get_one::<bool>(name)
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}

fn text_arg(matches: &ArgMatches, name: &str) -> String {
    matches
        .try_get_one::<String>(name)
        .ok()
        .flatten()
        .cloned()
        .unwrap_or_default()
}

fn many_arg(matches: &ArgMatches, name: &str) -> Vec<String> {
    matches
        .try_get_many::<String>(name)
        .ok()
        .flatten()
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
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
                .into());
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
    let server = verb(sub, "server").1;
    let (name, args) = verb(server, "list");
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
        tags: many_arg(sub, "tag"),
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
        profiles: many_arg(sub, "profile"),
        fragments: many_arg(sub, "fragment"),
        tags: many_arg(sub, "tag"),
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
        return Err(AppError::RunNeedsDryRun.into());
    }
    let mut prompt = text_arg(sub, "prompt");
    if prompt.is_empty() {
        prompt = many_arg(sub, "prompt_words").join(" ");
    }
    let request = RunRequest {
        agent: text_arg(sub, "agent"),
        profile: text_arg(sub, "profile"),
        fragments: many_arg(sub, "fragment"),
        tags: many_arg(sub, "tag"),
        llm: text_arg(sub, "llm"),
        prompt,
        saved_command: text_arg(sub, "command"),
        workspace: text_arg(sub, "workspace"),
    };
    let outcome = cx.library.dry_run(&cx.flags, &request);
    notices::say(&outcome.notices, PROG, cx.format, cx.catalog, err);
    let previewed = outcome.result.map_err(Failure::from)?;
    emit(cx.format, out, &previewed.plan, |out| {
        text::dry_run(out, &previewed.plan)
    })
    .map_err(output_failure)?;
    if previewed.fatal.is_empty() {
        return Ok(0);
    }
    Err(AppError::aborted(&previewed.fatal).into())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
