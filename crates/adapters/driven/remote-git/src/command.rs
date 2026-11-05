//! Running git: the environment scrubbed, non-interactive, its own account of
//! a failure kept, and bounded in time: a git that does not finish within its
//! limit is killed with everything it started, and the wait for its output is
//! bounded too (`run-within`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use deps_ports::Why;
use process_env::EnvSnapshot;
use run_within::{run_within, RunError, Spec, Within};

use crate::auth::Credential;

/// How long one git invocation may run. Go bounds its remote sync at sixty
/// seconds (`adapters/cli/run.go`, `syncCtx`); here each invocation (a clone,
/// a fetch, a push) has that long on its own.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long, once a git is over, its output is waited for: a helper it
/// started that outlives it can hold the pipes open.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// The git the remotes run: which program, and how long each invocation has.
/// The seam tests narrow to a stub program and a tiny limit.
#[derive(Clone, Debug)]
pub struct Git {
    program: PathBuf,
    timeout: Duration,
    /// How long, once a git is over, its output is waited for.
    drain_grace: Duration,
    /// The environment git inherits, less the repository-location variables.
    env: Arc<EnvSnapshot>,
}

/// Why a git invocation did not succeed, with git's words.
pub(crate) enum Failed {
    /// It did not finish within its limit.
    TimedOut(Why),
    /// It failed, or could not be run.
    Said(Why),
}

impl Failed {
    pub(crate) fn into_why(self) -> Why {
        match self {
            Failed::TimedOut(why) | Failed::Said(why) => why,
        }
    }
}

/// The variables git itself says are local to a repository: `GIT_DIR` and its
/// siblings, and the numbered config overrides `git -c` exports. Asked of git
/// once, from the filesystem root with an empty environment, so nothing of
/// the caller's can steer the question. When git cannot be asked, every `GIT_`
/// variable counts: the sanitiser errs towards stripping.
pub(crate) fn local_variables() -> &'static Option<Vec<String>> {
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
        (output.status.is_some_and(|s| s.success()) && !names.is_empty()).then_some(names)
    })
}

/// Whether `key` changes which repository git operates on, or how it reads
/// it.
pub(crate) fn is_local(key: &str, local: &Option<Vec<String>>) -> bool {
    let Some(names) = local else {
        return key.starts_with("GIT_");
    };
    if names.iter().any(|n| n == key) {
        return true;
    }
    // The numbered pairs a counter counts are matched by prefix: no fixed
    // list can enumerate them, and git refuses a count whose pairs are gone.
    names.iter().any(|n| n == "GIT_CONFIG_COUNT")
        && (key.starts_with("GIT_CONFIG_KEY_") || key.starts_with("GIT_CONFIG_VALUE_"))
}

/// The environment git is run in: `inherited`, less the repository-location
/// variables, and non-interactive.
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
        ("SSH_ASKPASS".into(), "".into()),
    ]);
    env
}

impl Git {
    /// `program` (a path, or a name looked up on PATH), each invocation
    /// bounded by `timeout`, run in `env`'s environment.
    pub fn new(program: impl Into<PathBuf>, timeout: Duration, env: Arc<EnvSnapshot>) -> Git {
        Git {
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

    /// Run `git args` in `dir` (the process's working directory where none),
    /// feeding `stdin`; its output. A failure is git's own words.
    pub(crate) fn run(
        &self,
        dir: Option<&Path>,
        args: &[&str],
        stdin: Option<&[u8]>,
    ) -> Result<Vec<u8>, Why> {
        self.run_with(dir, args, stdin, &Credential::none())
    }

    /// `run` with `credential` in git's environment, set after the scrub so
    /// the caller's own `GIT_CONFIG_*` never mixes with it; git's words in a
    /// failure have it blotted out.
    pub(crate) fn run_with(
        &self,
        dir: Option<&Path>,
        args: &[&str],
        stdin: Option<&[u8]>,
        credential: &Credential,
    ) -> Result<Vec<u8>, Why> {
        self.attempt(dir, args, stdin, credential)
            .map_err(Failed::into_why)
    }

    /// `run_with`, saying whether a failure was the limit.
    pub(crate) fn attempt(
        &self,
        dir: Option<&Path>,
        args: &[&str],
        stdin: Option<&[u8]>,
        credential: &Credential,
    ) -> Result<Vec<u8>, Failed> {
        let label = args.first().copied().unwrap_or("");
        let mut spec = Spec::new(&self.program)
            .args(args)
            .envs(environment(&self.env))
            .envs(credential.pairs().iter().map(|(k, v)| (k, v)));
        if let Some(dir) = dir {
            spec = spec.current_dir(dir);
        }
        if let Some(bytes) = stdin {
            spec = spec.stdin(bytes);
        }
        let within = Within::new(self.timeout).drain_grace(self.drain_grace);
        let output = run_within(&spec, &within).map_err(|e| match e {
            RunError::NotFound { .. } => Failed::Said(
                "the git binary was not found on PATH (it is required to reach a remote)"
                    .to_string(),
            ),
            other => Failed::Said(format!("git {label}: {}", other.io())),
        })?;
        let Some(status) = output.status else {
            return Err(Failed::TimedOut(format!(
                "git {label}: did not finish within {:?} and was killed",
                self.timeout
            )));
        };
        if status.success() {
            return Ok(output.stdout);
        }
        let said = String::from_utf8_lossy(&output.stderr);
        let said = credential.redact(said.trim());
        Err(Failed::Said(if said.is_empty() {
            format!("git {label}: {status}")
        } else {
            format!("git {label}: {said}")
        }))
    }

    /// `run`, its output as trimmed text.
    pub(crate) fn text(&self, dir: &Path, args: &[&str]) -> Result<String, Why> {
        self.run(Some(dir), args, None)
            .map(|out| String::from_utf8_lossy(&out).trim().to_string())
    }
}

#[cfg(test)]
#[path = "command.test.rs"]
pub(crate) mod tests;
