//! Why a project's store could not be opened, beyond what annals-store-sql
//! reports itself.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StoreOpenError {
    /// The directory the database goes in could not be made.
    CreateDir { path: String, detail: String },
    /// The open, run on a blocking thread, panicked or was cancelled.
    Interrupted { path: String, detail: String },
}

impl fmt::Display for StoreOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreOpenError::CreateDir { path, detail } => {
                write!(f, "cannot create the store's directory {path}: {detail}")
            }
            StoreOpenError::Interrupted { path, detail } => {
                write!(f, "opening the store {path} did not finish: {detail}")
            }
        }
    }
}

impl Diagnostic for StoreOpenError {
    fn code(&self) -> &'static str {
        match self {
            StoreOpenError::CreateDir { .. } => "task-store-sqlite-create-dir",
            StoreOpenError::Interrupted { .. } => "task-store-sqlite-open-interrupted",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            StoreOpenError::CreateDir { path, detail }
            | StoreOpenError::Interrupted { path, detail } => vec![
                ("path", path.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
        }
    }
}

/// One value of every error, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/u/.ctxloom/projects/jolly-mute-magma".to_string();
    vec![
        Box::new(StoreOpenError::CreateDir {
            path: path(),
            detail: "permission denied".to_string(),
        }),
        Box::new(StoreOpenError::Interrupted {
            path: path(),
            detail: "task panicked".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
