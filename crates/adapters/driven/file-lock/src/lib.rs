//! ctxloom's advisory file locks, taken as Go ctxloom's `safefs` takes
//! them, so the Rust and Go binaries take turns on one file:
//!
//! - a lock is `flock(2)` on a lock file (shared or exclusive), created
//!   with its directory if missing, refused when it is not a regular file;
//! - once granted it is re-checked against the file now at its path, and
//!   taken again while it is not (Go's `takeCurrent`, [`LOCK_ATTEMPTS`]
//!   tries): `ctxloom clean` reaps lock files, and a lock on an unlinked
//!   file excludes nobody. That re-check is [`LOCK_PROTOCOL`] 2;
//! - it is waited for at most a bounded time, then refused naming the lock
//!   (Go waits without end).
//!
//! [`lock_file`] is Go's `sessions.WithFileLock`: the lock serializing a
//! read-modify-write of a file ctxloom does not own (an agent's
//! configuration), a sidecar under the home ([`path::sidecar_for`]) that
//! records the path it guards, taken under the reap interlock held shared
//! when it is one `ctxloom clean` may reap.

mod error;
pub mod path;

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::time::{Duration, Instant};

pub use error::{diagnostic_samples, LockError, EN_US};

/// The lock-taking protocol every taker here follows, as every
/// ctxloom-family binary reports it (`version --format json`'s
/// `lock_protocol`, Go's `safefs.LockProtocol`): 2 is the re-check, a lock
/// granted on a file no longer at its path taken again. ctxloom removes a
/// lock file only while every running binary reports at least this.
pub const LOCK_PROTOCOL: u32 = 2;

/// How many times a lock granted on a file no longer at its path is taken
/// again before giving up: Go's `lockAttempts`.
pub const LOCK_ATTEMPTS: u32 = 16;

/// How long a taker waits for whoever holds a lock before giving up: Go
/// waits without end, so a process that hangs holding it would stall
/// every other.
pub const LOCK_WAIT: Duration = Duration::from_secs(10);

/// How long a taker waits for a reap of the home lock directory to finish:
/// Go's `ReapInterlockWait`.
pub const REAP_INTERLOCK_WAIT: Duration = Duration::from_secs(30);

/// How often a wait for a lock tries again.
const LOCK_POLL: Duration = Duration::from_millis(20);

/// A lock file's mode, before umask: Go's `lockFileMode`. Not group- or
/// world-writable, since who can open it is who can take it.
#[cfg(unix)]
const LOCK_FILE_MODE: u32 = 0o644;

/// A lock directory's mode, before umask: Go's `lockDirMode`.
#[cfg(unix)]
const LOCK_DIR_MODE: u32 = 0o755;

/// How a lock is held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// One holder, beside nobody.
    Exclusive,
    /// Any number of holders, beside no exclusive one.
    Shared,
}

/// A held lock, released when dropped.
#[derive(Debug)]
pub struct Locked {
    _file: File,
}

/// Take the exclusive lock at `lock_path`, an absolute path, waiting up to
/// `wait` for whoever holds it.
pub fn lock_within(lock_path: &Path, wait: Duration) -> Result<Locked, LockError> {
    lock(
        lock_path,
        Kind::Exclusive,
        None,
        &mut until(lock_path, wait),
    )
}

/// Take the lock at `lock_path`, an absolute path, as `kind`, calling
/// `pause` each time another holds it, until `pause` gives up. A lock file
/// it creates records `guarded`, when given (Go's `LockGuarding`).
pub fn lock(
    lock_path: &Path,
    kind: Kind,
    guarded: Option<&Path>,
    pause: &mut impl FnMut() -> Result<(), LockError>,
) -> Result<Locked, LockError> {
    if !lock_path.is_absolute() {
        return Err(LockError::RelativePath {
            path: lock_path.display().to_string(),
        });
    }
    if let Some(dir) = lock_path.parent() {
        make_dir(dir)?;
    }
    take_current(lock_path, || take_once(lock_path, kind, guarded, pause))
}

/// The lock `take` grants, taken again while what it granted is no longer
/// the file at `lock_path`: `ctxloom clean` reaps lock files, and a lock
/// on a file since unlinked excludes nobody, because the next taker
/// creates a new file at the path and locks that. Go's `takeCurrent`.
pub fn take_current(
    lock_path: &Path,
    mut take: impl FnMut() -> Result<File, LockError>,
) -> Result<Locked, LockError> {
    for _ in 0..LOCK_ATTEMPTS {
        let file = take()?;
        if same_file_at(&file, lock_path) {
            return Ok(Locked { _file: file });
        }
    }
    Err(LockError::Replaced {
        path: lock_path.display().to_string(),
        attempts: LOCK_ATTEMPTS,
    })
}

/// The lock file at `lock_path`, opened (created if need be, recording
/// `guarded` when given) and locked as `kind`, calling `pause` each time
/// another holds it, until `pause` gives up.
pub fn take_once(
    lock_path: &Path,
    kind: Kind,
    guarded: Option<&Path>,
    pause: &mut impl FnMut() -> Result<(), LockError>,
) -> Result<File, LockError> {
    if let Some(guarded) = guarded {
        record(lock_path, guarded)?;
    }
    let file = open(lock_path)?;
    loop {
        let tried = match kind {
            Kind::Exclusive => fs4::FileExt::try_lock(&file),
            Kind::Shared => fs4::FileExt::try_lock_shared(&file),
        };
        match tried {
            Ok(()) => return Ok(file),
            Err(fs4::TryLockError::WouldBlock) => pause()?,
            Err(fs4::TryLockError::Error(e)) => return Err(LockError::io(lock_path, e)),
        }
    }
}

/// A pause between tries for the lock at `lock_path` that sleeps a poll,
/// and gives up once `wait` has passed since it was made.
pub fn until(lock_path: &Path, wait: Duration) -> impl FnMut() -> Result<(), LockError> {
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
            Some(_) => Err(LockError::Timeout {
                path: path.clone(),
                waited_ms: u64::try_from(wait.as_millis()).unwrap_or(u64::MAX),
            }),
        }
    }
}

/// How long [`lock_file`] waits for each lock it takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Waits {
    /// For the sidecar's holder.
    pub lock: Duration,
    /// For a reap of the home lock directory to finish.
    pub reap_interlock: Duration,
}

impl Waits {
    /// [`LOCK_WAIT`] and [`REAP_INTERLOCK_WAIT`].
    pub const DEFAULT: Waits = Waits {
        lock: LOCK_WAIT,
        reap_interlock: REAP_INTERLOCK_WAIT,
    };
}

/// The locks [`lock_file`] holds, released when dropped: the sidecar
/// first, then the reap interlock, as Go releases them.
#[derive(Debug)]
pub struct FileLock {
    _sidecar: Locked,
    _interlock: Option<Locked>,
}

/// Go's `sessions.WithFileLock` under `home`: the lock serializing one
/// read-modify-write of `target`, a file ctxloom does not own. Its sidecar
/// is [`path::sidecar_for`]'s, recording `target`'s absolute path when it
/// creates it; one in the home lock directory is taken with the reap
/// interlock held shared from before the take until after the release, and
/// a reap that holds it past `waits.reap_interlock` refuses the lock
/// ([`LockError::ReapInProgress`]).
pub fn lock_file(home: &Path, target: &Path, waits: Waits) -> Result<FileLock, LockError> {
    let target = path::absolute(target)?;
    let home = path::absolute(home)?;
    let sidecar = path::sidecar_for(&home, &target);
    let interlock = match &sidecar.interlock {
        Some(interlock) => {
            let mut wait = until(interlock, waits.reap_interlock);
            let mut reaping = || {
                wait().map_err(|e| match e {
                    LockError::Timeout { path, waited_ms } => {
                        LockError::ReapInProgress { path, waited_ms }
                    }
                    other => other,
                })
            };
            Some(lock(interlock, Kind::Shared, None, &mut reaping)?)
        }
        None => None,
    };
    let held = lock(
        &sidecar.lock,
        Kind::Exclusive,
        Some(&target),
        &mut until(&sidecar.lock, waits.lock),
    )?;
    Ok(FileLock {
        _sidecar: held,
        _interlock: interlock,
    })
}

/// Create the lock file at `lock_path` recording `guarded` (its path and a
/// newline, Go's `guardRecord`) when none is there; one already there is
/// left exactly as it is. Go's `createRecording`.
fn record(lock_path: &Path, guarded: &Path) -> Result<(), LockError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(LOCK_FILE_MODE);
    }
    let mut file = match options.open(lock_path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => return Ok(()),
        Err(e) => return Err(LockError::io(lock_path, e)),
    };
    let mut line = guarded.as_os_str().as_encoded_bytes().to_vec();
    line.push(b'\n');
    file.write_all(&line)
        .map_err(|e| LockError::io(lock_path, e))
}

/// The lock file at `lock_path`, opened to be locked: read-only when it is
/// there (a lock needs no write access), created when it is not, and
/// refused unless it is a regular file, checked before the open (a FIFO's
/// open would block) and on the open handle.
fn open(lock_path: &Path) -> Result<File, LockError> {
    let not_regular = || LockError::NotRegular {
        path: lock_path.display().to_string(),
    };
    if fs::metadata(lock_path).is_ok_and(|m| !m.is_file()) {
        return Err(not_regular());
    }
    let file = match File::open(lock_path) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(false);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(LOCK_FILE_MODE);
            }
            options.open(lock_path)
        }
        opened => opened,
    }
    .map_err(|e| LockError::io(lock_path, e))?;
    require_regular(&file, lock_path)?;
    Ok(file)
}

/// `file`, the open handle on `lock_path`, refused unless it is a regular
/// file: the path can be swapped between a check and the open. Go's
/// `requireRegular`.
fn require_regular(file: &File, lock_path: &Path) -> Result<(), LockError> {
    let meta = file.metadata().map_err(|e| LockError::io(lock_path, e))?;
    if meta.is_file() {
        Ok(())
    } else {
        Err(LockError::NotRegular {
            path: lock_path.display().to_string(),
        })
    }
}

/// Make `dir` and its parents, traversable and not group- or
/// world-writable.
fn make_dir(dir: &Path) -> Result<(), LockError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(LOCK_DIR_MODE);
    }
    builder.create(dir).map_err(|e| LockError::io(dir, e))
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
#[path = "lib.test.rs"]
mod tests;
