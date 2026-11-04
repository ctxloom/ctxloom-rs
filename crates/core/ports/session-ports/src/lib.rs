//! The ports the sessions application needs from outside, beyond taskloom's
//! `ports` (which project a call is about, the clock, the delay, command ids)
//! and angzarr-ports' stores. Each is a trait a driven adapter implements;
//! nothing here does I/O.
//!
//! - [`ProjectStores`]: a project's ctxloom store, opened where its project
//!   says (`~/.ctxloom/projects/<id>/ctxloom.db`, D26).
//! - [`Liveness`]: whether a session's owner is running, asked of its lock.
//! - [`SessionFiles`]: a session's files: its canonical transcript and the
//!   directory around it.
//! - [`GoSessions`]: Go's `~/.ctxloom/sessions/<harp>/session.yaml`, read by
//!   `session import` alone.

use std::future::Future;

use angzarr_diagnostic::Report;
use angzarr_embedded::{Processed, ReadStore, Store, StoreError};
use ports::Project;

/// An instant, as every recorded event carries one.
pub use angzarr_embedded::Timestamp;

/// A project's ctxloom store, opened where its project says.
pub trait ProjectStores {
    /// A project's store: the base ports, one-moment reads, and the
    /// positions it holds, listed (what a catch-up reads when it starts).
    type Store: Store + ReadStore + Processed + Send + Sync;

    /// The project's store, to write: opened (and created) on first use and
    /// held after.
    fn open(
        &mut self,
        project: &Project,
    ) -> impl Future<Output = Result<&Self::Store, StoreError>> + Send;

    /// The project's store, to read: `None` if it has none yet, and nothing
    /// is created. A store already held for a write serves it too.
    fn open_existing(
        &mut self,
        project: &Project,
    ) -> impl Future<Output = Result<Option<&Self::Store>, StoreError>> + Send;
}

/// What a session's liveness lock says: the lock and nothing else, never the
/// session's end stamp (Go: `sessionlock.Inspect`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LockState {
    /// The lock is held by a running process.
    Held,
    /// The lock file is there and nothing holds it: the owner ended.
    Free,
    /// No lock file, an unreadable one, or a filesystem whose locks cannot be
    /// trusted: the lock proves nothing, and is never read as free.
    Unproven,
}

/// A session's liveness, asked of its lock without holding anything.
///
/// Synchronous, and it touches the filesystem, as the project identity does.
pub trait Liveness {
    fn lock_state(&mut self, harp: &str) -> LockState;
}

/// A session's canonical transcript, as it is on disk.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TranscriptFile {
    pub path: String,
    pub bytes: u64,
}

/// One file under a session's directory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileItem {
    /// Absolute path.
    pub path: String,
    /// Relative to the session's directory, slash-separated.
    pub rel: String,
    pub bytes: u64,
}

/// A session's files (D27: the canonical transcript stays a content file,
/// `~/.ctxloom/sessions/<harp>/transcripts/transcript.jsonl`).
///
/// Synchronous: file I/O through the file-system seam.
pub trait SessionFiles {
    /// The canonical transcript of `harp`, when one was captured: a regular
    /// file that stats. A failure to stat is absence, never an invented size.
    fn transcript(&mut self, harp: &str) -> Result<Option<TranscriptFile>, Report>;

    /// The canonical transcript's text, when one was captured.
    fn read_transcript(&mut self, harp: &str) -> Result<Option<String>, Report>;

    /// When `harp`'s CONVERSATION was last written: the newest mtime of a
    /// file under its transcripts and its native history (directories,
    /// symlinks and the record are not the conversation). `None` when neither
    /// holds anything.
    fn last_active(&mut self, harp: &str) -> Result<Option<Timestamp>, Report>;

    /// Every regular file under `harp`'s directory that a purge or remove
    /// could name, sorted by `rel`: the persisted members (transcripts,
    /// native history and anything authored), never the ephemeral ones (the
    /// engine homes, the locks, the worktrees and the scratch).
    fn survey(&mut self, harp: &str) -> Result<Vec<FileItem>, Report>;

    /// Remove the files at `rels` under `harp`'s directory; the bytes freed.
    /// A file already gone is not a failure.
    fn destroy(&mut self, harp: &str, rels: &[String]) -> Result<u64, Report>;

    /// Move a session's output directory from `from` to `to` (a rename
    /// moves it beside itself, to the new name): `false` when there is
    /// nothing at `from` to move. A move, never a copy then a delete, and
    /// refused whole when something is already at `to`.
    fn move_output(&mut self, from: &str, to: &str) -> Result<bool, Report>;
}

/// One of Go's session directories, as `session import` reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GoSession {
    /// The directory's name: the harp.
    pub harp: String,
    /// The sidecar's text (`session.yaml`), or why it could not be read: one
    /// unreadable session must not hide the others.
    pub sidecar: Result<String, Report>,
    /// Whether the directory held a `keep` marker file.
    pub keep_marker: bool,
}

/// Go's session directories under a home, read-only. NEVER the real
/// `~/.ctxloom` in a test.
pub trait GoSessions {
    /// Every session directory that carries a sidecar, by harp.
    fn go_sessions(&mut self) -> Result<Vec<GoSession>, Report>;
}
