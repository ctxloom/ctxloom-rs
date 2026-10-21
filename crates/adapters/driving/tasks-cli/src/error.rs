//! Why a command did not complete.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use presentation::UnknownFormat;
use tasks_application::AppError;

use crate::Notice;

#[derive(Clone, PartialEq, Debug)]
pub enum CliError {
    /// The use case failed: its error's diagnostic.
    App(AppError),
    /// The answer could not be written out.
    Output { detail: String },
    /// An import left something behind: projects it stopped in, and
    /// records it could not bring over.
    Incomplete { stopped: usize, lost: usize },
    /// The command line could not be parsed: clap's account of why, with
    /// the usage.
    Usage { detail: String },
    /// A format the command does not speak: yaml, which only `loadout`
    /// does.
    Format(UnknownFormat),
    /// `loadout` asked for in a format other than yaml.
    LoadoutFormat { format: String },
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
            CliError::Usage { detail } => f.write_str(detail),
            CliError::Format(error) => error.fmt(f),
            CliError::LoadoutFormat { format } => {
                write!(f, "the loadout is printed only as yaml, not {format}")
            }
        }
    }
}

impl Diagnostic for CliError {
    fn code(&self) -> &'static str {
        match self {
            CliError::App(error) => error.code(),
            CliError::Output { .. } => "tasks-cli-output",
            CliError::Incomplete { .. } => "tasks-cli-import-incomplete",
            CliError::Usage { .. } => "tasks-cli-usage",
            CliError::Format(error) => error.code(),
            CliError::LoadoutFormat { .. } => "tasks-cli-loadout-format",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            CliError::App(error) => error.args(),
            CliError::Output { detail } | CliError::Usage { detail } => {
                vec![("detail", detail.as_str().into())]
            }
            CliError::Incomplete { stopped, lost } => {
                vec![("stopped", (*stopped).into()), ("lost", (*lost).into())]
            }
            CliError::Format(error) => error.args(),
            CliError::LoadoutFormat { format } => vec![("format", format.as_str().into())],
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
        Box::new(CliError::Incomplete {
            stopped: 1,
            lost: 2,
        }),
        Box::new(CliError::Usage {
            detail: "error: unrecognized subcommand 'frobnicate'\n".to_string(),
        }),
        Box::new(Notice::Project {
            root: "/work/tree".to_string(),
            id: "jolly-mute-magma".to_string(),
        }),
        Box::new(CliError::LoadoutFormat {
            format: "json".to_string(),
        }),
        Box::new(Notice::NoProject),
        Box::new(Notice::NothingToPrune),
        Box::new(Notice::ReportOnly {
            command: "taskloom prune --older-than 7d --status Done --yes".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
