//! A session's liveness, asked of its lock. Go: `internal/shared/sessionlock`
//! (release/0.7).
//!
//! The signal is an EXCLUSIVE FILE LOCK that the session-owning process holds
//! on `~/.ctxloom/sessions/<harp>.lock` for as long as it runs, BESIDE the
//! session directory and never inside it. A probe tries the lock:
//!
//! - it acquires it: the owner is DEAD, the kernel having released the lock
//!   however the owner ended (a kill, a crash, power loss): [`LockState::Free`];
//! - it fails: the owner is ALIVE: [`LockState::Held`];
//! - anything else (no lock file, an unreadable one, an error) is
//!   [`LockState::Unproven`], never read as free.
//!
//! A probe never creates a lock file (one it minted would be unlocked and
//! read as free on the next probe), and holds nothing past its answer.

use std::path::PathBuf;
use std::sync::Arc;

use file_system::{Fs, Kind, Open};
use os::LockKind;
use session_ports::{Liveness, LockState};

/// Liveness over the lock files under one home.
pub struct LockFiles {
    fs: Arc<dyn Fs>,
    home: PathBuf,
}

impl LockFiles {
    /// The locks under `home`, read through `fs`.
    pub fn new(fs: Arc<dyn Fs>, home: impl Into<PathBuf>) -> LockFiles {
        LockFiles {
            fs,
            home: home.into(),
        }
    }

    /// Where `harp`'s lock is: `<home>/.ctxloom/sessions/<harp>.lock`.
    pub fn path(&self, harp: &str) -> PathBuf {
        self.home
            .join(".ctxloom")
            .join("sessions")
            .join(format!("{harp}.lock"))
    }
}

impl Liveness for LockFiles {
    fn lock_state(&mut self, harp: &str) -> LockState {
        let path = self.path(harp);
        // Only a regular file is a lock; one a probe cannot see is no proof.
        if !self
            .fs
            .metadata(&path)
            .is_ok_and(|meta| meta.kind == Kind::File)
        {
            return LockState::Unproven;
        }
        // Read-only: a lock needs no write access, and the open never creates.
        let Ok(file) = self.fs.open(&path, Open::Read) else {
            return LockState::Unproven;
        };
        // The lock taken is let go when `file` is dropped, with this answer.
        match self.fs.os().try_lock(&file, LockKind::Exclusive) {
            Ok(true) => LockState::Free,
            Ok(false) => LockState::Held,
            Err(_) => LockState::Unproven,
        }
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
