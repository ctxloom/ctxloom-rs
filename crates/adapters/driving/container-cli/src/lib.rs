//! The `container` commands as a driving adapter (S11): arguments in,
//! answers out, no logic.
//!
//! ```text
//! ctxloom container                    (bare: help, which is no payload)
//! ctxloom container check [backend]    diagnostic only
//! ctxloom container build [backend] [--runtime ...]
//! ctxloom container prune [--yes] [--runtime ...] [--min-age ...]
//! ctxloom container scaffold
//! ctxloom container tooling [list]     (bare: lists)
//! ```
//!
//! The whole command line is parsed by the program's own; this crate gives it
//! the `container` noun and, given the matches of one, runs it.

mod commands;
mod fault;
mod text;

use std::io::Write;

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{write_json, Format};
use isolation_application::ContainerUse;

pub use commands::nouns;
#[cfg(any(test, feature = "test-support"))]
pub use fault::diagnostic_samples;
pub use fault::{CliFault, EN_US};

pub use cli_support::{Call, Failure};

fn failure(cause: impl Diagnostic + 'static, exit: u8) -> Failure {
    Failure::new(cause, exit)
}

fn app(error: isolation_application::IsolationError) -> Failure {
    let exit = error.exit_code();
    failure(error, exit)
}

fn output(error: std::io::Error) -> Failure {
    failure(
        CliFault::Output {
            ext_err_txt: error.to_string(),
        },
        1,
    )
}

/// Run `container` (given the matches of the whole line).
pub fn run<G>(
    uses: &dyn ContainerUse,
    call: &Call<'_, G>,
    out: &mut dyn Write,
) -> Result<u8, Failure> {
    let format = call.explicit_format.unwrap_or(call.default_format);
    let sub = call
        .matches
        .subcommand_matches("container")
        .ok_or_else(|| {
            failure(
                CliFault::NoSuchCommand {
                    noun: "container".to_string(),
                },
                2,
            )
        })?;
    match sub.subcommand() {
        None => return bare(call, out),
        Some(("check", m)) => {
            let report = uses
                .check(m.get_one::<String>("backend").map(String::as_str))
                .map_err(app)?;
            match format {
                Format::Json => write_json(out, &report),
                Format::Text => text::check(&report, out),
            }
            .map_err(output)?;
        }
        Some(("build", m)) => {
            uses.build(
                m.get_one::<String>("backend").map(String::as_str),
                m.get_one::<String>("runtime").map(String::as_str),
            )
            .map_err(app)?;
        }
        Some(("prune", m)) => {
            return Err(app(uses.prune(
                m.get_one::<String>("runtime").map(String::as_str),
                m.get_flag("yes"),
            )));
        }
        Some(("scaffold", _)) => {
            let report = uses.scaffold().map_err(app)?;
            writeln!(out, "Devcontainer: {}", report.dir)
                .and_then(|()| {
                    writeln!(
                        out,
                        "Edit its Dockerfile (the engine's agent stage layers on top), then run `ctxloom container build`."
                    )
                })
                .map_err(output)?;
        }
        // `tooling` and `tooling list` are one command: the bare form lists.
        Some(("tooling", _)) => {
            let report = uses.tooling().map_err(app)?;
            match format {
                Format::Json => write_json(out, &report),
                Format::Text => text::tooling(&report, out),
            }
            .map_err(output)?;
        }
        Some((other, _)) => {
            return Err(failure(
                CliFault::NoSuchCommand {
                    noun: other.to_string(),
                },
                2,
            ))
        }
    }
    Ok(0)
}

/// The bare noun prints its help, which is prose: no encoding carries it, so
/// a machine-readable format aimed at it is refused rather than answered with
/// text and a 0.
fn bare<G>(call: &Call<'_, G>, out: &mut dyn Write) -> Result<u8, Failure> {
    if call.explicit_format == Some(Format::Json) {
        return Err(failure(
            CliFault::HelpNotPayload {
                format: "json".to_string(),
            },
            2,
        ));
    }
    let mut container = commands::nouns()
        .remove(0)
        .bin_name("ctxloom container")
        .disable_help_subcommand(true);
    write!(out, "{}", container.render_help()).map_err(output)?;
    Ok(0)
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "fault.test.rs"]
mod fault_tests;
