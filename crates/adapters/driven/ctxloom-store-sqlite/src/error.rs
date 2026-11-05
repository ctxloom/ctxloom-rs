//! Why a project's store could not be opened: the project's id was refused
//! (the shared `project-dir` crate's check), or the opener failed
//! (`sqlite-store-opener`'s). Neither has a code of its own here: each is
//! reported under its crate's code and catalog.

use std::fmt;

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_embedded::angzarr_ports::{Arg as PortArg, Cause};
use angzarr_embedded::StoreError;
use project_dir::ProjectDirError;
use sqlite_store_opener::OpenError;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StoreOpenError {
    /// The project's id is not one plain name: no path was made from it.
    UnsafeProject(ProjectDirError),
    /// The opener refused or failed.
    Open(OpenError),
}

impl fmt::Display for StoreOpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for StoreOpenError {}

impl Diagnostic for StoreOpenError {
    fn code(&self) -> &str {
        match self {
            StoreOpenError::UnsafeProject(unsafe_id) => unsafe_id.static_code(),
            StoreOpenError::Open(error) => error.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            StoreOpenError::UnsafeProject(unsafe_id) => unsafe_id.args(),
            StoreOpenError::Open(error) => error.args(),
        }
    }
}

impl StoreOpenError {
    /// The error as the store's own: how the port carries it in a
    /// `StoreError`.
    pub fn into_store_error(self) -> StoreError {
        match self {
            StoreOpenError::UnsafeProject(unsafe_id) => StoreError::Unavailable(Cause::new(
                unsafe_id.static_code(),
                unsafe_id
                    .id()
                    .map(|id| ("id", PortArg::Str(id.to_string()))),
            )),
            StoreOpenError::Open(error) => error.into_store_error(),
        }
    }
}
