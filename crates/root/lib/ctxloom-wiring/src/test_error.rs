//! What the test-only commands and the seed raise (`test-support` only).
//!
//! Their own type, beside [`LibError`](crate::LibError): a cargo feature adds
//! code and never a variant of a public error, and the command line carries
//! any `Diagnostic` out through `Failure`.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TestError {
    /// The test-only seed could not plant a session.
    Seed { detail: String },
    /// A test command that refuses.
    Refused { verb: String },
    /// A test command with fatal findings.
    Findings { count: u64 },
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for TestError {}

impl Diagnostic for TestError {
    fn code(&self) -> &str {
        match self {
            TestError::Seed { .. } => code!("seed"),
            TestError::Refused { .. } => code!("test-refused"),
            TestError::Findings { .. } => code!("test-findings"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            TestError::Seed { detail } => vec![("ext_err_txt", detail.as_str().into())],
            TestError::Refused { verb } => vec![("verb", verb.as_str().into())],
            TestError::Findings { count } => vec![("count", (*count).into())],
        }
    }
}

/// One value of every test error, for a catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(TestError::Seed {
            detail: "no such project".to_string(),
        }),
        Box::new(TestError::Refused {
            verb: "refuse".to_string(),
        }),
        Box::new(TestError::Findings { count: 1 }),
    ]
}

#[cfg(test)]
#[path = "test_error.test.rs"]
mod tests;
