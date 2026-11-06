//! Running git under a time limit: a git that hangs is killed and reaped, and
//! the failure says so.

use std::time::{Duration, Instant};

use super::{Failed, Git, DEFAULT_TIMEOUT};
use crate::auth::Credential;
use crate::fixtures::{env, exists, fs, root, runs_scripts, slurp};

const TINY: Duration = Duration::from_millis(20);

/// A "git" that is a script, in a directory of its own, run by `sh` and
/// never exec'd as written (`fixture_program`).
pub(crate) fn stub(body: &str) -> (file_system::Rooted, Git) {
    let dir = root();
    let path = dir.root().join("git");
    fixture_program::install_program(&*fs(), &path, body).unwrap();
    let git = Git::new(&path, TINY, env());
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
fn a_helper_the_git_started_is_killed_with_it() {
    if !runs_scripts() || !exists("/proc/self") {
        return;
    }
    // The helper holds git's pipes, as a hook or a credential helper would.
    let (dir, _) = stub("#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$0\")/helper\"\nwait\n");
    // A loaded machine may not have started the helper by the first limit:
    // ask again with a longer one until it has.
    let started = Instant::now();
    for secs in [1, 5, 25] {
        let git = Git::new(dir.root().join("git"), Duration::from_secs(secs), env());
        let why = git.run(Some(dir.root()), &["fetch"], None).unwrap_err();
        assert!(why.contains("did not finish"), "{why}");
        if exists(dir.root().join("helper")) {
            break;
        }
    }
    assert!(started.elapsed() < Duration::from_secs(40));
    let pid = slurp(dir.root().join("helper"));
    assert!(
        run_within::process_gone(pid.trim()),
        "the helper {pid} outlived the git"
    );
}

#[test]
fn a_helper_that_outlives_a_finished_git_does_not_hold_the_caller() {
    if !runs_scripts() {
        return;
    }
    let (dir, _) = stub("#!/bin/sh\nsleep 5 &\necho fetched\n");
    let git = Git::new(dir.root().join("git"), Duration::from_secs(30), env())
        .drain_grace(Duration::from_millis(50));
    let started = Instant::now();
    let out = git.run(Some(dir.root()), &["fetch"], None).unwrap();
    assert_eq!(out, b"fetched\n");
    assert!(started.elapsed() < Duration::from_secs(4));
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
    let git = Git::new(dir.root().join("git"), Duration::from_secs(60), env());
    let out = git
        .run(Some(dir.root()), &["big"], Some(b"fed:".as_slice()))
        .unwrap();
    assert_eq!(out.len(), 200_004);
    assert!(out.starts_with(b"fed:"));
}

#[test]
fn a_missing_git_is_named_as_one() {
    let dir = root();
    let git = Git::new(dir.root().join("no-such-git"), DEFAULT_TIMEOUT, env());
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
    let git = Git::new(dir.root(), DEFAULT_TIMEOUT, env());
    let why = git.run(Some(dir.root()), &["status"], None).unwrap_err();
    assert!(why.starts_with("git status: "), "{why}");
    assert!(!why.contains("not found"), "{why}");
}
