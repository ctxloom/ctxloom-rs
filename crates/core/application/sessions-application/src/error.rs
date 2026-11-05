//! Why a use case did not complete, and what it refused.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic, Report};

use sessions::SessionRejection;

use crate::DeadLetter;

/// Why a use case did not complete. A wrapper of another error
/// ([`AppError::Rejected`], [`AppError::Failed`], [`AppError::Identity`],
/// [`AppError::Files`], [`AppError::DeadLettered`]) is that error's
/// diagnostic: its code, args and English.
#[derive(Clone, PartialEq, Debug)]
pub enum AppError {
    /// No session of that name (Go's `errNoSession`).
    NoSession { harp: String },
    /// A transcript asked for of a session that has none captured.
    NoCapturedTranscript { harp: String },
    /// The domain refused the command: tell the person.
    Rejected(Report),
    /// The store or the kernel failed in a way a retry will not mend.
    Failed(Report),
    /// The project could not be resolved.
    Identity(Report),
    /// A session's files could not be read or removed.
    Files(Report),
    /// Every attempt the retry budget allows failed with a retryable error;
    /// `error` is the last.
    GaveUp { attempts: u32, error: Report },
    /// `error`, from a write whose kernel calls answered `dead_letters`
    /// (never none): `error`'s diagnostic with the letters beside it for the
    /// driving adapter to say.
    DeadLettered {
        error: Box<AppError>,
        dead_letters: Vec<DeadLetter>,
    },
}

impl AppError {
    /// The dead letters the kernel calls of the use case that failed with
    /// this answered: none for most.
    pub fn dead_letters(&self) -> &[DeadLetter] {
        match self {
            AppError::DeadLettered { dead_letters, .. } => dead_letters,
            _ => &[],
        }
    }

    /// The process exit code: 2 when the request is invalid on its own
    /// content (a name no session may have), 1 when it ran against state and
    /// the answer was negative.
    pub fn exit_code(&self) -> u8 {
        match self {
            AppError::DeadLettered { error, .. } => error.exit_code(),
            AppError::Rejected(cause) => {
                let invalid = [
                    SessionRejection::HarpInvalid {
                        harp: String::new(),
                        reason: String::new(),
                    }
                    .code()
                    .to_string(),
                    SessionRejection::HarpTooLong {
                        harp: String::new(),
                        limit: 0,
                        length: 0,
                    }
                    .code()
                    .to_string(),
                ];
                if invalid.iter().any(|code| code == cause.code()) {
                    2
                } else {
                    1
                }
            }
            AppError::NoSession { .. }
            | AppError::NoCapturedTranscript { .. }
            | AppError::Failed(_)
            | AppError::Identity(_)
            | AppError::Files(_)
            | AppError::GaveUp { .. } => 1,
        }
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
            AppError::NoSession { .. } => code!("no-session"),
            AppError::NoCapturedTranscript { .. } => code!("no-captured-transcript"),
            AppError::GaveUp { .. } => code!("gave-up"),
            AppError::Rejected(cause)
            | AppError::Failed(cause)
            | AppError::Identity(cause)
            | AppError::Files(cause) => cause.code(),
            AppError::DeadLettered { error, .. } => error.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            AppError::NoSession { harp } | AppError::NoCapturedTranscript { harp } => {
                vec![("harp", harp.as_str().into())]
            }
            AppError::GaveUp { attempts, error } => vec![
                ("attempts", u64::from(*attempts).into()),
                ("error", Arg::Nested(Box::new(error.clone()))),
            ],
            AppError::Rejected(cause)
            | AppError::Failed(cause)
            | AppError::Identity(cause)
            | AppError::Files(cause) => cause.args(),
            AppError::DeadLettered { error, .. } => error.args(),
        }
    }
}

/// The domain's refusal of a command, as a use case answers it.
impl From<angzarr_embedded_runner::Refusal<SessionRejection>> for AppError {
    fn from(refusal: angzarr_embedded_runner::Refusal<SessionRejection>) -> AppError {
        AppError::Rejected(refusal.report)
    }
}

/// What kept a command from being decided at all.
impl From<angzarr_embedded_runner::Unrun> for AppError {
    fn from(unrun: angzarr_embedded_runner::Unrun) -> AppError {
        match unrun {
            angzarr_embedded_runner::Unrun::GaveUp { attempts, error } => {
                AppError::GaveUp { attempts, error }
            }
            angzarr_embedded_runner::Unrun::Failed(report) => AppError::Failed(report),
        }
    }
}

/// Why `session remove` refused, after printing its plan: the answer ends with
/// what was NOT done (Go: `harpPurgeRefusal`), and exits 2.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The session's lock does not prove its owner dead: held, or no lock at
    /// all (every session from before the lock existed). `reason` says which.
    NotProvenDead {
        harp: String,
        reason: String,
        /// The command that was run, as the person spelled it (`session
        /// remove`, `session purge`, `session transcript purge`).
        verb: String,
    },
    /// The session is marked keep.
    Kept { harp: String },
    /// The session ended within the transcript retention, or nothing proves
    /// when it ended; `detail` says which.
    Retained { harp: String, detail: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for Refusal {
    fn code(&self) -> &str {
        match self {
            Refusal::NotProvenDead { .. } => code!("refused-not-dead"),
            Refusal::Kept { .. } => code!("refused-kept"),
            Refusal::Retained { .. } => code!("refused-retained"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Refusal::NotProvenDead { harp, reason, verb } => vec![
                ("harp", harp.as_str().into()),
                ("reason", reason.as_str().into()),
                ("verb", verb.as_str().into()),
            ],
            Refusal::Kept { harp } => vec![("harp", harp.as_str().into())],
            Refusal::Retained { harp, detail } => vec![
                ("harp", harp.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
        }
    }
}

/// What a report-only run ends with: the plan was not applied, and the exact
/// command that would apply it (Go: `reportPlanOnly`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemovedNothing {
    pub command: String,
}

impl fmt::Display for RemovedNothing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for RemovedNothing {
    fn code(&self) -> &str {
        code!("removed-nothing")
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        vec![("command", self.command.as_str().into())]
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// The en-US catalogs of every diagnostic a use case returns but its
/// adapters': the domain's rejections and this crate's.
pub const CATALOGS: [(&str, &str); 2] = [sessions::EN_US, EN_US];

/// One value of every error and refusal this crate defines (not the
/// wrappers, whose codes are their causes'), for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let harp = || "amber-swift-owl".to_string();
    let mut samples = crate::go_sidecar::diagnostic_samples();
    samples.extend::<Vec<Box<dyn Diagnostic>>>(vec![
        Box::new(crate::Why::NoProject {
            project_dir: "/work/elsewhere".to_string(),
        }),
        Box::new(crate::Why::NameTaken { harp: harp() }),
    ]);
    samples.extend::<Vec<Box<dyn Diagnostic>>>(vec![
        Box::new(AppError::NoSession { harp: harp() }),
        Box::new(AppError::NoCapturedTranscript { harp: harp() }),
        Box::new(AppError::GaveUp {
            attempts: 10,
            error: Report::new("x-busy"),
        }),
        Box::new(Refusal::NotProvenDead {
            harp: harp(),
            reason: "no lock file".to_string(),
            verb: "session remove".to_string(),
        }),
        Box::new(Refusal::Kept { harp: harp() }),
        Box::new(Refusal::Retained {
            harp: harp(),
            detail: "it ended an hour ago".to_string(),
        }),
        Box::new(RemovedNothing {
            command: "ctxloom session remove amber-swift-owl --yes".to_string(),
        }),
    ]);
    samples
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
