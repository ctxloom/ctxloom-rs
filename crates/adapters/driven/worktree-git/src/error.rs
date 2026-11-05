//! Why a workspace was refused.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WorktreeError {
    /// The agent's checkout path is occupied by something that cannot be
    /// shown to be its own earlier checkout. Never adopted, never a degrade.
    Unadoptable {
        path: String,
        branch: String,
        why: String,
    },
    /// A session's work directory could not be read.
    Unreadable { path: String, why: String },
    /// A git invocation did not finish within its limit; it was killed. The
    /// run is not degraded to the live directory: a hung git says nothing
    /// about whether a worktree could be made.
    GitTimedOut { command: String, timeout: String },
}

impl fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for WorktreeError {
    fn code(&self) -> &str {
        match self {
            WorktreeError::Unadoptable { .. } => code!("unadoptable"),
            WorktreeError::Unreadable { .. } => code!("unreadable"),
            WorktreeError::GitTimedOut { .. } => code!("git-timed-out"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            WorktreeError::Unadoptable { path, branch, why } => vec![
                ("path", path.as_str().into()),
                ("branch", branch.as_str().into()),
                ("why", why.as_str().into()),
            ],
            WorktreeError::Unreadable { path, why } => {
                vec![("path", path.as_str().into()), ("why", why.as_str().into())]
            }
            WorktreeError::GitTimedOut { command, timeout } => vec![
                ("command", command.as_str().into()),
                ("timeout", timeout.as_str().into()),
            ],
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error this crate defines, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(WorktreeError::Unadoptable {
            path: "/work/ctxloom-wt-a".to_string(),
            branch: "ctxloom/a".to_string(),
            why: "it is not a registered worktree".to_string(),
        }),
        Box::new(WorktreeError::Unreadable {
            path: "/work".to_string(),
            why: "permission denied".to_string(),
        }),
        Box::new(WorktreeError::GitTimedOut {
            command: "worktree".to_string(),
            timeout: "30s".to_string(),
        }),
    ]
}
