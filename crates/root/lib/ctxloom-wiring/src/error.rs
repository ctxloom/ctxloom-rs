//! Why the wiring could not do what it was asked.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};
use ctxloom_store_sqlite::StoreOpenError;

/// This crate's en-US messages.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LibError {
    /// The home variable is not set (or is empty).
    NoHome { variable: String },
    /// A session id that is not a UUID.
    BadSession { session: String, detail: String },
    /// No runtime could be started to open the store.
    NoRuntime { detail: String },
    /// The project's store would not open.
    Store(StoreOpenError),
    /// The test-only seed could not plant a session.
    #[cfg(feature = "test-support")]
    Seed { detail: String },
}

impl fmt::Display for LibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for LibError {}

impl Diagnostic for LibError {
    fn code(&self) -> &str {
        match self {
            LibError::NoHome { .. } => code!("no-home"),
            LibError::BadSession { .. } => code!("bad-session"),
            LibError::NoRuntime { .. } => code!("no-runtime"),
            LibError::Store(_) => code!("store"),
            #[cfg(feature = "test-support")]
            LibError::Seed { .. } => code!("seed"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            LibError::NoHome { variable } => vec![("variable", variable.as_str().into())],
            LibError::BadSession { session, detail } => vec![
                ("session", session.as_str().into()),
                ("ext_err_txt", detail.as_str().into()),
            ],
            LibError::NoRuntime { detail } => vec![("ext_err_txt", detail.as_str().into())],
            LibError::Store(cause) => vec![("error", Arg::Nested(Box::new(cause.clone())))],
            #[cfg(feature = "test-support")]
            LibError::Seed { detail } => vec![("ext_err_txt", detail.as_str().into())],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(LibError::NoHome {
            variable: "HOME".to_string(),
        }),
        Box::new(LibError::BadSession {
            session: "x".to_string(),
            detail: "invalid length".to_string(),
        }),
        Box::new(LibError::NoRuntime {
            detail: "too many open files".to_string(),
        }),
        Box::new(LibError::Store(StoreOpenError::Open(
            ctxloom_store_sqlite::OpenError::Unplaced {
                path: "/home/u/.ctxloom/projects/p/ctxloom.db".to_string(),
                detail: "no place on disk".to_string(),
            },
        ))),
        Box::new(LibError::Store(StoreOpenError::UnsafeProject(
            project_dir::ProjectDirError::IdSeparator {
                id: "../escape".to_string(),
            },
        ))),
        #[cfg(feature = "test-support")]
        Box::new(LibError::Seed {
            detail: "no such project".to_string(),
        }),
    ]
}
