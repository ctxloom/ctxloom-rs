//! Running git: the environment scrubbed, non-interactive, its own account of
//! a failure kept, and bounded in time: a git that does not finish within its
//! limit is killed and reaped, never waited on.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use deps_ports::Why;

use crate::auth::Credential;

/// How long one git invocation may run. Go bounds its remote sync at sixty
/// seconds (`adapters/cli/run.go`, `syncCtx`); here each invocation (a clone,
/// a fetch, a push) has that long on its own.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// How often a running git is asked whether it has finished.
const POLL: Duration = Duration::from_millis(5);

/// The git the remotes run: which program, and how long each invocation has.
/// The seam tests narrow to a stub program and a tiny limit.
#[derive(Clone, Debug)]
pub struct Git {
    program: PathBuf,
    timeout: Duration,
}

impl Default for Git {
    fn default() -> Git {
        Git::new("git", DEFAULT_TIMEOUT)
    }
}

/// Why a run produced no output.
pub(crate) enum Unfinished {
    /// It could not be started or waited on.
    Failed(std::io::Error),
    /// It did not finish within its limit; it was killed and reaped.
    TimedOut {
        /// The killed process, for a test that must show it gone.
        #[cfg(test)]
        pid: u32,
    },
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

/// `command` run to its end with `stdin` fed to it, or killed and reaped at
/// `timeout`.
pub(crate) fn output_within(
    command: &mut Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Output, Unfinished> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(Unfinished::Failed)?;
    #[cfg(test)]
    let pid = child.id();
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // A broken pipe means git stopped reading; its exit says why.
        let _ = pipe.write_all(bytes);
    }
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
                // helper git started outlives the git it was killed with).
                return Err(Unfinished::TimedOut {
                    #[cfg(test)]
                    pid,
                });
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Unfinished::Failed(e));
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

/// The variables git itself says are local to a repository: `GIT_DIR` and its
/// siblings, and the numbered config overrides `git -c` exports. Asked of git
/// once, from the filesystem root with an empty environment, so nothing of
/// the caller's can steer the question. When git cannot be asked, every `GIT_`
/// variable counts: the sanitiser errs towards stripping.
pub(crate) fn local_variables() -> &'static Option<Vec<String>> {
    static LOCAL: OnceLock<Option<Vec<String>>> = OnceLock::new();
    LOCAL.get_or_init(|| {
        let top = os::host().flavor().top();
        let output = output_within(
            Command::new("git")
                .args(["rev-parse", "--local-env-vars"])
                .current_dir(top)
                .env_clear()
                .stdin(Stdio::null()),
            None,
            DEFAULT_TIMEOUT,
        )
        .ok()?;
        let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .map(str::to_string)
            .collect();
        (output.status.success() && !names.is_empty()).then_some(names)
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

/// The environment git is run in: the process's, less the repository-location
/// variables, and non-interactive.
fn environment() -> Vec<(OsString, OsString)> {
    let local = local_variables();
    let mut env: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(key, _)| !is_local(&key.to_string_lossy(), local))
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
    /// bounded by `timeout`.
    pub fn new(program: impl Into<PathBuf>, timeout: Duration) -> Git {
        Git {
            program: program.into(),
            timeout,
        }
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
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .env_clear()
            .envs(environment())
            .envs(credential.pairs().iter().map(|(k, v)| (k, v)))
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        let output = output_within(&mut command, stdin, self.timeout).map_err(|e| match e {
            Unfinished::TimedOut { .. } => Failed::TimedOut(format!(
                "git {label}: did not finish within {:?} and was killed",
                self.timeout
            )),
            Unfinished::Failed(e) if e.kind() == std::io::ErrorKind::NotFound => Failed::Said(
                "the git binary was not found on PATH (it is required to reach a remote)"
                    .to_string(),
            ),
            Unfinished::Failed(e) => Failed::Said(format!("git {label}: {e}")),
        })?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        let said = String::from_utf8_lossy(&output.stderr);
        let said = credential.redact(said.trim());
        Err(Failed::Said(if said.is_empty() {
            format!("git {label}: {}", output.status)
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
