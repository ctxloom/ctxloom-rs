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
#[allow(dead_code)] // used by the green commit
mod text;

use std::io::Write;

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::Format;
use clap::ArgMatches;
use isolation_application::ContainerUse;

pub use commands::nouns;
#[cfg(any(test, feature = "test-support"))]
pub use fault::diagnostic_samples;
pub use fault::{CliFault, EN_US};

/// One command, as this crate is asked to run it.
pub struct Call<'a> {
    pub matches: &'a ArgMatches,
    pub explicit_format: Option<Format>,
    pub default_format: Format,
}

/// Why a command did not complete, and its exit code.
#[derive(Debug)]
pub struct Failure {
    pub cause: Box<dyn Diagnostic>,
    pub exit: u8,
}

#[allow(dead_code)] // used by the green commit
fn failure(cause: impl Diagnostic + 'static, exit: u8) -> Failure {
    Failure {
        cause: Box::new(cause),
        exit,
    }
}

#[allow(dead_code)] // used by the green commit
fn app(error: isolation_application::IsolationError) -> Failure {
    let exit = error.exit_code();
    failure(error, exit)
}

#[allow(dead_code)] // used by the green commit
fn output(error: std::io::Error) -> Failure {
    failure(
        CliFault::Output {
            ext_err_txt: error.to_string(),
        },
        1,
    )
}

/// Run `container` (given the matches of the whole line).
pub fn run(uses: &dyn ContainerUse, call: &Call<'_>, out: &mut dyn Write) -> Result<u8, Failure> {
    let _ = (uses, call, out);
    todo!("S11 green")
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "fault.test.rs"]
mod fault_tests;
