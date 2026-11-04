//! Why `init` did not complete.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};
use ctxloom_ports::Failure;

#[derive(Debug)]
pub enum InitError {
    /// The directory has no `.ctxloom`, and a new project's init is not built.
    NewProjectNotBuilt { app_dir: String },
    /// The directory could not be inspected: the adapter's own words.
    Files(Failure),
}

impl fmt::Display for InitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for InitError {
    fn code(&self) -> &str {
        match self {
            InitError::NewProjectNotBuilt { .. } => code!("new-project-not-built"),
            InitError::Files(cause) => cause.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            InitError::NewProjectNotBuilt { app_dir } => vec![("app_dir", app_dir.as_str().into())],
            InitError::Files(cause) => cause.args(),
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error this crate defines (not the wrapper), for the
/// catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![Box::new(InitError::NewProjectNotBuilt {
        app_dir: "/work/.ctxloom".to_string(),
    })]
}
