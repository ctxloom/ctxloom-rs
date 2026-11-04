//! The setup commands as a driving adapter (S9): arguments in, answers out, no
//! logic. `manage install|uninstall|check|hooks|statusline|gitignore`,
//! `materialize` and the hidden `util config-write`.
//!
//! The whole command line is parsed by the program's own command line (one
//! behaviour for help, usage errors and the exit ladder); this crate gives it
//! the setup [`nouns`] and, given the matches of one, [`run`]s it.

mod commands;
mod notices;
mod text;

use std::io::{self, Read, Write};

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{write_json, Catalog, Format};
use clap::ArgMatches;
use library_application::Flags;
use serde::Serialize;
use setup_application::{InstallRequest, MaterializeRequest, Setup, SetupError, SurfaceSpec};

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

impl From<SetupError> for Failure {
    fn from(error: SetupError) -> Failure {
        let exit = error.exit_code();
        Failure {
            cause: Box::new(error),
            exit,
        }
    }
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
        flags: Flags {
            degraded: call.degraded,
            config_set: call.config_set.clone(),
            no_companions: call.no_companions,
        },
        format,
        catalog,
    };
    let (_, sub) = call
        .matches
        .subcommand()
        .filter(|(name, _)| *name == noun)
        .ok_or_else(|| {
            Failure::from(SetupError::Request {
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
    let written = match cx.format {
        Format::Json => write_json(out, value),
        Format::Text => text(out),
    };
    written.map_err(|e| {
        Failure::from(SetupError::Request {
            detail: format!("the answer could not be written: {e}"),
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

fn one(matches: &ArgMatches, name: &str) -> Option<String> {
    matches.try_get_one::<String>(name).ok().flatten().cloned()
}

fn many(matches: &ArgMatches, name: &str) -> Vec<String> {
    matches
        .try_get_many::<String>(name)
        .ok()
        .flatten()
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
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
            let plan = cx.setup.install_plan(one(args, "engine").as_deref())?;
            emit(cx, out, &plan, |w| text::plan(w, &plan))?;
        }
        "install" => {
            let done = cx.setup.install(&InstallRequest {
                engine: one(args, "engine"),
                no_pull: flag(args, "no-pull"),
                flags: cx.flags.clone(),
            })?;
            notices::say(&done.notices, PROG, cx.format, cx.catalog, err);
            emit(cx, out, &done.value, |w| text::install(w, &done.value))?;
        }
        "uninstall" => {
            let report = cx.setup.uninstall()?;
            emit(cx, out, &report, |w| text::uninstall(w, &report))?;
            if let Some(first) = report.errors.first() {
                return Err(Failure::from(SetupError::Request {
                    detail: first.clone(),
                }));
            }
        }
        "check" => check(cx, out, err)?,
        "hooks" => match args.subcommand() {
            Some(("list", list)) => {
                let done = cx.setup.hooks_list(
                    one(list, "event").as_deref(),
                    &many(list, "profile"),
                    &cx.flags,
                )?;
                notices::say(&done.notices, PROG, cx.format, cx.catalog, err);
                emit(cx, out, &done.value, |w| text::hooks(w, &done.value))?;
            }
            _ => check(cx, out, err)?,
        },
        "statusline" => {
            let enabled = matches!(args.subcommand_name(), Some("install"));
            if args.subcommand_name().is_none() {
                return Ok(0);
            }
            let done = cx.setup.set_statusline(enabled, &cx.flags)?;
            notices::say(&done.notices, PROG, cx.format, cx.catalog, err);
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
                let report = cx.setup.gitignore_install()?;
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
    let done = cx.setup.check(&cx.flags)?;
    notices::say(&done.notices, PROG, cx.format, cx.catalog, err);
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
        .collect::<Result<Vec<_>, _>>()?;
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
    let mut done = cx.setup.materialize(&request)?;
    notices::say(&done.notices, PROG, cx.format, cx.catalog, err);
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
        Failure::from(SetupError::ConfigWrite {
            detail: format!("read patch from stdin: {e}"),
        })
    })?;
    let file = one(args, "file").unwrap_or_default();
    let done = cx
        .setup
        .config_write(&file, one(args, "filetype").as_deref(), &patch)?;
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
