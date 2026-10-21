//! Why a command did not complete.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

#[derive(Clone, PartialEq, Debug)]
pub enum CliError {
    /// The use case failed: its error's diagnostic.
    App(AppError),
    /// The answer could not be written out.
    Output { detail: String },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::App(error) => error.fmt(f),
            CliError::Output { detail } => write!(f, "cannot write the output: {detail}"),
        }
    }
}

impl Diagnostic for CliError {
    fn code(&self) -> &'static str {
        match self {
            CliError::App(error) => error.code(),
            CliError::Output { .. } => "tasks-cli-output",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            CliError::App(error) => error.args(),
            CliError::Output { detail } => vec![("detail", detail.as_str().into())],
        }
    }
}

/// One value of every error this crate defines (not the wrapper), for the
/// catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![Box::new(CliError::Output {
        detail: "broken pipe".to_string(),
    })]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
