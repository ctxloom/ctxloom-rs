//! Why a project's store could not be opened. A third party's text (the
//! OS's, SQLite's) is the arg `ext_err_txt`, verbatim.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StoreOpenError {
    /// The directory the database goes in could not be made.
    CreateDir { path: String, detail: String },
    /// The filesystem could not say where the database is on disk.
    Unplaced { path: String, detail: String },
    /// The store would not open.
    Open { path: String, detail: String },
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
            StoreOpenError::CreateDir { .. } => code!("create-dir"),
            StoreOpenError::Unplaced { .. } => code!("unplaced"),
            StoreOpenError::Open { .. } => code!("open"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        let (StoreOpenError::CreateDir { path, detail }
        | StoreOpenError::Unplaced { path, detail }
        | StoreOpenError::Open { path, detail }) = self;
        vec![
            ("path", path.as_str().into()),
            ("ext_err_txt", detail.as_str().into()),
        ]
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let at = |path: &str, detail: &str| (path.to_string(), detail.to_string());
    let (path, detail) = at("/home/x/.ctxloom/projects/p", "permission denied");
    vec![
        Box::new(StoreOpenError::CreateDir {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(StoreOpenError::Unplaced {
            path: path.clone(),
            detail: detail.clone(),
        }),
        Box::new(StoreOpenError::Open { path, detail }),
    ]
}

impl StoreOpenError {
    /// The error as angzarr-ports' cause, its code and args: how the
    /// angzarr-embedded store carries it in a `StoreError`.
    pub fn cause(&self) -> angzarr_embedded::angzarr_ports::Cause {
        use angzarr_embedded::angzarr_ports::{Arg as PortArg, Cause};
        let code = match self {
            StoreOpenError::CreateDir { .. } => code!("create-dir"),
            StoreOpenError::Unplaced { .. } => code!("unplaced"),
            StoreOpenError::Open { .. } => code!("open"),
        };
        let (StoreOpenError::CreateDir { path, detail }
        | StoreOpenError::Unplaced { path, detail }
        | StoreOpenError::Open { path, detail }) = self;
        Cause::new(
            code,
            [
                ("path", PortArg::Str(path.clone())),
                ("ext_err_txt", PortArg::Str(detail.clone())),
            ],
        )
    }
}
