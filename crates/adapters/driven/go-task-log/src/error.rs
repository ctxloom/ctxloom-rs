//! Why a Go task log could not be read: as Go's fold refuses a log, the
//! whole log, never one line.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LogError {
    /// No home to find Go's logs in.
    NoHome,
    /// The log, or the directory of logs, could not be read.
    Unreadable { path: String, detail: String },
    /// A line is not a JSON record Go could decode.
    Malformed {
        path: String,
        line: u32,
        detail: String,
    },
    /// A record without an op.
    NoOp { path: String, line: u32 },
    /// A record whose op Go's fold does not know.
    UnknownOp { path: String, line: u32, op: String },
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::NoHome => f.write_str("HOME is not set, so Go's task logs cannot be found"),
            LogError::Unreadable { path, detail } => write!(f, "cannot read {path}: {detail}"),
            LogError::Malformed { path, line, detail } => {
                write!(f, "{path}:{line}: malformed task event: {detail}")
            }
            LogError::NoOp { path, line } => write!(f, "{path}:{line}: record has no op field"),
            LogError::UnknownOp { path, line, op } => write!(
                f,
                "{path}:{line}: unrecognized op {op}: the log was likely written by a newer taskloom"
            ),
        }
    }
}

impl Diagnostic for LogError {
    fn code(&self) -> &'static str {
        match self {
            LogError::NoHome => "go-task-log-no-home",
            LogError::Unreadable { .. } => "go-task-log-unreadable",
            LogError::Malformed { .. } => "go-task-log-malformed",
            LogError::NoOp { .. } => "go-task-log-no-op",
            LogError::UnknownOp { .. } => "go-task-log-unknown-op",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            LogError::NoHome => Vec::new(),
            LogError::Unreadable { path, detail } => vec![
                ("path", path.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            LogError::Malformed { path, line, detail } => vec![
                ("path", path.as_str().into()),
                ("line", (*line).into()),
                ("detail", detail.as_str().into()),
            ],
            LogError::NoOp { path, line } => {
                vec![("path", path.as_str().into()), ("line", (*line).into())]
            }
            LogError::UnknownOp { path, line, op } => vec![
                ("path", path.as_str().into()),
                ("line", (*line).into()),
                ("op", op.as_str().into()),
            ],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "go-task-log",
    include_str!("../locales/en-US/go-task-log.ftl"),
);

/// One value of every error, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/u/.ctxloom/tasks/jolly-mute-magma.jsonl".to_string();
    vec![
        Box::new(LogError::NoHome),
        Box::new(LogError::Unreadable {
            path: path(),
            detail: "permission denied".to_string(),
        }),
        Box::new(LogError::Malformed {
            path: path(),
            line: 3,
            detail: "expected value at line 1 column 1".to_string(),
        }),
        Box::new(LogError::NoOp {
            path: path(),
            line: 4,
        }),
        Box::new(LogError::UnknownOp {
            path: path(),
            line: 5,
            op: "archive".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
