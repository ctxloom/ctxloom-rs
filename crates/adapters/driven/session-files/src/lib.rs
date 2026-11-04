//! A session's files, through the file-system seam (D27: the canonical
//! transcript stays a content file in Go's format and place,
//! `~/.ctxloom/sessions/<harp>/transcripts/transcript.jsonl`).
//!
//! - the transcript is a regular file that stats: a stat that fails for any
//!   reason is absence, "never an invented size";
//! - the conversation's last activity is the newest file mtime under the
//!   transcripts and the native history: not a directory's own, not a
//!   symlink's, not the record's (Go: `TranscriptActivity`);
//! - the survey names every file under the persisted members, never the
//!   ephemeral ones (the engine homes, the locks, the worktrees, the
//!   scratch): a purge runs no git, so a worktree is untouched by
//!   construction, not by a filter applied after;
//! - removal stays inside the session's directory.

mod error;

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use angzarr_diagnostic::Report;
use file_system::{Fs, Kind};
use os::Mode;
use session_ports::{FileItem, GoSession, GoSessions, SessionFiles, Timestamp, TranscriptFile};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{FilesError, EN_US};

/// Members of a session directory a purge never walks (Go: the Ephemeral
/// rows of `paths.HarpMembers`).
const EPHEMERAL: [&str; 4] = ["home", "locks", "work", "scratch"];

/// Go's sidecar, the identity member: not a file a purge could name.
const SIDECAR: &str = "session.yaml";

/// The members that hold the conversation.
const CONVERSATION: [&str; 2] = ["transcripts", "native"];

/// The sessions' files under one home.
pub struct SessionDirs {
    fs: Arc<dyn Fs>,
    home: PathBuf,
}

impl SessionDirs {
    /// The session directories under `home`, read through `fs`.
    pub fn new(fs: Arc<dyn Fs>, home: impl Into<PathBuf>) -> SessionDirs {
        SessionDirs {
            fs,
            home: home.into(),
        }
    }

    /// `<home>/.ctxloom/sessions/<harp>`.
    pub fn dir(&self, harp: &str) -> PathBuf {
        self.home.join(".ctxloom").join("sessions").join(harp)
    }

    /// `<dir>/transcripts/transcript.jsonl`.
    pub fn transcript_path(&self, harp: &str) -> PathBuf {
        self.dir(harp).join("transcripts").join("transcript.jsonl")
    }
}

impl SessionDirs {
    /// The newest file mtime under `dir`: directories recursed into, symlinks
    /// and anything unreadable not counted.
    fn newest_under(&self, dir: &Path, newest: &mut Option<Timestamp>) {
        let Ok(entries) = self.fs.read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = dir.join(&entry.name);
            match entry.kind {
                Kind::Dir => self.newest_under(&path, newest),
                Kind::File => {
                    let modified = self.fs.symlink_metadata(&path).ok().map(|m| m.modified);
                    if let Some(at) = modified.and_then(timestamp) {
                        if newest.is_none_or(|n| (at.seconds, at.nanos) > (n.seconds, n.nanos)) {
                            *newest = Some(at);
                        }
                    }
                }
                Kind::Symlink | Kind::Other => {}
            }
        }
    }

    /// The files under `dir`, relative to `root`, appended to `found`.
    fn files_under(
        &self,
        dir: &Path,
        root: &Path,
        found: &mut Vec<FileItem>,
    ) -> Result<(), Report> {
        let entries = match self.fs.read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(Report::of(&FilesError::io("read", dir, &e))),
        };
        for entry in entries {
            let path = dir.join(&entry.name);
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let top_level = dir == root;
            match entry.kind {
                Kind::Dir if top_level && EPHEMERAL.contains(&rel.as_str()) => {}
                Kind::Dir => self.files_under(&path, root, found)?,
                Kind::File if top_level && rel == SIDECAR => {}
                Kind::File => {
                    let bytes = self
                        .fs
                        .symlink_metadata(&path)
                        .map_err(|e| Report::of(&FilesError::io("stat", &path, &e)))?
                        .len;
                    found.push(FileItem {
                        path: path.display().to_string(),
                        rel,
                        bytes,
                    });
                }
                Kind::Symlink | Kind::Other => {}
            }
        }
        Ok(())
    }
}

impl SessionFiles for SessionDirs {
    fn transcript(&mut self, harp: &str) -> Result<Option<TranscriptFile>, Report> {
        let path = self.transcript_path(harp);
        Ok(match self.fs.metadata(&path) {
            Ok(meta) if meta.kind == Kind::File => Some(TranscriptFile {
                path: path.display().to_string(),
                bytes: meta.len,
            }),
            _ => None,
        })
    }

    fn read_transcript(&mut self, harp: &str) -> Result<Option<String>, Report> {
        if self.transcript(harp)?.is_none() {
            return Ok(None);
        }
        let path = self.transcript_path(harp);
        self.fs
            .read_to_string(&path)
            .map(Some)
            .map_err(|e| Report::of(&FilesError::io("read", &path, &e)))
    }

    fn last_active(&mut self, harp: &str) -> Result<Option<Timestamp>, Report> {
        let mut newest = None;
        for member in CONVERSATION {
            self.newest_under(&self.dir(harp).join(member), &mut newest);
        }
        Ok(newest)
    }

    fn survey(&mut self, harp: &str) -> Result<Vec<FileItem>, Report> {
        let root = self.dir(harp);
        let mut found = Vec::new();
        self.files_under(&root, &root, &mut found)?;
        found.sort_by(|a, b| a.rel.cmp(&b.rel));
        Ok(found)
    }

    fn destroy(&mut self, harp: &str, rels: &[String]) -> Result<u64, Report> {
        let root = self.dir(harp);
        // Every path is checked before any is removed: a refusal removes
        // nothing.
        for rel in rels {
            let leaves = Path::new(rel).is_absolute()
                || Path::new(rel)
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)));
            if leaves {
                return Err(Report::of(&FilesError::Outside { rel: rel.clone() }));
            }
        }
        let mut freed = 0;
        for rel in rels {
            let path = root.join(rel);
            match self.fs.symlink_metadata(&path) {
                Ok(meta) if meta.kind == Kind::File => {
                    self.fs
                        .remove_file(&path)
                        .map_err(|e| Report::of(&FilesError::io("remove", &path, &e)))?;
                    freed += meta.len;
                }
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(Report::of(&FilesError::io("stat", &path, &e))),
            }
        }
        Ok(freed)
    }

    fn move_output(&mut self, from: &str, to: &str) -> Result<bool, Report> {
        let (from, to) = (Path::new(from), Path::new(to));
        match self.fs.symlink_metadata(from) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(Report::of(&FilesError::io("stat", from, &e))),
        }
        if self.fs.symlink_metadata(to).is_ok() {
            return Err(Report::of(&FilesError::OutputExists {
                path: to.display().to_string(),
            }));
        }
        if let Some(parent) = to.parent() {
            self.fs
                .create_dir_all(parent, Mode::from_bits(0o755))
                .map_err(|e| Report::of(&FilesError::io("create", parent, &e)))?;
        }
        self.fs
            .rename(from, to)
            .map_err(|e| Report::of(&FilesError::io("move", from, &e)))?;
        Ok(true)
    }
}

/// `at` as an instant; `None` for one before the epoch.
fn timestamp(at: SystemTime) -> Option<Timestamp> {
    let since = at.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(Timestamp {
        seconds: i64::try_from(since.as_secs()).ok()?,
        nanos: i32::try_from(since.subsec_nanos()).ok()?,
    })
}

/// Go's session directories under one home, read-only: every directory of
/// `~/.ctxloom/sessions` that Go calls a session (`IsSessionDir`): a real
/// directory (a symlink is none, "a sweep that followed one would act on
/// whatever it points at"), whose name is a usable harp, carrying a regular
/// `session.yaml`. A directory without one is not a session: what `session
/// remove` leaves behind on purpose, or an engine-created directory nobody
/// recorded.
pub struct GoDirs {
    fs: Arc<dyn Fs>,
    home: PathBuf,
}

impl GoDirs {
    /// Go's session directories under `home`, read through `fs`.
    pub fn new(fs: Arc<dyn Fs>, home: impl Into<PathBuf>) -> GoDirs {
        GoDirs {
            fs,
            home: home.into(),
        }
    }

    fn root(&self) -> PathBuf {
        self.home.join(".ctxloom").join("sessions")
    }
}

impl GoSessions for GoDirs {
    fn go_sessions(&mut self) -> Result<Vec<GoSession>, Report> {
        let root = self.root();
        let entries = match self.fs.read_dir(&root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Report::of(&FilesError::io("read", &root, &e))),
        };
        let mut found = Vec::new();
        for entry in entries {
            let Some(harp) = entry.name.to_str().map(str::to_string) else {
                continue;
            };
            if entry.kind != Kind::Dir || sessions::validate(&harp).is_err() {
                continue;
            }
            let dir = root.join(&harp);
            let sidecar = dir.join(SIDECAR);
            let regular = self
                .fs
                .symlink_metadata(&sidecar)
                .is_ok_and(|meta| meta.kind == Kind::File);
            if !regular {
                continue;
            }
            let keep_marker = self
                .fs
                .symlink_metadata(&dir.join("keep"))
                .is_ok_and(|meta| meta.kind == Kind::File);
            found.push(GoSession {
                harp,
                sidecar: self
                    .fs
                    .read_to_string(&sidecar)
                    .map_err(|e| Report::of(&FilesError::io("read", &sidecar, &e))),
                keep_marker,
            });
        }
        found.sort_by(|a, b| a.harp.cmp(&b.harp));
        Ok(found)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
