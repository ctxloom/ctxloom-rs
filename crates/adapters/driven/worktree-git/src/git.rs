//! Running git: the environment scrubbed of the repository-location variables
//! (as remote-git's, copied here because adapters never depend on each other),
//! non-interactive, its own account of a failure kept.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The variables git itself says are local to a repository, asked of git once
/// from the filesystem root with an empty environment. When git cannot be
/// asked, every `GIT_` variable counts.
fn local_variables() -> &'static Option<Vec<String>> {
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

/// `git args` in `dir`: stdout on success, git's own words on failure.
pub(crate) fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let label = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .copied()
        .unwrap_or("");
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(environment())
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "the git binary was not found on PATH".to_string()
            } else {
                format!("git {label}: {e}")
            }
        })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let said = said.trim();
    Err(if said.is_empty() {
        format!("git {label}: {}", output.status)
    } else {
        said.to_string()
    })
}
