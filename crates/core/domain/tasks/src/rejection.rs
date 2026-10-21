//! Why a command to a task was refused.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TaskRejection {
    /// An add without a harp id.
    HarpIdRequired,
    /// An add whose harp id is not the one its stream's root derives from.
    NotItsStream { harp_id: String },
    /// An add to a stream that already holds a task: the harp id is taken.
    TaskExists { harp_id: String },
    /// A command to a stream holding no task.
    NoTask,
    /// A command to a removed task.
    Removed { harp_id: String },
    /// Text that is empty once trimmed.
    TextRequired,
    /// A status change without a status.
    StatusRequired,
    /// A status starting with the status-class sigil.
    StatusReserved { status: String },
    /// Deferred without a revive trigger, given or already held.
    TriggerRequired,
    /// A retag with no tag once trimmed.
    TagRequired,
}

impl fmt::Display for TaskRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskRejection::HarpIdRequired => f.write_str("a task needs a harp id"),
            TaskRejection::NotItsStream { harp_id } => {
                write!(f, "task {harp_id} does not belong in this stream")
            }
            TaskRejection::TaskExists { harp_id } => write!(f, "task {harp_id} already exists"),
            TaskRejection::NoTask => f.write_str("there is no such task"),
            TaskRejection::Removed { harp_id } => write!(f, "task {harp_id} was removed"),
            TaskRejection::TextRequired => f.write_str("a task needs text"),
            TaskRejection::StatusRequired => f.write_str("a status change needs a status"),
            TaskRejection::StatusReserved { status } => write!(
                f,
                "the status {status} is not allowed: '@' is reserved for status classes"
            ),
            TaskRejection::TriggerRequired => {
                f.write_str("a Deferred task needs a trigger: the condition that should revive it")
            }
            TaskRejection::TagRequired => f.write_str("a retag needs at least one tag"),
        }
    }
}

impl Diagnostic for TaskRejection {
    fn code(&self) -> &'static str {
        match self {
            TaskRejection::HarpIdRequired => "tasks-harp-id-required",
            TaskRejection::NotItsStream { .. } => "tasks-not-its-stream",
            TaskRejection::TaskExists { .. } => "tasks-task-exists",
            TaskRejection::NoTask => "tasks-no-task",
            TaskRejection::Removed { .. } => "tasks-removed",
            TaskRejection::TextRequired => "tasks-text-required",
            TaskRejection::StatusRequired => "tasks-status-required",
            TaskRejection::StatusReserved { .. } => "tasks-status-reserved",
            TaskRejection::TriggerRequired => "tasks-trigger-required",
            TaskRejection::TagRequired => "tasks-tag-required",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            TaskRejection::NotItsStream { harp_id }
            | TaskRejection::TaskExists { harp_id }
            | TaskRejection::Removed { harp_id } => {
                vec![("harp_id", harp_id.as_str().into())]
            }
            TaskRejection::StatusReserved { status } => vec![("status", status.as_str().into())],
            _ => Vec::new(),
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = ("tasks", include_str!("../locales/en-US/tasks.ftl"));

/// One value of every rejection, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let harp_id = || "swift-amber".to_string();
    vec![
        Box::new(TaskRejection::HarpIdRequired),
        Box::new(TaskRejection::NotItsStream { harp_id: harp_id() }),
        Box::new(TaskRejection::TaskExists { harp_id: harp_id() }),
        Box::new(TaskRejection::NoTask),
        Box::new(TaskRejection::Removed { harp_id: harp_id() }),
        Box::new(TaskRejection::TextRequired),
        Box::new(TaskRejection::StatusRequired),
        Box::new(TaskRejection::StatusReserved {
            status: "@open".to_string(),
        }),
        Box::new(TaskRejection::TriggerRequired),
        Box::new(TaskRejection::TagRequired),
    ]
}
