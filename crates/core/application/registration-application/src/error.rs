//! Why a registration did not complete; and this crate's catalog.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic, Report};

#[derive(Clone, PartialEq, Debug)]
pub enum RegistrationError {
    /// `--engine` names no agent this binary knows. `known` lists them,
    /// comma-separated.
    UnknownEngine { engine: String, known: String },
    /// An install found no agent in use at the scope, and none was named.
    NoBackends { known: String },
    /// One agent's configuration could not be found, read or written.
    EngineFailed { engine: String, error: Report },
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistrationError::UnknownEngine { engine, known } => {
                write!(f, "unknown engine \"{engine}\"; known engines: {known}")
            }
            RegistrationError::NoBackends { known } => write!(
                f,
                "no agent backends detected; name one with --engine ({known})"
            ),
            RegistrationError::EngineFailed { engine, error } => write!(f, "{engine}: {error}"),
        }
    }
}

impl std::error::Error for RegistrationError {}

impl Diagnostic for RegistrationError {
    fn code(&self) -> &'static str {
        match self {
            RegistrationError::UnknownEngine { .. } => "registration-application-unknown-engine",
            RegistrationError::NoBackends { .. } => "registration-application-no-backends",
            RegistrationError::EngineFailed { .. } => "registration-application-engine-failed",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            RegistrationError::UnknownEngine { engine, known } => vec![
                ("engine", engine.as_str().into()),
                ("known", known.as_str().into()),
            ],
            RegistrationError::NoBackends { known } => vec![("known", known.as_str().into())],
            RegistrationError::EngineFailed { engine, error } => vec![
                ("engine", engine.as_str().into()),
                ("error", Arg::nested(error)),
            ],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "registration-application",
    include_str!("../locales/en-US/registration-application.ftl"),
);

/// One value of every error this crate defines, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(RegistrationError::UnknownEngine {
            engine: "foo".to_string(),
            known: "claude-code".to_string(),
        }),
        Box::new(RegistrationError::NoBackends {
            known: "claude-code".to_string(),
        }),
        Box::new(RegistrationError::EngineFailed {
            engine: "claude-code".to_string(),
            error: Report::of(&RegistrationError::NoBackends {
                known: "claude-code".to_string(),
            }),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
