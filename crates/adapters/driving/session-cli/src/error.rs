//! Why a `session` command did not complete at the command line, before or
//! after the application ran.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CliFault {
    /// A `--state` that is not a state (Go: `checkSessionState`).
    BadState { value: String, states: String },
    /// A bare `edit`: a session has no document to open, and no field was
    /// named (Go: `runSessionEdit`).
    EditNothing { harp: String },
    /// The runtime commands block on could not start.
    Runtime { ext_err_txt: String },
    /// The answer could not be written.
    Output { ext_err_txt: String },
}

impl CliFault {
    /// The process exit code: 2, a request invalid on its own content, or 1.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliFault::BadState { .. } | CliFault::EditNothing { .. } => 2,
            CliFault::Runtime { .. } | CliFault::Output { .. } => 1,
        }
    }
}

impl fmt::Display for CliFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for CliFault {
    fn code(&self) -> &str {
        match self {
            CliFault::BadState { .. } => code!("bad-state"),
            CliFault::EditNothing { .. } => code!("edit-nothing"),
            CliFault::Runtime { .. } => code!("runtime"),
            CliFault::Output { .. } => code!("output"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            CliFault::BadState { value, states } => vec![
                ("value", value.as_str().into()),
                ("states", states.as_str().into()),
            ],
            CliFault::EditNothing { harp } => vec![("harp", harp.as_str().into())],
            CliFault::Runtime { ext_err_txt } | CliFault::Output { ext_err_txt } => {
                vec![("ext_err_txt", ext_err_txt.as_str().into())]
            }
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every fault, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(CliFault::BadState {
            value: "running".to_string(),
            states: "live, ended, unknown".to_string(),
        }),
        Box::new(CliFault::EditNothing {
            harp: "amber-swift-owl".to_string(),
        }),
        Box::new(CliFault::Runtime {
            ext_err_txt: "no timer".to_string(),
        }),
        Box::new(CliFault::Output {
            ext_err_txt: "broken pipe".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
