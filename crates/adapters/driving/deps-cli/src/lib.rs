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
mod pins;
mod refresh;
mod text;

use std::io::{self, Write};

use angzarr_presentation::{Catalog, Format};
use clap::ArgMatches;
use cli_support::{flag, one, say_notices};
use deps_application::{
    AppError, CreateRequest, Deps, DiscoverRequest, Edit, MoveRequest, Notice, Outcome,
    ProjectRefresh, PushRequest,
};
use serde::Serialize;

pub use commands::{bundle_verbs, is_bundle_verb, nouns};

/// The program's name, for the warnings.
const PROG: &str = "ctxloom";

pub use cli_support::{Call, Failure};

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

struct Context<'a> {
    deps: &'a Deps,
    refresh: &'a dyn ProjectRefresh,
    format: Format,
    catalog: &'a Catalog,
}

/// Run `noun` (given the matches of the whole line); `refresh` is the project
/// refresh a pull ends with.
pub fn run<G>(
    deps: &Deps,
    refresh: &dyn ProjectRefresh,
    noun: &str,
    call: &Call<'_, G>,
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
        refresh,
        format,
        catalog,
    };
    let (_, sub) = call
        .matches
        .subcommand()
        .filter(|(name, _)| *name == noun)
        .ok_or_else(|| AppError::DefaultNeedsName.into_failure())?;
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
    say_notices(&outcome.notices, PROG, cx.format, cx.catalog, err);
    outcome.result.map_err(IntoFailure::into_failure)
}

/// Write `value` as json, or as `text` says; a write that fails is a failure.
fn emit<T: Serialize>(
    format: Format,
    out: &mut dyn Write,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Failure> {
    cli_support::emit_or(format, out, value, text, |e| {
        AppError::File {
            what: "the answer".to_string(),
            detail: e.to_string(),
        }
        .into_failure()
    })
}

fn text_arg(matches: &ArgMatches, name: &str) -> String {
    one(matches, name).unwrap_or_default()
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
                name: one(args, "name"),
                url: one(args, "url"),
                forge: one(args, "forge"),
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
                return Err(AppError::DefaultNeedsName.into_failure());
            } else {
                Some(name.as_str())
            };
            let set = answer(cx, deps.remote_default(chosen), err)?;
            emit(cx.format, out, &set, |out| match chosen {
                Some(name) => writeln!(out, "Set default remote to: {name}"),
                None => writeln!(out, "Cleared default remote."),
            })?;
        }
        _ => discover(cx, args, out, err)?,
    }
    Ok(0)
}

/// `remote discover`: the progress line is the stream's, closed before a
/// warning or an error is said after it.
fn discover(
    cx: &Context<'_>,
    args: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Failure> {
    let query = args
        .get_many::<String>("query")
        .map(|words| words.cloned().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let request = DiscoverRequest {
        query: query.clone(),
        source: text_arg(args, "source"),
        limit: args.get_one::<usize>("limit").copied().unwrap_or(30),
        min_stars: args.get_one::<u64>("stars").copied().unwrap_or(0),
    };
    let _ = write!(err, "Searching repositories...");
    let outcome = cx.deps.remote_discover(&request);
    let _ = match &outcome.result {
        Ok(found) => writeln!(err, " found {}", found.count),
        Err(_) => writeln!(err),
    };
    let found = answer(cx, outcome, err)?;
    emit(cx.format, out, &found, |out| {
        text::discovered(out, &found, &query)
    })
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
            let yes = flag(args, "yes");
            let pulled = answer(
                cx,
                deps.pull_and_refresh(cx.refresh, flag(args, "force"), lock, yes),
                err,
            )?;
            let skipped = pulled
                .skipped
                .as_ref()
                .map(|refresh| refresh::view(refresh, cx.catalog));
            let shown = refresh::Pulled {
                report: &pulled.report,
                refresh_skipped: skipped,
            };
            emit(cx.format, out, &shown, |out| {
                text::pull(out, &pulled.report)?;
                shown.refresh_skipped.as_ref().map_or(Ok(()), |skipped| {
                    refresh::text(out, skipped, "ctxloom deps pull --yes")
                })
            })?;
            if let Some(failure) = pulled.report.failure() {
                return Err(failure.into_failure());
            }
            if let Some(failure) = pulled.failure {
                return Err(failure.into_failure());
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
        say_notices(&warnings, PROG, cx.format, cx.catalog, err);
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
