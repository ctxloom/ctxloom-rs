//! Why a run launched nothing, what it says when the person cancels; and
//! this crate's catalog.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

#[derive(Clone, PartialEq, Debug)]
pub enum RunError {
    /// The use case failed: its error's diagnostic.
    App(AppError),
    /// No task has the harp id given.
    NotFound { harp_id: String },
    /// No harp id, and no terminal to pick one on.
    NotInteractive,
    /// Reading the person's choice failed (not a quit: an I/O fault).
    ReadSelection { detail: String },
    /// `ctxloom` is not on PATH.
    NoCtxloom,
    /// `ctxloom run` ran and failed: its own output says why.
    Exited { status: i32 },
    /// `ctxloom run` could not be started, or ended with no status (a
    /// signal).
    Launch { detail: String },
}

/// What a run says on stderr besides an error.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunNotice {
    /// The person quit the picker: nothing launched, and that is no error.
    Cancelled,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::App(error) => error.fmt(f),
            RunError::NotFound { harp_id } => write!(f, "task not found: {harp_id}"),
            RunError::NotInteractive => f.write_str(
                "no task harp id given and not a terminal; pass a task harp id (see `taskloom list`)",
            ),
            RunError::ReadSelection { detail } => write!(f, "read task selection: {detail}"),
            RunError::NoCtxloom => f.write_str(
                "taskloom run requires ctxloom on PATH to launch the agent session \
                 (https://github.com/ctxloom/ctxloom); all other taskloom commands work without it",
            ),
            RunError::Exited { status } => write!(f, "ctxloom run exited with status {status}"),
            RunError::Launch { detail } => write!(f, "ctxloom run: {detail}"),
        }
    }
}

impl fmt::Display for RunNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunNotice::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for RunError {}

impl Diagnostic for RunError {
    fn code(&self) -> &'static str {
        match self {
            RunError::App(error) => error.code(),
            RunError::NotFound { .. } => "tasks-run-not-found",
            RunError::NotInteractive => "tasks-run-not-interactive",
            RunError::ReadSelection { .. } => "tasks-run-read-selection",
            RunError::NoCtxloom => "tasks-run-no-ctxloom",
            RunError::Exited { .. } => "tasks-run-exited",
            RunError::Launch { .. } => "tasks-run-launch",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            RunError::App(error) => error.args(),
            RunError::NotFound { harp_id } => vec![("harp_id", harp_id.as_str().into())],
            RunError::NotInteractive | RunError::NoCtxloom => Vec::new(),
            RunError::ReadSelection { detail } | RunError::Launch { detail } => {
                vec![("detail", detail.as_str().into())]
            }
            RunError::Exited { status } => vec![("status", Arg::Int(i64::from(*status)))],
        }
    }
}

impl Diagnostic for RunNotice {
    fn code(&self) -> &'static str {
        match self {
            RunNotice::Cancelled => "tasks-run-cancelled",
        }
    }

    // Equivalent mutant: an empty Vec is its only body, `vec![]` too.
    #[cfg_attr(test, mutants::skip)]
    fn args(&self) -> Vec<(&'static str, Arg)> {
        Vec::new()
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = ("tasks-run", include_str!("../locales/en-US/tasks-run.ftl"));

/// One value of every error and notice this crate defines (not the
/// wrapper), for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(RunError::NotFound {
            harp_id: "swift-amber".to_string(),
        }),
        Box::new(RunError::NotInteractive),
        Box::new(RunError::ReadSelection {
            detail: "Input/output error (os error 5)".to_string(),
        }),
        Box::new(RunError::NoCtxloom),
        Box::new(RunError::Exited { status: 3 }),
        Box::new(RunError::Launch {
            detail: "terminated by signal 9".to_string(),
        }),
        Box::new(RunNotice::Cancelled),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
