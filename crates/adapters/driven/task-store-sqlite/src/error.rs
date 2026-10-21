//! Why a project's store could not be opened, beyond what annals-store-sql
//! reports itself.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StoreOpenError {
    /// The directory the database goes in could not be made.
    CreateDir { path: String, detail: String },
    /// No runtime could be started to open the database and check it.
    NoRuntime { detail: String },
}

impl fmt::Display for StoreOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreOpenError::CreateDir { path, detail } => {
                write!(f, "cannot create the store's directory {path}: {detail}")
            }
            StoreOpenError::NoRuntime { detail } => {
                write!(f, "cannot start a runtime to check the store: {detail}")
            }
        }
    }
}

impl Diagnostic for StoreOpenError {
    fn code(&self) -> &'static str {
        match self {
            StoreOpenError::CreateDir { .. } => "task-store-sqlite-create-dir",
            StoreOpenError::NoRuntime { .. } => "task-store-sqlite-no-runtime",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            StoreOpenError::CreateDir { path, detail } => vec![
                ("path", path.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            StoreOpenError::NoRuntime { detail } => vec![("detail", detail.as_str().into())],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "task-store-sqlite",
    include_str!("../locales/en-US/task-store-sqlite.ftl"),
);

/// One value of every error, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/u/.ctxloom/projects/jolly-mute-magma".to_string();
    vec![
        Box::new(StoreOpenError::CreateDir {
            path: path(),
            detail: "permission denied".to_string(),
        }),
        Box::new(StoreOpenError::NoRuntime {
            detail: "too many open files".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
