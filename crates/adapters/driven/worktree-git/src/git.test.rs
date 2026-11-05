//! Running git: which variables are scrubbed, and what a failure says.

use std::path::Path;
use std::time::{Duration, Instant};

use super::{environment, is_local, local_variables, Git, GitError, DEFAULT_TIMEOUT};
use crate::fixtures::{env, exists, fs, put, put_exec, root, runs_scripts, slurp};

/// A "git" that is a script, in a directory of its own.
fn stub(body: &str) -> (file_system::Rooted, Git, Duration) {
    let dir = root();
    let path = dir.root().join("git");
    put_exec(&path, body);
    let tiny = Duration::from_millis(20);
    (dir, Git::new(fs(), path, tiny, env()), tiny)
}

fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    Git::on(fs(), env())
        .run(dir, args)
        .map_err(|e| e.to_string())
}

fn names(list: &[&str]) -> Option<Vec<String>> {
    Some(list.iter().map(ToString::to_string).collect())
}

#[test]
fn git_names_its_repository_local_variables() {
    let local = local_variables().as_ref().expect("git answers");
    assert!(local.iter().any(|n| n == "GIT_DIR"), "{local:?}");
}

#[test]
fn a_variable_git_calls_local_is_scrubbed_and_others_are_not() {
    let local = names(&["GIT_DIR"]);
    assert!(is_local("GIT_DIR", &local));
    assert!(!is_local("HOME", &local));
    assert!(!is_local("GIT_SSH", &local), "only what git listed");
}

#[test]
fn when_git_cannot_be_asked_every_git_variable_is_scrubbed() {
    assert!(is_local("GIT_INDEX_FILE", &None));
    assert!(!is_local("HOME", &None));
}

#[test]
fn indexed_config_variables_are_scrubbed_only_with_their_count() {
    let counted = names(&["GIT_CONFIG_COUNT"]);
    assert!(is_local("GIT_CONFIG_KEY_0", &counted));
    assert!(is_local("GIT_CONFIG_VALUE_3", &counted));
    assert!(!is_local("GIT_CONFIG_OTHER", &counted));
    assert!(!is_local("FOO", &counted));
    assert!(!is_local("GIT_CONFIG_KEY_0", &names(&["GIT_DIR"])));
}

#[test]
fn the_environment_keeps_the_callers_and_never_prompts() {
    let given =
        process_env::EnvSnapshot::of([("PATH", "/bin"), ("FOO", "bar"), ("GIT_DIR", "/elsewhere")]);
    let env = environment(&given);
    let has = |k: &str, v: &str| env.iter().any(|(key, val)| key == k && val == v);
    assert!(has("PATH", "/bin"));
    assert!(has("FOO", "bar"));
    assert!(!env.iter().any(|(k, _)| k == "GIT_DIR"));
    assert!(has("GIT_TERMINAL_PROMPT", "0"));
    assert!(has("GIT_ASKPASS", ""));
}

#[test]
fn a_silent_failure_is_named_by_the_command_not_its_flags() {
    let dir = root();
    put(dir.root().join("a"), "1");
    put(dir.root().join("b"), "2");
    let said = run(dir.root(), &["diff", "--no-index", "--quiet", "a", "b"]).unwrap_err();
    assert_eq!(said, "git diff: exit status: 1");
}

#[test]
fn a_directory_that_cannot_be_entered_is_told_apart_from_a_missing_git() {
    let dir = root();
    let file = dir.root().join("file");
    put(&file, "x");
    let said = run(&file, &["status"]).unwrap_err();
    assert!(said.starts_with("git status: "), "{said}");
    let gone = run(&dir.root().join("gone"), &["status"]).unwrap_err();
    assert!(gone.starts_with("git status: "), "{gone}");
    assert!(
        !gone.contains("binary"),
        "a missing directory is not a missing git: {gone}"
    );
    assert!(gone.contains("gone"), "{gone}");
}

#[test]
fn a_missing_git_is_named_as_one_whatever_the_directory() {
    let dir = root();
    let git = Git::new(fs(), dir.root().join("no-such-git"), DEFAULT_TIMEOUT, env());
    let said = git.run(dir.root(), &["status"]).unwrap_err().to_string();
    assert_eq!(said, "the git binary was not found on PATH");
}

// S11 hardening: a git that hangs is killed at its limit, not waited on.
#[test]
fn a_git_that_outlives_its_limit_is_killed_reaped_and_named() {
    if !runs_scripts() {
        return;
    }
    let (dir, git, tiny) = stub("#!/bin/sh\nexec sleep 30\n");
    let error = git.run(dir.root(), &["--no-pager", "status"]).unwrap_err();
    let GitError::TimedOut { command, timeout } = &error else {
        panic!("not a timeout: {error:?}");
    };
    assert_eq!(command, "status", "named by the command, not its flags");
    assert_eq!(*timeout, tiny);
    assert_eq!(
        error.to_string(),
        "git status did not finish within 20ms and was killed"
    );
}

/// Whether the process is gone: no `/proc` entry, or a zombie nobody has
/// reaped yet (an orphan waits for init).
fn gone(pid: &str) -> bool {
    // A process can vanish between the two looks.
    !exists(format!("/proc/{pid}"))
        || fs()
            .read_to_string(Path::new(&format!("/proc/{pid}/stat")))
            .map_or(true, |stat| stat.contains(") Z "))
}

#[test]
fn a_hook_the_git_started_is_killed_with_it() {
    if !runs_scripts() || !exists("/proc/self") {
        return;
    }
    // The hook holds git's pipes, as a real one would.
    let (dir, _, _) = stub("#!/bin/sh\nsleep 30 &\necho $! > \"$(dirname \"$0\")/hook\"\nwait\n");
    // A loaded machine may not have started the hook by the first limit:
    // ask again with a longer one until it has.
    let started = Instant::now();
    for secs in [1, 5, 25] {
        let git = Git::new(
            fs(),
            dir.root().join("git"),
            Duration::from_secs(secs),
            env(),
        );
        let error = git.run(dir.root(), &["checkout"]).unwrap_err();
        assert!(matches!(error, GitError::TimedOut { .. }), "{error:?}");
        if exists(dir.root().join("hook")) {
            break;
        }
    }
    assert!(started.elapsed() < Duration::from_secs(40));
    let pid = slurp(dir.root().join("hook"));
    assert!(gone(pid.trim()), "the hook {pid} outlived the git");
}

#[test]
fn a_hook_that_outlives_a_finished_git_does_not_hold_the_caller() {
    if !runs_scripts() {
        return;
    }
    let (dir, _, _) = stub("#!/bin/sh\nsleep 5 &\necho done\n");
    let git = Git::new(fs(), dir.root().join("git"), Duration::from_secs(30), env())
        .drain_grace(Duration::from_millis(50));
    let started = Instant::now();
    assert_eq!(git.run(dir.root(), &["gc"]).unwrap(), "done\n");
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn a_timeout_is_a_diagnostic_and_a_failure_of_git_is_not() {
    if !runs_scripts() {
        return;
    }
    let (dir, git, _) = stub("#!/bin/sh\nexec sleep 30\n");
    let timed_out = git.run(dir.root(), &["status"]).unwrap_err();
    let diagnostic = timed_out.diagnostic().expect("a diagnostic");
    assert_eq!(
        crate::WorktreeError::GitTimedOut {
            command: "status".to_string(),
            timeout: "20ms".to_string()
        },
        diagnostic
    );
    assert_eq!(GitError::Said("x".to_string()).diagnostic(), None);
}

#[test]
fn a_git_that_finishes_within_its_limit_is_not_disturbed() {
    if !runs_scripts() {
        return;
    }
    // Output larger than a pipe's buffer, then a failure's own words.
    let (dir, _, _) = stub("#!/bin/sh\nif [ \"$1\" = fail ]; then echo oops >&2; exit 3; fi\nhead -c 200000 /dev/zero | tr '\\0' x\n");
    let git = Git::new(fs(), dir.root().join("git"), Duration::from_secs(60), env());
    let out = git.run(dir.root(), &["big"]).unwrap();
    assert_eq!(out.len(), 200_000);
    let said = git.run(dir.root(), &["fail"]).unwrap_err();
    assert_eq!(said, GitError::Said("oops".to_string()));
}

#[test]
fn the_default_limit_is_gos_thirty_seconds() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
}
