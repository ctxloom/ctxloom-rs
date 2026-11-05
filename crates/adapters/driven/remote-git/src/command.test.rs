//! Running git under a time limit: a git that hangs is killed and reaped, and
//! the failure says so.

use std::process::Command;
use std::time::Duration;

use super::{output_within, Failed, Git, Unfinished, DEFAULT_TIMEOUT};
use crate::auth::Credential;
use crate::fixtures::{exists, put_exec, root, runs_scripts};

const TINY: Duration = Duration::from_millis(20);

/// A "git" that is a script, in a directory of its own, run once so it is past
/// the moment another thread's fork may hold it open for writing (ETXTBSY).
pub(crate) fn stub(body: &str) -> (file_system::Rooted, Git) {
    let dir = root();
    let path = dir.root().join("git");
    put_exec(&path, body);
    let git = Git::new(&path, TINY);
    while matches!(git.run(Some(dir.root()), &["warm"], None), Err(why) if why.contains("busy")) {
        std::thread::yield_now();
    }
    (dir, git)
}

pub(crate) const HANGS: &str = "#!/bin/sh\nexec sleep 30\n";

#[test]
fn a_git_that_outlives_its_limit_is_killed_reaped_and_named() {
    if !runs_scripts() {
        return;
    }
    let (dir, git) = stub(HANGS);
    let why = git
        .run(Some(dir.root()), &["fetch", "--prune"], None)
        .unwrap_err();
    assert_eq!(why, "git fetch: did not finish within 20ms and was killed");
}

#[test]
fn the_killed_process_is_reaped_not_left_a_zombie() {
    if !runs_scripts() {
        return;
    }
    let (dir, _) = stub(HANGS);
    let mut command = Command::new(dir.root().join("git"));
    command.stdin(std::process::Stdio::null());
    let Err(Unfinished::TimedOut { pid }) = output_within(&mut command, None, TINY) else {
        panic!("a hung process finished");
    };
    // A reaped process has no /proc entry; a zombie keeps one.
    if exists("/proc/self") {
        assert!(!exists(format!("/proc/{pid}")), "pid {pid} was not reaped");
    }
}

#[test]
fn a_timeout_is_told_apart_from_a_failure() {
    if !runs_scripts() {
        return;
    }
    let (dir, git) = stub(HANGS);
    let none = Credential::none();
    assert!(matches!(
        git.attempt(Some(dir.root()), &["status"], None, &none),
        Err(Failed::TimedOut(_))
    ));
    let (dir, git) = stub("#!/bin/sh\necho no >&2\nexit 1\n");
    assert!(matches!(
        git.attempt(Some(dir.root()), &["status"], None, &none),
        Err(Failed::Said(why)) if why == "git status: no"
    ));
}

#[test]
fn a_git_that_finishes_within_its_limit_is_not_disturbed() {
    if !runs_scripts() {
        return;
    }
    // Output larger than a pipe's buffer, and input fed to it.
    let (dir, _) = stub("#!/bin/sh\ncat\nhead -c 200000 /dev/zero | tr '\\0' x\n");
    let git = Git::new(dir.root().join("git"), Duration::from_secs(60));
    let out = git
        .run(Some(dir.root()), &["big"], Some(b"fed:".as_slice()))
        .unwrap();
    assert_eq!(out.len(), 200_004);
    assert!(out.starts_with(b"fed:"));
}

#[test]
fn a_missing_git_is_named_as_one() {
    let dir = root();
    let git = Git::new(dir.root().join("no-such-git"), DEFAULT_TIMEOUT);
    let why = git.run(Some(dir.root()), &["status"], None).unwrap_err();
    assert!(why.starts_with("the git binary was not found"), "{why}");
}

#[test]
fn the_default_limit_is_gos_sixty_second_sync() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(60));
}

#[test]
fn a_git_that_cannot_be_started_for_another_reason_is_not_called_missing() {
    // A directory is found but cannot be executed.
    let dir = root();
    let git = Git::new(dir.root(), DEFAULT_TIMEOUT);
    let why = git.run(Some(dir.root()), &["status"], None).unwrap_err();
    assert!(why.starts_with("git status: "), "{why}");
    assert!(!why.contains("not found"), "{why}");
}
