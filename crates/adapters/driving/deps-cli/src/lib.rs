//! The remote and dependency commands as a driving adapter (S3): arguments
//! in, answers out, no logic. `remote list|show|create|edit|remove|default|
//! discover`, `deps list|pull|check|upgrade|hold|unhold` and the `bundle`
//! verbs `push` and `move`.
//!
//! The whole command line is parsed by the program's own command line (one
//! behaviour for help, usage errors and the exit ladder); this crate gives it
//! its [`nouns`] and [`bundle_verbs`] and, given the matches of one, [`run`]s
//! it: the arguments become a request of the application, and its answer is
//! written as json or as text, with what it noticed said on the error stream.

mod commands;
mod notices;
mod text;

use std::io::{self, Write};

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{write_json, Catalog, Format};
use clap::ArgMatches;
use deps::registry::Edit;
use deps_application::{AppError, CreateRequest, Deps, MoveRequest, Notice, Outcome, PushRequest};
use serde::Serialize;

pub use commands::{bundle_verbs, is_bundle_verb, nouns};

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

struct Context<'a> {
    deps: &'a Deps,
    format: Format,
    catalog: &'a Catalog,
}

/// Run `noun` (given the matches of the whole line).
pub fn run(
    deps: &Deps,
    noun: &str,
    call: &Call<'_>,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let format = call.explicit_format.unwrap_or(if call.stdout_is_terminal {
        Format::Text
    } else {
        call.default_format
    });
    let cx = Context {
        deps,
        format,
        catalog,
    };
    let (_, sub) = call
        .matches
        .subcommand()
        .filter(|(name, _)| *name == noun)
        .ok_or_else(|| Failure::from(AppError::DefaultNeedsName))?;
    let (verb, args) = match sub.subcommand() {
        Some((name, matches)) => (name, matches),
        None => ("", sub),
    };
    match noun {
        "remote" => remote(&cx, verb, args, out, err),
        "deps" => dependencies(&cx, verb, args, out, err),
        _ => bundle(&cx, verb, args, out, err),
    }
}

/// The notices, said; then the answer's error or the answer.
fn answer<T>(cx: &Context<'_>, outcome: Outcome<T>, err: &mut dyn Write) -> Result<T, Failure> {
    notices::say(&outcome.notices, PROG, cx.format, cx.catalog, err);
    outcome.result.map_err(Failure::from)
}

/// Write `value` as json, or as `text` says.
fn emit<T: Serialize>(
    format: Format,
    out: &mut dyn Write,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Failure> {
    let written = match format {
        Format::Json => write_json(out, value),
        Format::Text => text(out),
    };
    written.map_err(|e| {
        Failure::from(AppError::File {
            what: "the answer".to_string(),
            detail: e.to_string(),
        })
    })
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

/// A flag given with a value, even an empty one; none where it was not given.
fn given(matches: &ArgMatches, name: &str) -> Option<String> {
    matches.try_get_one::<String>(name).ok().flatten().cloned()
}

fn remote(
    cx: &Context<'_>,
    verb: &str,
    args: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let deps = cx.deps;
    match verb {
        "" | "list" => {
            let listing = answer(cx, deps.remote_list(), err)?;
            emit(cx.format, out, &listing, |out| {
                text::remote_list(out, &listing)
            })?;
        }
        "show" => {
            let browsed = answer(cx, deps.remote_show(&text_arg(args, "remote")), err)?;
            emit(cx.format, out, &browsed, |out| {
                text::remote_show(out, &browsed)
            })?;
        }
        "create" => {
            let created = answer(
                cx,
                deps.remote_create(&CreateRequest {
                    name: text_arg(args, "name"),
                    url: text_arg(args, "url"),
                    forge: text_arg(args, "forge"),
                }),
                err,
            )?;
            emit(cx.format, out, &created, |out| {
                writeln!(
                    out,
                    "Added remote '{}' \u{2192} {}",
                    created.name, created.url
                )
            })?;
        }
        "edit" => {
            let edit = Edit {
                name: given(args, "name"),
                url: given(args, "url"),
                forge: given(args, "forge"),
            };
            let edited = answer(cx, deps.remote_edit(&text_arg(args, "remote"), &edit), err)?;
            emit(cx.format, out, &edited, |out| {
                text::remote_edit(out, &edited)
            })?;
        }
        "remove" => {
            let removed = answer(
                cx,
                deps.remote_remove(&text_arg(args, "remote"), flag(args, "yes")),
                err,
            )?;
            emit(cx.format, out, &removed, |out| {
                text::remote_remove(out, &removed)
            })?;
        }
        "default" => {
            let name = text_arg(args, "remote");
            let chosen = if flag(args, "clear") {
                None
            } else if name.is_empty() {
                return Err(AppError::DefaultNeedsName.into());
            } else {
                Some(name.as_str())
            };
            let set = answer(cx, deps.remote_default(chosen), err)?;
            emit(cx.format, out, &set, |out| match chosen {
                Some(name) => writeln!(out, "Set default remote to: {name}"),
                None => writeln!(out, "Cleared default remote."),
            })?;
        }
        _ => {
            answer(cx, deps.remote_discover(), err)?;
        }
    }
    Ok(0)
}

fn dependencies(
    cx: &Context<'_>,
    verb: &str,
    args: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let deps = cx.deps;
    match verb {
        "" | "list" => {
            let listing = answer(cx, deps.list(), err)?;
            emit(cx.format, out, &listing, |out| {
                text::deps_list(out, &listing)
            })?;
        }
        "pull" => {
            let _ = writeln!(err, "Pulling dependencies...");
            let lock = args.get_one::<bool>("lock").copied().unwrap_or(true);
            let report = answer(cx, deps.pull(flag(args, "force"), lock), err)?;
            emit(cx.format, out, &report, |out| text::pull(out, &report))?;
            if let Some(failure) = report.failure() {
                return Err(failure.into());
            }
        }
        "check" => return check(cx, args, out, err),
        "upgrade" => {
            let _ = writeln!(err, "Resolving latest commits for pinned dependencies...");
            let report = answer(cx, deps.upgrade(flag(args, "yes")), err)?;
            emit(cx.format, out, &report, |out| text::upgrade(out, &report))?;
        }
        verb => {
            let held = verb == "hold";
            let result = answer(cx, deps.hold(&text_arg(args, "name"), held), err)?;
            emit(cx.format, out, &result, |out| text::hold(out, &result))?;
        }
    }
    Ok(0)
}

fn check(
    cx: &Context<'_>,
    args: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let reference = text_arg(args, "reference");
    if !reference.is_empty() {
        let checked = answer(cx, cx.deps.check_reference(&reference), err)?;
        emit(cx.format, out, &checked, |out| {
            text::check_reference(out, &checked)
        })?;
        return Ok(0);
    }
    let report = answer(cx, cx.deps.check(), err)?;
    if report.entries > 0 {
        let _ = writeln!(err, "Checking {} items for updates...", report.entries);
        let warnings: Vec<Notice> = report
            .refresh_failures
            .iter()
            .map(|f| Notice::text(format!("fetch {}: {}", f.url, f.error)))
            .chain(report.unchecked.iter().filter_map(unchecked_warning))
            .collect();
        notices::say(&warnings, PROG, cx.format, cx.catalog, err);
    }
    emit(cx.format, out, &report, |out| text::check(out, &report))?;
    Ok(0)
}

/// The warning an entry that failed to be checked earns (a fact about the
/// entry is in the report instead).
fn unchecked_warning(row: &deps_application::UncheckedRow) -> Option<Notice> {
    let text = match row.reason {
        "unparseable" => format!(
            "{}: could not parse reference ({}); skipping the update check for it",
            row.reference, row.error
        ),
        "unresolvable" => format!(
            "{}: could not resolve {:?} ({}); skipping the update check for it",
            row.reference, row.constraint, row.error
        ),
        _ => return None,
    };
    Some(Notice::text(text))
}

fn bundle(
    cx: &Context<'_>,
    verb: &str,
    args: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    if verb == "push" {
        let pushed = answer(
            cx,
            cx.deps.push(&PushRequest {
                bundle: text_arg(args, "bundle"),
                remote: text_arg(args, "remote"),
                message: text_arg(args, "message"),
                pr: flag(args, "pr"),
            }),
            err,
        )?;
        emit(cx.format, out, &pushed, |out| text::pushed(out, &pushed))?;
    } else {
        let moved = answer(
            cx,
            cx.deps.move_bundle(&MoveRequest {
                name: text_arg(args, "name"),
                to: text_arg(args, "to"),
                force: flag(args, "force"),
                message: text_arg(args, "message"),
            }),
            err,
        )?;
        emit(cx.format, out, &moved, |out| text::moved(out, &moved))?;
    }
    Ok(0)
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
