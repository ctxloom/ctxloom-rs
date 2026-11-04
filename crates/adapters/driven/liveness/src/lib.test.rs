//! A session's state is the lock's verdict. Go: sessionlock_test.go and
//! session.feature ("The listing takes each session's state from its lock,
//! not its end stamp": held is live, free is ended, no lock file is unknown).

use std::path::Path;
use std::sync::Arc;

use file_system::{Fs, Open, Rooted};
use os::{LockKind, Mode};
use session_ports::{Liveness, LockState};

use super::LockFiles;

const HOME: &str = "/home/u";
const HARP: &str = "amber-swift-owl";

fn locks() -> (Arc<Rooted>, LockFiles) {
    let fs = Arc::new(Rooted::temp().unwrap());
    (fs.clone(), LockFiles::new(fs, HOME))
}

/// The lock file of `harp`, made (as a session that ran once made it).
fn made(fs: &Rooted, locks: &LockFiles, harp: &str) -> std::path::PathBuf {
    let path = locks.path(harp);
    fs.create_dir_all(path.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    fs.open(&path, Open::Create(Mode::from_bits(0o600)))
        .unwrap();
    path
}

// The lock is beside the session directory, never inside it: on Windows an
// open handle blocks deletion, so one inside would stop the sweep holding it
// from removing the directory.
#[test]
fn the_lock_sits_beside_the_session_directory() {
    let (_, locks) = locks();
    assert_eq!(
        locks.path(HARP),
        Path::new("/home/u/.ctxloom/sessions/amber-swift-owl.lock")
    );
}

// session.feature: "a finished session ... with a transcript": the lock file
// is there and free.
#[test]
fn a_lock_nothing_holds_proves_the_owner_ended() {
    let (fs, mut locks) = locks();
    made(&fs, &locks, HARP);
    assert_eq!(locks.lock_state(HARP), LockState::Free);
}

// session.feature: "a session ... that is running again": a held lock is live
// whatever the index says.
#[test]
fn a_lock_a_running_process_holds_is_live() {
    let (fs, mut locks) = locks();
    let path = made(&fs, &locks, HARP);
    let owner = fs.open(&path, Open::Read).unwrap();
    assert!(fs.os().try_lock(&owner, LockKind::Exclusive).unwrap());
    assert_eq!(locks.lock_state(HARP), LockState::Held);
    drop(owner);
    assert_eq!(
        locks.lock_state(HARP),
        LockState::Free,
        "the owner ended, and the kernel let the lock go"
    );
}

// session.feature: "a finished session ... with a transcript but no session
// lock": "cannot determine" is never permission.
#[test]
fn no_lock_file_proves_nothing_and_a_probe_makes_none() {
    let (fs, mut locks) = locks();
    assert_eq!(locks.lock_state(HARP), LockState::Unproven);
    assert_eq!(locks.lock_state(HARP), LockState::Unproven);
    assert!(
        !fs.exists(&locks.path(HARP)).unwrap(),
        "a lock file the probe made would read as free on the next"
    );
}

// Go: only a regular file is a lock.
#[test]
fn a_directory_where_the_lock_should_be_proves_nothing() {
    let (fs, mut locks) = locks();
    fs.create_dir_all(&locks.path(HARP), Mode::from_bits(0o755))
        .unwrap();
    assert_eq!(locks.lock_state(HARP), LockState::Unproven);
}

// A probe holds nothing past its answer: a session starting right after it
// can take the lock.
#[test]
fn a_probe_holds_nothing_after_it_answers() {
    let (fs, mut locks) = locks();
    let path = made(&fs, &locks, HARP);
    assert_eq!(locks.lock_state(HARP), LockState::Free);
    let owner = fs.open(&path, Open::Read).unwrap();
    assert!(
        fs.os().try_lock(&owner, LockKind::Exclusive).unwrap(),
        "the probe let go"
    );
}

// Harps are separate: one session's held lock says nothing of another's.
#[test]
fn each_harp_has_its_own_lock() {
    let (fs, mut locks) = locks();
    let a = made(&fs, &locks, "amber-swift-owl");
    made(&fs, &locks, "brisk-copper-moth");
    let owner = fs.open(&a, Open::Read).unwrap();
    assert!(fs.os().try_lock(&owner, LockKind::Exclusive).unwrap());
    assert_eq!(locks.lock_state("amber-swift-owl"), LockState::Held);
    assert_eq!(locks.lock_state("brisk-copper-moth"), LockState::Free);
    assert_eq!(locks.lock_state("calm-dusty-wren"), LockState::Unproven);
}
