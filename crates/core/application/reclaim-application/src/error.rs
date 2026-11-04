//! Why a reclaim use case did not complete.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};
use ctxloom_ports::Failure;
use sessions_application::AppError;

#[derive(Debug)]
pub enum ReclaimError {
    /// A flag's age or date that is neither.
    BadAge { flag: String, value: String },
    /// The cache could not be measured or removed: the adapter's own words.
    Cache(Failure),
    /// A sessions use case failed.
    Sessions(AppError),
}

impl ReclaimError {
    /// The process exit code: 2 for a flag that is invalid on its own
    /// content, else what the sessions use case asks for, else 1.
    pub fn exit_code(&self) -> u8 {
        match self {
            ReclaimError::BadAge { .. } => 2,
            ReclaimError::Sessions(error) => error.exit_code(),
            ReclaimError::Cache(_) => 1,
        }
    }
}

impl From<AppError> for ReclaimError {
    fn from(error: AppError) -> ReclaimError {
        ReclaimError::Sessions(error)
    }
}

impl fmt::Display for ReclaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for ReclaimError {
    fn code(&self) -> &str {
        match self {
            ReclaimError::BadAge { .. } => code!("bad-age"),
            ReclaimError::Cache(cause) => cause.code(),
            ReclaimError::Sessions(cause) => cause.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            ReclaimError::BadAge { flag, value } => {
                vec![
                    ("flag", flag.as_str().into()),
                    ("value", value.as_str().into()),
                ]
            }
            ReclaimError::Cache(cause) => cause.args(),
            ReclaimError::Sessions(cause) => cause.args(),
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error this crate defines (not the wrappers, whose codes
/// are their causes'), for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![Box::new(ReclaimError::BadAge {
        flag: "--purge-older-than".to_string(),
        value: "soon".to_string(),
    })]
}
