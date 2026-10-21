//! Why a lock could not be taken; and this crate's catalog.

use std::fmt;
use std::path::Path;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LockError {
    /// A lock file, or its directory, could not be made or opened.
    Io { path: String, detail: String },
    /// A lock path that is relative, which would land wherever the process
    /// happens to run.
    RelativePath { path: String },
    /// The lock path is something other than a regular file (a FIFO, a
    /// directory): locking it is not locking at all. Go's
    /// `ErrNotRegularFile`.
    NotRegular { path: String },
    /// The lock was held by another process for longer than the wait.
    Timeout { path: String, waited_ms: u64 },
    /// Each lock taken was on a lock file unlinked meanwhile. Go's
    /// `ErrLockReplaced`.
    Replaced { path: String, attempts: u32 },
    /// A reap of the home lock directory held the reap interlock for longer
    /// than the wait. Go's `ErrLockReapInProgress`.
    ReapInProgress { path: String, waited_ms: u64 },
    /// A relative path to guard, and no working directory to resolve it in.
    NoWorkingDirectory { path: String, detail: String },
}

impl LockError {
    pub(crate) fn io(path: &Path, error: impl fmt::Display) -> LockError {
        LockError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    }
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::Io { path, detail } => write!(f, "{path}: {detail}"),
            LockError::RelativePath { path } => write!(
                f,
                "{path} is a relative path; a lock is only ever taken at an absolute one"
            ),
            LockError::NotRegular { path } => write!(
                f,
                "the lock {path} is not a regular file, so it cannot be locked"
            ),
            LockError::Timeout { path, waited_ms } => write!(
                f,
                "the lock {path} is still held after {waited_ms}ms: another ctxloom, taskloom or ltk process holds it; try again once it exits"
            ),
            LockError::Replaced { path, attempts } => write!(
                f,
                "the lock {path} was replaced {attempts} times while being taken: try again"
            ),
            LockError::ReapInProgress { path, waited_ms } => write!(
                f,
                "a reap of the home lock directory (`ctxloom clean --yes`) is in progress: it still held {path} after {waited_ms}ms; let it finish, then retry"
            ),
            LockError::NoWorkingDirectory { path, detail } => write!(
                f,
                "the working directory cannot be read, so {path} has no absolute path to lock: {detail}"
            ),
        }
    }
}

impl std::error::Error for LockError {}

impl Diagnostic for LockError {
    fn code(&self) -> &'static str {
        match self {
            LockError::Io { .. } => "file-lock-io",
            LockError::RelativePath { .. } => "file-lock-relative-path",
            LockError::NotRegular { .. } => "file-lock-not-regular",
            LockError::Timeout { .. } => "file-lock-timeout",
            LockError::Replaced { .. } => "file-lock-replaced",
            LockError::ReapInProgress { .. } => "file-lock-reap-in-progress",
            LockError::NoWorkingDirectory { .. } => "file-lock-no-working-directory",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let s = |v: &String| Arg::from(v.as_str());
        match self {
            LockError::Io { path, detail } | LockError::NoWorkingDirectory { path, detail } => {
                vec![("path", s(path)), ("detail", s(detail))]
            }
            LockError::RelativePath { path } | LockError::NotRegular { path } => {
                vec![("path", s(path))]
            }
            LockError::Timeout { path, waited_ms }
            | LockError::ReapInProgress { path, waited_ms } => {
                vec![("path", s(path)), ("waited_ms", Arg::from(*waited_ms))]
            }
            LockError::Replaced { path, attempts } => {
                vec![("path", s(path)), ("attempts", Arg::from(*attempts))]
            }
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = ("file-lock", include_str!("../locales/en-US/file-lock.ftl"));

/// One value of every error this crate defines, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/u/.ctxloom/locks/u__.claude.json__d07667d34aae78ed.lock".to_string();
    vec![
        Box::new(LockError::Io {
            path: path(),
            detail: "Permission denied (os error 13)".to_string(),
        }),
        Box::new(LockError::RelativePath {
            path: "locks/x.lock".to_string(),
        }),
        Box::new(LockError::NotRegular { path: path() }),
        Box::new(LockError::Timeout {
            path: path(),
            waited_ms: 10_000,
        }),
        Box::new(LockError::Replaced {
            path: path(),
            attempts: 16,
        }),
        Box::new(LockError::ReapInProgress {
            path: "/home/u/.ctxloom/locks/reap.interlock".to_string(),
            waited_ms: 30_000,
        }),
        Box::new(LockError::NoWorkingDirectory {
            path: ".mcp.json".to_string(),
            detail: "No such file or directory (os error 2)".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
