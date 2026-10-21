//! Why a use case did not complete.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic, Report};

/// Why a use case did not complete. A wrapper of another error
/// ([`AppError::Rejected`], [`AppError::Failed`], [`AppError::Identity`]) is
/// that error's diagnostic: its code, args and English.
#[derive(Clone, PartialEq, Debug)]
pub enum AppError {
    /// The domain refused the command: tell the person.
    Rejected(Report),
    /// The store or the kernel failed in a way a retry will not mend.
    Failed(Report),
    /// The project could not be resolved.
    Identity(Report),
    /// Every attempt the retry budget allows failed with a retryable error;
    /// `error` is the last.
    GaveUp { attempts: u32, error: Report },
    /// Every freshly minted task id was already taken.
    HarpsExhausted { attempts: u32 },
    /// A task the store accepted is not there when read back.
    Vanished { harp_id: String },
    /// A change to an existing task where the work root has no project.
    NoProject,
    /// An import of a project Go keeps no task log for.
    NoGoLog { project: String },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Rejected(cause) | AppError::Failed(cause) | AppError::Identity(cause) => {
                cause.fmt(f)
            }
            AppError::GaveUp { attempts, error } => {
                write!(f, "gave up after {attempts} attempts: {error}")
            }
            AppError::HarpsExhausted { attempts } => {
                write!(f, "no free task id after {attempts} tries")
            }
            AppError::Vanished { harp_id } => {
                write!(f, "task {harp_id} was written but cannot be read back")
            }
            AppError::NoProject => {
                f.write_str("there is no project here yet, so no task to change")
            }
            AppError::NoGoLog { project } => {
                write!(f, "Go taskloom has no task log for project {project}")
            }
        }
    }
}

impl Diagnostic for AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::Rejected(cause) | AppError::Failed(cause) | AppError::Identity(cause) => {
                cause.code()
            }
            AppError::GaveUp { .. } => "tasks-application-gave-up",
            AppError::HarpsExhausted { .. } => "tasks-application-harps-exhausted",
            AppError::Vanished { .. } => "tasks-application-vanished",
            AppError::NoProject => "tasks-application-no-project",
            AppError::NoGoLog { .. } => "tasks-application-no-go-log",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            AppError::Rejected(cause) | AppError::Failed(cause) | AppError::Identity(cause) => {
                cause.args()
            }
            AppError::GaveUp { attempts, error } => vec![
                ("attempts", (*attempts).into()),
                ("error", Arg::nested(error)),
            ],
            AppError::HarpsExhausted { attempts } => vec![("attempts", (*attempts).into())],
            AppError::Vanished { harp_id } => vec![("harp_id", harp_id.as_str().into())],
            AppError::NoProject => Vec::new(),
            AppError::NoGoLog { project } => vec![("project", project.as_str().into())],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "tasks-application",
    include_str!("../locales/en-US/tasks-application.ftl"),
);

/// The en-US catalogs of every diagnostic a use case returns but its
/// adapters': the domain's rejections and this crate's errors.
pub const CATALOGS: [(&str, &str); 2] = [tasks::EN_US, EN_US];

/// One value of every error this crate defines, and of every reason an
/// import gives for leaving a record behind (not the wrappers, whose codes
/// are their causes'), for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(AppError::GaveUp {
            attempts: 10,
            error: Report::new("x-busy", "busy"),
        }),
        Box::new(AppError::HarpsExhausted { attempts: 100 }),
        Box::new(AppError::Vanished {
            harp_id: "swift-amber".to_string(),
        }),
        Box::new(AppError::NoProject),
        Box::new(AppError::NoGoLog {
            project: "jolly-mute-magma".to_string(),
        }),
        Box::new(crate::Why::Displaced),
        Box::new(crate::Why::HarpTaken),
        Box::new(crate::Why::TaskNotImported),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
