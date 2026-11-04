//! Why a use case did not do what it was asked.

use std::collections::BTreeMap;
use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};
use angzarr_embedded::{Failure, Rejected};
use run_ports::Unserved;

/// Why a credential did not identify. For the AUDIT only: the caller is told
/// nothing of which (plan 5.3; listener plan 4.3: "never the token").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// No run by that id, or no run at that stream.
    UnknownRun,
    /// The token does not hash to the run's credential.
    Mismatch,
    /// The run has ended: every token, the owner's included, is revoked (D29).
    Revoked,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppError {
    /// The credential did not identify.
    Unauthenticated(Why),
    /// Authorized as far as the credential goes, but the request is for a
    /// stream, type or domain the identity may not reach: answered as an
    /// unknown domain is (the real reason is for the host's log).
    Denied(Unserved),
    /// The domain refused the command: its code and args.
    Refused {
        code: String,
        args: BTreeMap<String, String>,
    },
    /// The store or kernel failed.
    Failed { reason: String },
}

impl AppError {
    /// What a request the edge did not serve is: a hidden domain is a denial,
    /// the kernel's own failure its refusal or failure.
    pub(crate) fn from_unserved(unserved: Unserved) -> AppError {
        match unserved {
            Unserved::Failed(failure) => AppError::from_failure(failure),
            hidden @ Unserved::Hidden { .. } => AppError::Denied(hidden),
        }
    }

    /// A domain's refusal as its code and args; any other failure as its text.
    pub(crate) fn from_failure(failure: Failure<Rejected>) -> AppError {
        match failure {
            Failure::Rejected(rejected) => {
                let report = rejected.report();
                AppError::Refused {
                    code: report.code().to_string(),
                    args: report
                        .args()
                        .into_iter()
                        .map(|(name, arg)| (name.to_string(), arg_text(&arg)))
                        .collect(),
                }
            }
            other => AppError::Failed {
                reason: other.to_string(),
            },
        }
    }
}

/// An arg as text: a number or string as it is, a cause as its code.
fn arg_text(arg: &Arg) -> String {
    match arg {
        Arg::Int(n) => n.to_string(),
        Arg::Uint(n) => n.to_string(),
        Arg::Str(s) => s.clone(),
        Arg::Nested(cause) => cause.code().to_string(),
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for AppError {
    fn code(&self) -> &str {
        match self {
            AppError::Unauthenticated(_) => code!("unauthenticated"),
            AppError::Denied(_) => code!("denied"),
            AppError::Refused { .. } => code!("refused"),
            AppError::Failed { .. } => code!("failed"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            AppError::Unauthenticated(_) => Vec::new(),
            // What the caller may be told: the unknown domain it asked for.
            AppError::Denied(Unserved::Hidden { domain }) => {
                vec![("domain", domain.as_str().into())]
            }
            AppError::Denied(Unserved::Failed(_)) => vec![("domain", "".into())],
            AppError::Refused { code, .. } => vec![("reason", code.as_str().into())],
            AppError::Failed { reason } => vec![("reason", reason.as_str().into())],
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
    vec![
        Box::new(AppError::Unauthenticated(Why::Revoked)),
        Box::new(AppError::Denied(Unserved::Hidden {
            domain: "runs".to_string(),
        })),
        Box::new(AppError::Refused {
            code: "sessions-stopped".to_string(),
            args: BTreeMap::new(),
        }),
        Box::new(AppError::Failed {
            reason: "unavailable".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
