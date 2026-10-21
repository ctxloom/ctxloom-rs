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
    /// An import left something behind: projects it stopped in, and
    /// records it could not bring over.
    Incomplete { stopped: usize, lost: usize },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::App(error) => error.fmt(f),
            CliError::Output { detail } => write!(f, "cannot write the output: {detail}"),
            CliError::Incomplete { stopped, lost } => write!(
                f,
                "not everything was imported: {stopped} projects stopped, {lost} records not imported"
            ),
        }
    }
}

impl Diagnostic for CliError {
    fn code(&self) -> &'static str {
        match self {
            CliError::App(error) => error.code(),
            CliError::Output { .. } => "tasks-cli-output",
            CliError::Incomplete { .. } => "tasks-cli-import-incomplete",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            CliError::App(error) => error.args(),
            CliError::Output { detail } => vec![("detail", detail.as_str().into())],
            CliError::Incomplete { stopped, lost } => {
                vec![("stopped", (*stopped).into()), ("lost", (*lost).into())]
            }
        }
    }
}

/// One value of every error this crate defines (not the wrapper), for the
/// catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(CliError::Output {
            detail: "broken pipe".to_string(),
        }),
        Box::new(CliError::Incomplete {
            stopped: 1,
            lost: 2,
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
