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
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            RegistryError::NoHome => Vec::new(),
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
            RegistryError::RelativePath { path } => vec![("path", path.as_str().into())],
        }
    }
}

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
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
