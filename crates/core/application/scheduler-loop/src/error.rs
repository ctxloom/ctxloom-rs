//! Why a tick could not run.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoopError {
    /// The due model could not be caught up from the store.
    CatchUpFailed { reason: String },
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for LoopError {
    fn code(&self) -> &str {
        match self {
            LoopError::CatchUpFailed { .. } => code!("catch-up-failed"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            LoopError::CatchUpFailed { reason } => vec![("reason", reason.as_str().into())],
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![Box::new(LoopError::CatchUpFailed {
        reason: "the store is gone".to_string(),
    })]
}
