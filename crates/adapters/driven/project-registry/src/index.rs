//! The home registry, `~/.ctxloom/projects/index.yaml`: every project id
//! and the path it was registered for, in Go's schema (version 1). Fields
//! this version does not know are kept as read, so Go and Rust can share
//! the file. Changes are made under an exclusive lock on
//! `index.yaml.lock`, the lock file Go's registry takes, and written by
//! rename, so a reader never sees half a file.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
/// giving up: Go waits without end, so a process that hangs holding it
/// would stall every other.
pub const LOCK_WAIT: Duration = Duration::from_secs(10);

/// How often a wait for the lock tries again.
const LOCK_POLL: Duration = Duration::from_millis(20);

/// How many times a lock granted on a file no longer at its path is taken
/// again before giving up: Go's `lockAttempts`.
pub const LOCK_ATTEMPTS: u32 = 16;

/// The lock-taking protocol [`take_current`] follows, as every
/// ctxloom-family binary reports it (`version --format json`'s
/// `lock_protocol`, Go's `safefs.LockProtocol`): 2 is the re-check, a
/// lock granted on a file no longer at its path taken again. ctxloom
/// removes a lock file only while every running binary reports at least
/// this.
pub const LOCK_PROTOCOL: u32 = 2;

/// An exclusive lock on the registry at `path`, held until dropped.
pub struct Locked {
    _file: File,
}

/// Take the lock of the registry at `path`, an absolute path, waiting up
/// to `wait` (in use, [`LOCK_WAIT`]) for whoever holds it.
pub fn lock_within(path: &Path, wait: Duration) -> Result<Locked, RegistryError> {
    let lock_path = absolute(path)?.with_file_name("index.yaml.lock");
    if let Some(dir) = lock_path.parent() {
        fs::create_dir_all(dir).map_err(|e| RegistryError::io(dir, e))?;
    }
    let mut pause = until(&lock_path, wait);
    take_current(&lock_path, || take_once(&lock_path, &mut pause))
}

/// The lock `take` grants, taken again while what it granted is no longer
/// the file at `lock_path`: `ctxloom clean` reaps lock files, and a lock
/// on a file since unlinked excludes nobody, because the next taker
/// creates a new file at the path and locks that. Go's `takeCurrent`.
pub(crate) fn take_current(
    lock_path: &Path,
    mut take: impl FnMut() -> Result<File, RegistryError>,
) -> Result<Locked, RegistryError> {
    for _ in 0..LOCK_ATTEMPTS {
        let file = take()?;
        if same_file_at(&file, lock_path) {
            return Ok(Locked { _file: file });
        }
    }
    Err(RegistryError::LockReplaced {
        path: lock_path.display().to_string(),
        attempts: LOCK_ATTEMPTS,
    })
}

/// The lock file at `lock_path`, opened (created if need be) and locked,
/// calling `pause` each time another holds it, until `pause` gives up.
pub(crate) fn take_once(
    lock_path: &Path,
    pause: &mut impl FnMut() -> Result<(), RegistryError>,
) -> Result<File, RegistryError> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
        .map_err(|e| RegistryError::io(lock_path, e))?;
    loop {
        match fs4::FileExt::try_lock(&file) {
            Ok(()) => return Ok(file),
            Err(fs4::TryLockError::WouldBlock) => pause()?,
            Err(fs4::TryLockError::Error(e)) => return Err(RegistryError::io(lock_path, e)),
        }
    }
}

/// A pause between tries for the lock at `lock_path` that sleeps a poll,
/// and gives up once `wait` has passed since it was made.
pub(crate) fn until(lock_path: &Path, wait: Duration) -> impl FnMut() -> Result<(), RegistryError> {
    let deadline = Instant::now() + wait;
    let path = lock_path.display().to_string();
    move || {
        let now = Instant::now();
        // None exactly while the deadline is still ahead.
        match now.checked_duration_since(deadline) {
            None => {
                std::thread::sleep((deadline - now).min(LOCK_POLL));
                Ok(())
            }
            Some(_) => Err(RegistryError::LockTimeout {
                path: path.clone(),
                waited_ms: u64::try_from(wait.as_millis()).unwrap_or(u64::MAX),
            }),
        }
    }
}

/// Whether `file` is the file at `path` now.
#[cfg(unix)]
fn same_file_at(file: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (file.metadata(), fs::metadata(path)) {
        (Ok(held), Ok(on_disk)) => held.dev() == on_disk.dev() && held.ino() == on_disk.ino(),
        _ => false,
    }
}

/// Whether `file` is the file at `path` now: elsewhere an open file cannot
/// be unlinked, so it is.
#[cfg(not(unix))]
// Not compiled on unix, where the mutation tests run.
#[cfg_attr(test, mutants::skip)]
fn same_file_at(_file: &File, path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
#[path = "index.test.rs"]
mod tests;
