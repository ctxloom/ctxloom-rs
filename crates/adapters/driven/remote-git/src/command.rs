//! Running git: the environment scrubbed, non-interactive, its own account of
//! a failure kept.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use deps_ports::Why;

/// The variables git itself says are local to a repository: `GIT_DIR` and its
/// siblings, and the numbered config overrides `git -c` exports. Asked of git
/// once, from the filesystem root with an empty environment, so nothing of
/// the caller's can steer the question. When git cannot be asked, every `GIT_`
/// variable counts: the sanitiser errs towards stripping.
pub(crate) fn local_variables() -> &'static Option<Vec<String>> {
    static LOCAL: OnceLock<Option<Vec<String>>> = OnceLock::new();
    LOCAL.get_or_init(|| {
        let top = if cfg!(windows) { "C:\\" } else { "/" };
        let output = Command::new("git")
            .args(["rev-parse", "--local-env-vars"])
            .current_dir(top)
            .env_clear()
            .output()
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

/// Run `git args` in `dir` (the process's working directory where none),
/// feeding `stdin`; its output. A failure is git's own words.
pub(crate) fn run(dir: Option<&Path>, args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, Why> {
    let label = args.first().copied().unwrap_or("");
    let mut command = Command::new("git");
    command
        .args(args)
        .env_clear()
        .envs(environment())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "the git binary was not found on PATH (it is required to reach a remote)".to_string()
        } else {
            format!("git {label}: {e}")
        }
    })?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // A broken pipe means git stopped reading; its exit says why.
        let _ = pipe.write_all(bytes);
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("git {label}: {e}"))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let said = said.trim();
    Err(if said.is_empty() {
        format!("git {label}: {}", output.status)
    } else {
        format!("git {label}: {said}")
    })
}

/// `run`, its output as trimmed text.
pub(crate) fn text(dir: &Path, args: &[&str]) -> Result<String, Why> {
    run(Some(dir), args, None).map(|out| String::from_utf8_lossy(&out).trim().to_string())
}
