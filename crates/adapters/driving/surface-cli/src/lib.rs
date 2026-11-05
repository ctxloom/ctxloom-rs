//! The setup commands as a driving adapter (S9): arguments in, answers out, no
//! logic. `manage install|uninstall|check|hooks|statusline|gitignore`,
//! `materialize` and the hidden `util config-write`.
//!
//! The whole command line is parsed by the program's own command line (one
//! behaviour for help, usage errors and the exit ladder); this crate gives it
//! the setup [`nouns`] and, given the matches of one, [`run`]s it.

mod commands;
mod text;

use std::io::{self, Read, Write};

use angzarr_presentation::{Catalog, Format};
use clap::ArgMatches;
use cli_support::{flag, many, one, say_notices};
use library_application::Flags;
use serde::Serialize;
use setup_application::{InstallRequest, MaterializeRequest, Setup, SetupError, SurfaceSpec};

pub use commands::nouns;

/// The program's name, for the warnings.
const PROG: &str = "ctxloom";

pub use cli_support::Failure;

/// One command, with the global flags the library's use cases take.
pub type Call<'a> = cli_support::Call<'a, Flags>;

/// A setup error as the failure it ends the command in.
trait IntoFailure {
    fn into_failure(self) -> Failure;
}

impl IntoFailure for SetupError {
    fn into_failure(self) -> Failure {
        let exit = self.exit_code();
        Failure::new(self, exit)
    }
}

fn failed(error: SetupError) -> Failure {
    error.into_failure()
}

struct Context<'a> {
    setup: &'a Setup,
    flags: Flags,
    format: Format,
    catalog: &'a Catalog,
}

/// Run `noun` (given the matches of the whole line).
pub fn run(
    setup: &Setup,
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
        setup,
        flags: call.globals.clone(),
        format,
        catalog,
    };
    let (_, sub) = call
        .matches
        .subcommand()
        .filter(|(name, _)| *name == noun)
        .ok_or_else(|| {
            failed(SetupError::Request {
                detail: format!("no {noun} command"),
            })
        })?;
    match noun {
        "manage" => manage(&cx, sub, out, err),
        "materialize" => materialize(&cx, sub, out, err),
        _ => util(&cx, sub, &mut io::stdin(), out),
    }
}

fn emit<T: Serialize>(
    cx: &Context<'_>,
    out: &mut dyn Write,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> Result<(), Failure> {
    cli_support::emit_or(cx.format, out, value, text, |e| {
        failed(SetupError::Request {
            detail: format!("the answer could not be written: {e}"),
        })
    })
}

fn manage(
    cx: &Context<'_>,
    sub: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let Some((verb, args)) = sub.subcommand() else {
        return Ok(0);
    };
    match verb {
        "install" if flag(args, "print") => {
            let plan = cx
                .setup
                .install_plan(one(args, "engine").as_deref())
                .map_err(failed)?;
            emit(cx, out, &plan, |w| text::plan(w, &plan))?;
        }
        "install" => {
            let done = cx
                .setup
                .install(&InstallRequest {
                    engine: one(args, "engine"),
                    no_pull: flag(args, "no-pull"),
                    flags: cx.flags.clone(),
                })
                .map_err(failed)?;
            say_notices(&done.notices, PROG, cx.format, cx.catalog, err);
            emit(cx, out, &done.value, |w| text::install(w, &done.value))?;
        }
        "uninstall" => {
            let report = cx.setup.uninstall().map_err(failed)?;
            emit(cx, out, &report, |w| text::uninstall(w, &report))?;
            if let Some(first) = report.errors.first() {
                return Err(failed(SetupError::Request {
                    detail: first.clone(),
                }));
            }
        }
        "check" => check(cx, out, err)?,
        "hooks" => match args.subcommand() {
            Some(("list", list)) => {
                let done = cx
                    .setup
                    .hooks_list(
                        one(list, "event").as_deref(),
                        &many(list, "profile"),
                        &cx.flags,
                    )
                    .map_err(failed)?;
                say_notices(&done.notices, PROG, cx.format, cx.catalog, err);
                emit(cx, out, &done.value, |w| text::hooks(w, &done.value))?;
            }
            _ => check(cx, out, err)?,
        },
        "statusline" => {
            let enabled = matches!(args.subcommand_name(), Some("install"));
            if args.subcommand_name().is_none() {
                return Ok(0);
            }
            let done = cx
                .setup
                .set_statusline(enabled, &cx.flags)
                .map_err(failed)?;
            say_notices(&done.notices, PROG, cx.format, cx.catalog, err);
            let state = if done.value { "enabled" } else { "disabled" };
            let doc = StatuslineAnswer {
                status: state,
                statusline: done.value,
            };
            emit(cx, out, &doc, |w| {
                writeln!(w, "ctxloom HUD statusline: {state}")?;
                writeln!(
                    w,
                    "Run 'ctxloom run' or 'ctxloom materialize' to apply the change."
                )
            })?;
        }
        _ => {
            if args.subcommand_name() == Some("install") {
                let report = cx.setup.gitignore_install().map_err(failed)?;
                emit(cx, out, &report, |w| text::gitignore(w, &report))?;
            }
        }
    }
    Ok(0)
}

#[derive(Serialize)]
struct StatuslineAnswer {
    status: &'static str,
    statusline: bool,
}

fn check(cx: &Context<'_>, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), Failure> {
    let done = cx.setup.check(&cx.flags).map_err(failed)?;
    say_notices(&done.notices, PROG, cx.format, cx.catalog, err);
    emit(cx, out, &done.value, |w| text::check(w, &done.value))
}

fn materialize(
    cx: &Context<'_>,
    args: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Failure> {
    let surfaces = many(args, "surface")
        .iter()
        .map(|s| SurfaceSpec::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(failed)?;
    let request = MaterializeRequest {
        profiles: many(args, "profiles"),
        target: one(args, "target"),
        yes: flag(args, "yes"),
        engines: many(args, "backend"),
        surfaces,
        release: flag(args, "release"),
        dry_run: flag(args, "dry-run") || flag(args, "diff"),
        leave_launch_context: false,
        flags: cx.flags.clone(),
    };
    let mut done = cx.setup.materialize(&request).map_err(failed)?;
    say_notices(&done.notices, PROG, cx.format, cx.catalog, err);
    for failure in &done.value.failures {
        done.value.errors.push(cx.catalog.message(failure));
    }
    emit(cx, out, &done.value, |w| text::materialize(w, &done.value))?;
    // A delivery that was refused leaves the file as it was, and the command
    // failed: a script gating on the exit code must not believe it landed.
    match done.value.failures.into_iter().next() {
        Some(first) => Err(Failure {
            cause: Box::new(first),
            exit: 1,
        }),
        None => Ok(0),
    }
}

/// `util config-write`: the patch is read from `input` (stdin), and only when
/// the verb is there.
fn util(
    cx: &Context<'_>,
    sub: &ArgMatches,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<u8, Failure> {
    let Some(("config-write", args)) = sub.subcommand() else {
        return Ok(0);
    };
    let mut patch = String::new();
    input.read_to_string(&mut patch).map_err(|e| {
        failed(SetupError::ConfigWrite {
            detail: format!("read patch from stdin: {e}"),
        })
    })?;
    let file = one(args, "file").unwrap_or_default();
    let done = cx
        .setup
        .config_write(&file, one(args, "filetype").as_deref(), &patch)
        .map_err(failed)?;
    emit(cx, out, &done, |w| {
        writeln!(
            w,
            "Merged {} into {} ({})",
            done.merged_keys.join(", "),
            done.file,
            if done.created { "created" } else { "updated" }
        )
    })?;
    Ok(0)
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "world.test.rs"]
mod world;
