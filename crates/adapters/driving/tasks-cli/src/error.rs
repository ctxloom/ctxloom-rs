//! Why a command did not complete.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

use crate::Notice;

#[derive(Clone, PartialEq, Debug)]
pub enum CliError {
    /// The use case failed: its error's diagnostic.
    App(AppError),
    /// The answer could not be written out.
    Output { detail: String },
    /// The command line could not be parsed: clap's account of why, with
    /// the usage.
    Usage { detail: String },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::App(error) => error.fmt(f),
            CliError::Output { detail } => write!(f, "cannot write the output: {detail}"),
            CliError::Usage { detail } => f.write_str(detail),
        }
    }
}

impl Diagnostic for CliError {
    fn code(&self) -> &'static str {
        match self {
            CliError::App(error) => error.code(),
            CliError::Output { .. } => "tasks-cli-output",
            CliError::Usage { .. } => "tasks-cli-usage",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            CliError::App(error) => error.args(),
            CliError::Output { detail } | CliError::Usage { detail } => {
                vec![("detail", detail.as_str().into())]
            }
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = ("tasks-cli", include_str!("../locales/en-US/tasks-cli.ftl"));

/// One value of every error and notice this crate defines (not the
/// wrapper), for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(CliError::Output {
            detail: "broken pipe".to_string(),
        }),
        Box::new(CliError::Usage {
            detail: "error: unrecognized subcommand 'frobnicate'\n".to_string(),
        }),
        Box::new(Notice::Project {
            root: "/work/tree".to_string(),
            id: "jolly-mute-magma".to_string(),
        }),
        Box::new(Notice::NoProject),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
