//! Why a watch stopped other than by being asked to; and this crate's
//! catalog.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

#[derive(Clone, PartialEq, Debug)]
pub enum WatchError {
    /// The use case failed: its error's diagnostic.
    App(AppError),
    /// The work root has no project yet, so there is nothing to follow;
    /// a watch mints none to get one.
    NoProject,
    /// A line could not be written: the reader went away.
    Output { detail: String },
}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchError::App(error) => error.fmt(f),
            WatchError::NoProject => {
                f.write_str("no project here yet; add a task first (taskloom add \"<task text>\")")
            }
            WatchError::Output { detail } => {
                write!(f, "cannot write the change stream: {detail}")
            }
        }
    }
}

impl std::error::Error for WatchError {}

impl Diagnostic for WatchError {
    fn code(&self) -> &'static str {
        match self {
            WatchError::App(error) => error.code(),
            WatchError::NoProject => "tasks-watch-no-project",
            WatchError::Output { .. } => "tasks-watch-output",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            WatchError::App(error) => error.args(),
            WatchError::NoProject => Vec::new(),
            WatchError::Output { detail } => vec![("detail", detail.as_str().into())],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "tasks-watch",
    include_str!("../locales/en-US/tasks-watch.ftl"),
);

/// One value of every error this crate defines (not the wrapper), for the
/// catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(WatchError::NoProject),
        Box::new(WatchError::Output {
            detail: "Broken pipe (os error 32)".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
