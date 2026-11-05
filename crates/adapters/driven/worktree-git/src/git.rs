//! Running git: the environment scrubbed of the repository-location variables
//! (as remote-git's, copied here because adapters never depend on each other),
//! non-interactive, its own account of a failure kept, and bounded in time: a
//! git that does not finish within its limit is killed and reaped, never
//! waited on.

use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use file_system::{Fs, Kind};

use crate::error::WorktreeError;

/// How long one git invocation may run: Go's `worktreeTeardownTimeout`,
/// `worktreeReapTimeout` and `upTimeout`, each thirty seconds, which bound
/// the same git calls (`isolation/worktree.go`, `worktree_reap.go`,
/// `operations/launch.go`).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How often a running git is asked whether it has finished.
const POLL: Duration = Duration::from_millis(5);

/// The git a workspace runs: which program, and how long each invocation
/// has. The seam tests narrow to a stub program and a tiny limit.
#[derive(Clone, Debug)]
pub struct Git {
    fs: Arc<dyn Fs>,
    program: PathBuf,
    timeout: Duration,
}

/// Why a git invocation did not succeed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GitError {
    /// Git's own account of its failure, or why it could not be run.
    Said(String),
    /// It did not finish within its limit; it was killed and reaped.
    TimedOut {
        command: String,
        timeout: Duration,
        /// The killed process, for a caller that must show it gone.
        pid: u32,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Said(said) => f.write_str(said),
            GitError::TimedOut {
                command, timeout, ..
            } => write!(
                f,
                "git {command} did not finish within {timeout:?} and was killed"
            ),
        }
    }
}

impl GitError {
    /// The diagnostic of a git that timed out; none for any other failure.
    pub(crate) fn diagnostic(&self) -> Option<WorktreeError> {
        match self {
            GitError::Said(_) => None,
            GitError::TimedOut {
                command, timeout, ..
            } => Some(WorktreeError::GitTimedOut {
                command: command.clone(),
                timeout: format!("{timeout:?}"),
            }),
        }
    }
}

/// `command` run to its end, or killed and reaped at `timeout`.
fn output_within(
    command: &mut Command,
    label: &str,
    timeout: Duration,
) -> Result<Output, GitError> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| GitError::Said(spawn_failure(&e, label)))?;
    let pid = child.id();
    // Read as it comes, so a git with much to say is never stuck on a full
    // pipe while this waits for it to end.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                // The readers end with whatever still holds the pipes (a
                // hook git started outlives the git it was killed with).
                return Err(GitError::TimedOut {
                    command: label.to_string(),
                    timeout,
                    pid,
                });
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GitError::Said(format!("git {label}: {e}")));
            }
        }
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    let mut pipe = pipe;
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = pipe.as_mut() {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

fn spawn_failure(e: &std::io::Error, label: &str) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        "the git binary was not found on PATH".to_string()
    } else {
        format!("git {label}: {e}")
    }
}

/// The variables git itself says are local to a repository, asked of git once
/// from the filesystem root with an empty environment. When git cannot be
/// asked, every `GIT_` variable counts.
fn local_variables() -> &'static Option<Vec<String>> {
    static LOCAL: OnceLock<Option<Vec<String>>> = OnceLock::new();
    LOCAL.get_or_init(|| {
        let top = os::host().flavor().top();
        let output = output_within(
            Command::new("git")
                .args(["rev-parse", "--local-env-vars"])
                .current_dir(top)
                .env_clear()
                .stdin(Stdio::null()),
            "rev-parse",
            DEFAULT_TIMEOUT,
        )
        .ok()?;
        let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        // A git that failed printed no names, so emptiness covers failure.
        (!names.is_empty()).then_some(names)
    })
}

fn is_local(key: &str, local: &Option<Vec<String>>) -> bool {
    let Some(names) = local else {
        return key.starts_with("GIT_");
    };
    if names.iter().any(|n| n == key) {
        return true;
    }
    names.iter().any(|n| n == "GIT_CONFIG_COUNT")
        && (key.starts_with("GIT_CONFIG_KEY_") || key.starts_with("GIT_CONFIG_VALUE_"))
}

fn environment() -> Vec<(OsString, OsString)> {
    let local = local_variables();
    let mut env: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(key, _)| !is_local(&key.to_string_lossy(), local))
        .collect();
    env.extend([
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_ASKPASS".into(), "".into()),
    ]);
    env
}

impl Git {
    /// The git on PATH, each invocation bounded by [`DEFAULT_TIMEOUT`]; `fs`
    /// is the filesystem its working directories are on.
    pub fn on(fs: Arc<dyn Fs>) -> Git {
        Git::new(fs, "git", DEFAULT_TIMEOUT)
    }

    /// `program` (a path, or a name looked up on PATH), each invocation
    /// bounded by `timeout`.
    pub fn new(fs: Arc<dyn Fs>, program: impl Into<PathBuf>, timeout: Duration) -> Git {
        Git {
            fs,
            program: program.into(),
            timeout,
        }
    }

    fn is_dir(&self, dir: &Path) -> bool {
        self.fs
            .metadata(dir)
            .is_ok_and(|meta| meta.kind == Kind::Dir)
    }

    /// `git args` in `dir`: stdout on success, git's own words on failure.
    pub(crate) fn run(&self, dir: &Path, args: &[&str]) -> Result<String, GitError> {
        let label = args
            .iter()
            .find(|a| !a.starts_with('-'))
            .copied()
            .unwrap_or("");
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .current_dir(dir)
            .env_clear()
            .envs(environment())
            .stdin(Stdio::null());
        let output = output_within(&mut command, label, self.timeout).map_err(|e| match e {
            // A spawn that finds nothing is the program missing or the
            // directory missing; only the directory can be checked here.
            GitError::Said(_) if !self.is_dir(dir) => {
                GitError::Said(format!("git {label}: {} is not a directory", dir.display()))
            }
            other => other,
        })?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let said = String::from_utf8_lossy(&output.stderr);
        let said = said.trim();
        Err(GitError::Said(if said.is_empty() {
            format!("git {label}: {}", output.status)
        } else {
            said.to_string()
        }))
    }
}

#[cfg(test)]
#[path = "git.test.rs"]
mod tests;
