//! Running git: the environment scrubbed of the repository-location variables
//! (as remote-git's, copied here because adapters never depend on each other),
//! non-interactive, its own account of a failure kept, and bounded in time: a
//! git that does not finish within its limit is killed with everything it
//! started, and the wait for its output is bounded too (`run-within`).

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use file_system::{Fs, Kind};
use process_env::EnvSnapshot;
use run_within::{run_within, RunError, Spec, Within};

use crate::error::WorktreeError;

/// How long one git invocation may run: Go's `worktreeTeardownTimeout`,
/// `worktreeReapTimeout` and `upTimeout`, each thirty seconds, which bound
/// the same git calls (`isolation/worktree.go`, `worktree_reap.go`,
/// `operations/launch.go`).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long, once a git is over, its output is waited for: a hook it started
/// that outlives it can hold the pipes open.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// The git a workspace runs: which program, and how long each invocation
/// has. The seam tests narrow to a stub program and a tiny limit.
#[derive(Clone, Debug)]
pub struct Git {
    fs: Arc<dyn Fs>,
    program: PathBuf,
    timeout: Duration,
    /// How long, once a git is over, its output is waited for.
    drain_grace: Duration,
    /// The environment git inherits, less the repository-location variables.
    env: Arc<EnvSnapshot>,
}

/// Why a git invocation did not succeed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GitError {
    /// Git's own account of its failure, or why it could not be run.
    Said(String),
    /// It did not finish within its limit; it was killed and reaped.
    TimedOut { command: String, timeout: Duration },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Said(said) => f.write_str(said),
            GitError::TimedOut { command, timeout } => write!(
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
            GitError::TimedOut { command, timeout } => Some(WorktreeError::GitTimedOut {
                command: command.clone(),
                timeout: format!("{timeout:?}"),
            }),
        }
    }
}

fn spawn_failure(e: &RunError, label: &str) -> String {
    match e {
        RunError::NotFound { .. } => "the git binary was not found on PATH".to_string(),
        other => format!("git {label}: {}", other.io()),
    }
}

/// The variables git itself says are local to a repository, asked of git once
/// from the filesystem root with an empty environment. When git cannot be
/// asked, every `GIT_` variable counts.
fn local_variables() -> &'static Option<Vec<String>> {
    static LOCAL: OnceLock<Option<Vec<String>>> = OnceLock::new();
    LOCAL.get_or_init(|| {
        let top = os::host().flavor().top();
        let spec = Spec::new("git")
            .args(["rev-parse", "--local-env-vars"])
            .current_dir(top);
        let output = run_within(
            &spec,
            &Within::new(DEFAULT_TIMEOUT).drain_grace(DRAIN_GRACE),
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

fn environment(inherited: &EnvSnapshot) -> Vec<(OsString, OsString)> {
    let local = local_variables();
    let mut env: Vec<(OsString, OsString)> = inherited
        .vars()
        .filter(|(key, _)| !is_local(&key.to_string_lossy(), local))
        .map(|(key, value)| (key.to_os_string(), value.to_os_string()))
        .collect();
    env.extend([
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_ASKPASS".into(), "".into()),
    ]);
    env
}

impl Git {
    /// The git on PATH, each invocation bounded by [`DEFAULT_TIMEOUT`]; `fs`
    /// is the filesystem its working directories are on, `env` the
    /// environment it is run in.
    pub fn on(fs: Arc<dyn Fs>, env: Arc<EnvSnapshot>) -> Git {
        Git::new(fs, "git", DEFAULT_TIMEOUT, env)
    }

    /// `program` (a path, or a name looked up on PATH), each invocation
    /// bounded by `timeout`, run in `env`'s environment.
    pub fn new(
        fs: Arc<dyn Fs>,
        program: impl Into<PathBuf>,
        timeout: Duration,
        env: Arc<EnvSnapshot>,
    ) -> Git {
        Git {
            fs,
            program: program.into(),
            timeout,
            drain_grace: DRAIN_GRACE,
            env,
        }
    }

    /// The same, waiting `grace` for a finished git's output (two seconds
    /// by default).
    pub fn drain_grace(mut self, grace: Duration) -> Git {
        self.drain_grace = grace;
        self
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
        let spec = Spec::new(&self.program)
            .args(args)
            .current_dir(dir)
            .envs(environment(&self.env));
        let within = Within::new(self.timeout).drain_grace(self.drain_grace);
        let output = run_within(&spec, &within).map_err(|e| {
            // A spawn that finds nothing is the program missing or the
            // directory missing; only the directory can be checked here.
            if self.is_dir(dir) {
                GitError::Said(spawn_failure(&e, label))
            } else {
                GitError::Said(format!("git {label}: {} is not a directory", dir.display()))
            }
        })?;
        let Some(status) = output.status else {
            return Err(GitError::TimedOut {
                command: label.to_string(),
                timeout: self.timeout,
            });
        };
        if status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let said = String::from_utf8_lossy(&output.stderr);
        let said = said.trim();
        Err(GitError::Said(if said.is_empty() {
            format!("git {label}: {status}")
        } else {
            said.to_string()
        }))
    }
}

#[cfg(test)]
#[path = "git.test.rs"]
mod tests;
