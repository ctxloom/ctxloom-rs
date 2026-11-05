//! Running git under a time limit: a git that hangs is killed and reaped, and
//! the failure says so.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{output_within, Failed, Git, Unfinished, DEFAULT_TIMEOUT};
use crate::auth::Credential;

const TINY: Duration = Duration::from_millis(20);

/// A "git" that is a script, in a directory of its own, run once so it is past
/// the moment another thread's fork may hold it open for writing (ETXTBSY).
pub(crate) fn stub(body: &str) -> (tempfile::TempDir, Git) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("git");
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let git = Git::new(&path, TINY);
    while matches!(git.run(Some(dir.path()), &["warm"], None), Err(why) if why.contains("busy")) {
        std::thread::yield_now();
    }
    (dir, git)
}

pub(crate) const HANGS: &str = "#!/bin/sh\nexec sleep 30\n";

#[test]
fn a_git_that_outlives_its_limit_is_killed_reaped_and_named() {
    let (dir, git) = stub(HANGS);
    let why = git
        .run(Some(dir.path()), &["fetch", "--prune"], None)
        .unwrap_err();
    assert_eq!(why, "git fetch: did not finish within 20ms and was killed");
}

#[test]
fn the_killed_process_is_reaped_not_left_a_zombie() {
    let (dir, _) = stub(HANGS);
    let mut command = Command::new(dir.path().join("git"));
    command.stdin(std::process::Stdio::null());
    let Err(Unfinished::TimedOut { pid }) = output_within(&mut command, None, TINY) else {
        panic!("a hung process finished");
    };
    // A reaped process has no /proc entry; a zombie keeps one.
    #[cfg(target_os = "linux")]
    assert!(
        !Path::new(&format!("/proc/{pid}")).exists(),
        "pid {pid} was not reaped"
    );
    let _ = pid;
}

#[test]
fn a_timeout_is_told_apart_from_a_failure() {
    let (dir, git) = stub(HANGS);
    let none = Credential::none();
    assert!(matches!(
        git.attempt(Some(dir.path()), &["status"], None, &none),
        Err(Failed::TimedOut(_))
    ));
    let (dir, git) = stub("#!/bin/sh\necho no >&2\nexit 1\n");
    assert!(matches!(
        git.attempt(Some(dir.path()), &["status"], None, &none),
        Err(Failed::Said(why)) if why == "git status: no"
    ));
}

#[test]
fn a_git_that_finishes_within_its_limit_is_not_disturbed() {
    // Output larger than a pipe's buffer, and input fed to it.
    let (dir, _) = stub("#!/bin/sh\ncat\nhead -c 200000 /dev/zero | tr '\\0' x\n");
    let git = Git::new(dir.path().join("git"), Duration::from_secs(60));
    let out = git
        .run(Some(dir.path()), &["big"], Some(b"fed:".as_slice()))
        .unwrap();
    assert_eq!(out.len(), 200_004);
    assert!(out.starts_with(b"fed:"));
}

#[test]
fn a_missing_git_is_named_as_one() {
    let dir = tempfile::tempdir().unwrap();
    let git = Git::new(dir.path().join("no-such-git"), DEFAULT_TIMEOUT);
    let why = git.run(Some(dir.path()), &["status"], None).unwrap_err();
    assert!(why.starts_with("the git binary was not found"), "{why}");
}

#[test]
fn the_default_limit_is_gos_sixty_second_sync() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(60));
}

#[test]
fn a_git_that_cannot_be_started_for_another_reason_is_not_called_missing() {
    // A directory is found but cannot be executed.
    let dir = tempfile::tempdir().unwrap();
    let git = Git::new(dir.path(), DEFAULT_TIMEOUT);
    let why = git.run(Some(dir.path()), &["status"], None).unwrap_err();
    assert!(why.starts_with("git status: "), "{why}");
    assert!(!why.contains("not found"), "{why}");
}
