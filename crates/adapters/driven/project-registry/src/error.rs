//! Why a project could not be resolved.

use std::fmt;
use std::path::Path;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RegistryError {
    /// HOME is not set, so there is no registry and no store.
    NoHome,
    /// A file could not be read or written.
    Io { path: String, detail: String },
    /// An id that is not a clean path segment: from the flag, the
    /// environment or a marker, any of which a third party can set.
    InvalidId {
        id: String,
        origin: String,
        reason: String,
    },
    /// The registry is not the YAML it should be.
    Unreadable { path: String, detail: String },
    /// Every freshly minted project id was already registered.
    Exhausted { attempts: u32 },
    /// The working directory, needed to find the work root, cannot be read
    /// (it was deleted, say).
    NoWorkingDirectory { detail: String },
    /// A file the registry would write or lock is at a relative path, which
    /// would land wherever the process happens to run.
    RelativePath { path: String },
    /// The registry's lock was held by another process for longer than
    /// the wait.
    LockTimeout { path: String, waited_ms: u64 },
    /// Each lock taken was on a lock file unlinked meanwhile.
    LockReplaced { path: String, attempts: u32 },
    /// The registry, a marker or the projects directory changed between
    /// planning a change and making it.
    Changed,
    /// What a removal would delete is not a database's directory.
    NotADatabase { path: String },
    /// A linked worktree whose git directory is gone: its primary checkout
    /// was deleted or moved.
    StaleWorktree { root: String, git_dir: String },
}

impl RegistryError {
    pub(crate) fn io(path: &Path, error: impl fmt::Display) -> RegistryError {
        RegistryError::Io {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::NoHome => f.write_str("no home directory: set HOME"),
            RegistryError::Io { path, detail } => write!(f, "{path}: {detail}"),
            RegistryError::InvalidId { id, origin, reason } => write!(
                f,
                "the project id {id} from {origin} is not a valid id: {reason}"
            ),
            RegistryError::Unreadable { path, detail } => {
                write!(f, "the project registry {path} cannot be read: {detail}")
            }
            RegistryError::Exhausted { attempts } => {
                write!(f, "no free project id after {attempts} tries")
            }
            RegistryError::NoWorkingDirectory { detail } => {
                write!(f, "the working directory cannot be read: {detail}")
            }
            RegistryError::RelativePath { path } => write!(
                f,
                "{path} is not an absolute path: nothing is written relative to the working directory"
            ),
            RegistryError::LockTimeout { path, waited_ms } => write!(
                f,
                "the project registry's lock {path} is still held after {waited_ms}ms: another ctxloom or taskloom process holds it; try again once it exits"
            ),
            RegistryError::LockReplaced { path, attempts } => write!(
                f,
                "the project registry's lock {path} was replaced {attempts} times while being taken: try again"
            ),
            RegistryError::Changed => f.write_str(
                "the project registry or a marker changed while the change was planned: nothing was changed; run it again",
            ),
            RegistryError::NotADatabase { path } => {
                write!(f, "{path} is not a project's database directory: not removed")
            }
            RegistryError::StaleWorktree { root, git_dir } => write!(
                f,
                "{root} is a linked worktree whose git directory {git_dir} is gone: run `git worktree repair` from its primary checkout, or remove the worktree"
            ),
        }
    }
}

impl Diagnostic for RegistryError {
    fn code(&self) -> &'static str {
        match self {
            RegistryError::NoHome => "project-registry-no-home",
            RegistryError::Io { .. } => "project-registry-io",
            RegistryError::InvalidId { .. } => "project-registry-invalid-id",
            RegistryError::Unreadable { .. } => "project-registry-unreadable",
            RegistryError::Exhausted { .. } => "project-registry-exhausted",
            RegistryError::NoWorkingDirectory { .. } => "project-registry-no-cwd",
            RegistryError::RelativePath { .. } => "project-registry-relative-path",
            RegistryError::LockTimeout { .. } => "project-registry-lock-timeout",
            RegistryError::LockReplaced { .. } => "project-registry-lock-replaced",
            RegistryError::Changed => "project-registry-changed",
            RegistryError::NotADatabase { .. } => "project-registry-not-a-database",
            RegistryError::StaleWorktree { .. } => "project-registry-stale-worktree",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            RegistryError::NoHome | RegistryError::Changed => Vec::new(),
            RegistryError::Io { path, detail } | RegistryError::Unreadable { path, detail } => {
                vec![
                    ("path", path.as_str().into()),
                    ("detail", detail.as_str().into()),
                ]
            }
            RegistryError::InvalidId { id, origin, reason } => vec![
                ("id", id.as_str().into()),
                ("origin", origin.as_str().into()),
                ("reason", reason.as_str().into()),
            ],
            RegistryError::Exhausted { attempts } => vec![("attempts", (*attempts).into())],
            RegistryError::NoWorkingDirectory { detail } => {
                vec![("detail", detail.as_str().into())]
            }
            RegistryError::RelativePath { path } | RegistryError::NotADatabase { path } => {
                vec![("path", path.as_str().into())]
            }
            RegistryError::LockTimeout { path, waited_ms } => vec![
                ("path", path.as_str().into()),
                ("waited_ms", (*waited_ms).into()),
            ],
            RegistryError::StaleWorktree { root, git_dir } => vec![
                ("root", root.as_str().into()),
                ("git_dir", git_dir.as_str().into()),
            ],
            RegistryError::LockReplaced { path, attempts } => vec![
                ("path", path.as_str().into()),
                ("attempts", (*attempts).into()),
            ],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "project-registry",
    include_str!("../locales/en-US/project-registry.ftl"),
);

/// One value of every error, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/u/.ctxloom/projects/index.yaml".to_string();
    let detail = || "permission denied".to_string();
    vec![
        Box::new(RegistryError::NoHome),
        Box::new(RegistryError::Io {
            path: path(),
            detail: detail(),
        }),
        Box::new(RegistryError::InvalidId {
            id: "../x".to_string(),
            origin: "--project".to_string(),
            reason: "it contains '/'".to_string(),
        }),
        Box::new(RegistryError::Unreadable {
            path: path(),
            detail: detail(),
        }),
        Box::new(RegistryError::Exhausted { attempts: 100 }),
        Box::new(RegistryError::NoWorkingDirectory {
            detail: "No such file or directory (os error 2)".to_string(),
        }),
        Box::new(RegistryError::RelativePath {
            path: "index.yaml".to_string(),
        }),
        Box::new(RegistryError::LockTimeout {
            path: "/home/u/.ctxloom/projects/index.yaml.lock".to_string(),
            waited_ms: 10000,
        }),
        Box::new(RegistryError::LockReplaced {
            path: "/home/u/.ctxloom/projects/index.yaml.lock".to_string(),
            attempts: 16,
        }),
        Box::new(RegistryError::Changed),
        Box::new(RegistryError::NotADatabase {
            path: "/home/u/.ctxloom/projects/jolly-mute-magma".to_string(),
        }),
        Box::new(RegistryError::StaleWorktree {
            root: "/work/linked".to_string(),
            git_dir: "/work/main/.git/worktrees/linked".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
