//! The home registry, `~/.ctxloom/projects/index.yaml`: every project id
//! and the path it was registered for, in Go's schema (version 1). Fields
//! this version does not know are kept as read, so Go and Rust can share
//! the file. Changes are made under an exclusive lock on
//! `index.yaml.lock`, the lock file Go's registry takes (taken through
//! file-lock, as every ctxloom lock is), and written by
//! rename, so a reader never sees half a file.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::path::canonical;
use crate::RegistryError;

/// The schema version this registry writes.
pub const SCHEMA_VERSION: u32 = 1;

/// One project: its id and the path it was registered for.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub project_id: String,
    pub path: String,
    /// RFC 3339, as written.
    pub created_at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_seen_at: String,
    /// Fields this version does not know, kept as read.
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_norway::Value>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Index {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub projects: Vec<Entry>,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_norway::Value>,
}

impl Index {
    /// The id registered for `root`, a canonical path, if any: the first,
    /// if several are. Each entry's path is compared canonical too, as Go
    /// compares them.
    pub fn id_for(&self, root: &Path) -> Option<&str> {
        self.projects
            .iter()
            .find(|e| canonical(Path::new(&e.path)) == root)
            .map(|e| e.project_id.as_str())
    }

    /// Whether `id` is registered, for any path.
    pub fn holds(&self, id: &str) -> bool {
        self.projects.iter().any(|e| e.project_id == id)
    }

    /// Register `id` for `path`, seen now.
    pub fn register(&mut self, id: &str, path: &str) {
        let now = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("now formats as RFC 3339");
        self.projects.push(Entry {
            project_id: id.to_string(),
            path: path.to_string(),
            created_at: now.clone(),
            last_seen_at: now,
            other: BTreeMap::new(),
        });
    }
}

/// The registry file under `home`.
pub fn path(home: &Path) -> PathBuf {
    home.join(".ctxloom").join("projects").join("index.yaml")
}

/// The registry at `path`; empty if there is none.
pub fn load(path: &Path) -> Result<Index, RegistryError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
        Err(e) => return Err(RegistryError::io(path, e)),
    };
    if text.trim().is_empty() {
        return Ok(Index::default());
    }
    serde_norway::from_str(&text).map_err(|e| RegistryError::Unreadable {
        path: path.display().to_string(),
        detail: e.to_string(),
    })
}

/// Write `index` to `path`, whole, by rename.
pub fn save(path: &Path, index: &Index) -> Result<(), RegistryError> {
    let mut index = index.clone();
    if index.schema_version == 0 {
        index.schema_version = SCHEMA_VERSION;
    }
    let text = serde_norway::to_string(&index).map_err(|e| RegistryError::io(path, e))?;
    write_atomically(path, text.as_bytes())
}

/// `path`, refused if it is relative: nothing here is written or locked
/// relative to the working directory.
fn absolute(path: &Path) -> Result<&Path, RegistryError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(RegistryError::RelativePath {
            path: path.display().to_string(),
        })
    }
}

/// Write `bytes` to `path`, an absolute path, by renaming a finished
/// temporary file over it, creating the directory if need be.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), RegistryError> {
    let dir = absolute(path)?.parent().unwrap_or(Path::new("/"));
    fs::create_dir_all(dir).map_err(|e| RegistryError::io(dir, e))?;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| RegistryError::io(dir, e))?;
    file.write_all(bytes)
        .and_then(|()| file.as_file().sync_all())
        .map_err(|e| RegistryError::io(path, e))?;
    file.persist(path)
        .map_err(|e| RegistryError::io(path, e.error))?;
    Ok(())
}

/// How long a change waits for whoever holds the registry's lock before
/// giving up (file-lock's wait: Go waits without end).
pub use file_lock::LOCK_WAIT;

/// An exclusive lock on the registry, held until dropped.
pub use file_lock::Locked;

/// Take the lock of the registry at `path`, an absolute path, waiting up
/// to `wait` (in use, [`LOCK_WAIT`]) for whoever holds it: `index.yaml.lock`
/// beside it, taken as every ctxloom lock is (file-lock's re-check and
/// bounded wait).
pub fn lock_within(path: &Path, wait: Duration) -> Result<Locked, RegistryError> {
    let lock_path = absolute(path)?.with_file_name("index.yaml.lock");
    file_lock::lock_within(&lock_path, wait).map_err(|e| from_lock(&lock_path, e))
}

/// A failure to take the lock at `lock_path` as the registry's: a timeout
/// and a replacement keep their own codes, naming the registry's lock;
/// anything else is an I/O failure on the lock file.
fn from_lock(lock_path: &Path, error: file_lock::LockError) -> RegistryError {
    use file_lock::LockError;
    match error {
        LockError::Timeout { path, waited_ms } => RegistryError::LockTimeout { path, waited_ms },
        LockError::Replaced { path, attempts } => RegistryError::LockReplaced { path, attempts },
        LockError::RelativePath { path } => RegistryError::RelativePath { path },
        LockError::Io { path, detail } => RegistryError::Io { path, detail },
        other => RegistryError::io(lock_path, other),
    }
}

#[cfg(test)]
#[path = "index.test.rs"]
mod tests;
