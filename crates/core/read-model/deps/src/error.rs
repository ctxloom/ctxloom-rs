//! Why a registry, a lockfile or an address could not be read or changed.
//! Every message is in `locales/en-US/messages.ftl`, under the crate's name;
//! the ones that name a remedy have a `-fix` message of their own.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic, Report};

/// This crate's en-US messages (`locales/en-US/messages.ftl`, ids under the
/// crate's name).
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// A registry or lockfile that could not be read or changed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DepsError {
    /// A file that is not the document its kind is: why, as the reader says.
    Unreadable { path: String, cause: Report },
    /// A file written by a newer format than this build reads.
    TooNew {
        path: String,
        kind: String,
        found: u64,
        current: u64,
    },
    /// A file older than the oldest format this build migrates (unstamped
    /// is 0).
    TooOld {
        path: String,
        kind: String,
        found: u64,
        oldest: u64,
    },
    /// A part of a file that is not the shape it should be: where, and what
    /// it should be.
    BadShape {
        path: String,
        key: String,
        detail: String,
    },
    /// A forge whose `type` is neither of the adapters.
    ForgeType { forge: String, found: String },
    /// A forge label that names no configured or built-in forge.
    UnknownForge { label: String },
    /// An address that names no repository.
    NoRepository { url: String, detail: String },
    /// A remote of that name is registered.
    RemoteExists { name: String },
    /// No remote of that name.
    RemoteNotFound { name: String },
    /// Another remote is registered for the same repository.
    SameRepository { existing: String },
    /// A remote renamed to nothing.
    EmptyName,
    /// An edit that names no field.
    NothingToEdit,
    /// A lockfile present with no document in it.
    LockEmpty { path: String },
    /// A lock entry not keyed by its own bundle identity.
    LockKey { path: String, key: String },
    /// A write that would replace locked entries with none.
    LockWouldErase { path: String, count: u64 },
    /// A write over a lockfile that cannot be read.
    LockUnreadable { path: String, detail: String },
}

impl fmt::Display for DepsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl std::error::Error for DepsError {}

impl Diagnostic for DepsError {
    fn code(&self) -> &str {
        match self {
            DepsError::Unreadable { .. } => code!("unreadable"),
            DepsError::TooNew { .. } => code!("too-new"),
            DepsError::TooOld { .. } => code!("too-old"),
            DepsError::BadShape { .. } => code!("bad-shape"),
            DepsError::ForgeType { .. } => code!("forge-type"),
            DepsError::UnknownForge { .. } => code!("unknown-forge"),
            DepsError::NoRepository { .. } => code!("no-repository"),
            DepsError::RemoteExists { .. } => code!("remote-exists"),
            DepsError::RemoteNotFound { .. } => code!("remote-not-found"),
            DepsError::SameRepository { .. } => code!("same-repository"),
            DepsError::EmptyName => code!("empty-name"),
            DepsError::NothingToEdit => code!("nothing-to-edit"),
            DepsError::LockEmpty { .. } => code!("lock-empty"),
            DepsError::LockKey { .. } => code!("lock-key"),
            DepsError::LockWouldErase { .. } => code!("lock-would-erase"),
            DepsError::LockUnreadable { .. } => code!("lock-unreadable"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            DepsError::Unreadable { path, cause } => vec![
                ("path", path.as_str().into()),
                ("cause", Arg::Nested(Box::new(cause.clone()))),
            ],
            DepsError::TooNew {
                path,
                kind,
                found,
                current,
            } => vec![
                ("path", path.as_str().into()),
                ("kind", kind.as_str().into()),
                ("found", (*found).into()),
                ("current", (*current).into()),
            ],
            DepsError::TooOld {
                path,
                kind,
                found,
                oldest,
            } => vec![
                ("path", path.as_str().into()),
                ("kind", kind.as_str().into()),
                ("found", (*found).into()),
                ("oldest", (*oldest).into()),
            ],
            DepsError::BadShape { path, key, detail } => vec![
                ("path", path.as_str().into()),
                ("key", key.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            DepsError::ForgeType { forge, found } => vec![
                ("forge", forge.as_str().into()),
                ("found", found.as_str().into()),
            ],
            DepsError::UnknownForge { label } => vec![("label", label.as_str().into())],
            DepsError::NoRepository { url, detail } => vec![
                ("url", url.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
            DepsError::RemoteExists { name } | DepsError::RemoteNotFound { name } => {
                vec![("name", name.as_str().into())]
            }
            DepsError::SameRepository { existing } => vec![("existing", existing.as_str().into())],
            DepsError::EmptyName | DepsError::NothingToEdit => Vec::new(),
            DepsError::LockEmpty { path } => vec![("path", path.as_str().into())],
            DepsError::LockKey { path, key } => {
                vec![("path", path.as_str().into()), ("key", key.as_str().into())]
            }
            DepsError::LockWouldErase { path, count } => {
                vec![("path", path.as_str().into()), ("count", (*count).into())]
            }
            DepsError::LockUnreadable { path, detail } => vec![
                ("path", path.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
        }
    }
}

/// One value of every error here, for a catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = "/p/.ctxloom/lock.yaml".to_string();
    vec![
        Box::new(DepsError::Unreadable {
            path: path.clone(),
            cause: Report::of(&DepsError::EmptyName),
        }),
        Box::new(DepsError::TooNew {
            path: path.clone(),
            kind: "lockfile".to_string(),
            found: 4,
            current: 3,
        }),
        Box::new(DepsError::TooOld {
            path: path.clone(),
            kind: "remotes".to_string(),
            found: 0,
            oldest: 1,
        }),
        Box::new(DepsError::BadShape {
            path: "/p/.ctxloom/remotes.yaml".to_string(),
            key: "remotes".to_string(),
            detail: "expected a mapping".to_string(),
        }),
        Box::new(DepsError::ForgeType {
            forge: "corp".to_string(),
            found: "svn".to_string(),
        }),
        Box::new(DepsError::UnknownForge {
            label: "no-such-forge".to_string(),
        }),
        Box::new(DepsError::NoRepository {
            url: "bundles".to_string(),
            detail: "a bare word".to_string(),
        }),
        Box::new(DepsError::RemoteExists {
            name: "origin".to_string(),
        }),
        Box::new(DepsError::RemoteNotFound {
            name: "origin".to_string(),
        }),
        Box::new(DepsError::SameRepository {
            existing: "origin".to_string(),
        }),
        Box::new(DepsError::EmptyName),
        Box::new(DepsError::NothingToEdit),
        Box::new(DepsError::LockEmpty { path: path.clone() }),
        Box::new(DepsError::LockKey {
            path: path.clone(),
            key: "demo".to_string(),
        }),
        Box::new(DepsError::LockWouldErase {
            path: path.clone(),
            count: 2,
        }),
        Box::new(DepsError::LockUnreadable {
            path,
            detail: "permission denied".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
