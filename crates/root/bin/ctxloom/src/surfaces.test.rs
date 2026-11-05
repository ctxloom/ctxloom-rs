//! What S9's acceptance tests share (`port/claude-surfaces`): Go's steps for
//! the engines' native files (`steps_cli_manage_shapes.go`, the
//! "registers a SessionStart hook" and "registers no MCP server" steps), a
//! command run with a payload on its standard input, and the files a
//! `materialize` leaves.
//!
//! Go's features run these against a project built by the steps "an
//! initialized ctxloom project", "a bundle ...", "a profile ..."; here that is
//! [`Project`].

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

use super::library::Project;
use super::{binary, Ran};

impl Project {
    /// Go's `I run "<command>" with input:`: `args` with `stdin` on the
    /// standard input, the project named by `CTXLOOM_ROOT` and the working
    /// directory its own, and `vars` in the environment.
    pub(crate) fn run_input(&self, args: &[&str], stdin: &str, vars: &[(&str, &str)]) -> Ran {
        let mut command = Command::new(binary());
        command
            .args(args)
            .env_clear()
            .env("HOME", self.home())
            .env("USERPROFILE", self.home())
            .env("CTXLOOM_ROOT", self.dir())
            .current_dir(self.dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in ["PATH", "SystemRoot"] {
            if let Some(value) = crate::tests::host_var(name) {
                command.env(name, value);
            }
        }
        for (name, value) in vars {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("ctxloom runs");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(stdin.as_bytes())
            .expect("the payload is written");
        Ran::of(child.wait_with_output().expect("ctxloom finishes"))
    }

    /// Go's `the file "<rel>" contains "<text>" exactly <n> times`.
    pub(crate) fn file_counts(&self, rel: &str, text: &str, times: usize) -> &Project {
        let held = self.read(rel).unwrap_or_else(|| panic!("no file {rel}"));
        assert_eq!(
            held.matches(text).count(),
            times,
            "{rel} should contain {text:?} {times} times: {held:?}"
        );
        self
    }

    /// The file at `rel` parsed as JSON.
    pub(crate) fn json_file(&self, rel: &str) -> Value {
        let held = self.read(rel).unwrap_or_else(|| panic!("no file {rel}"));
        serde_json::from_str(&held).unwrap_or_else(|e| panic!("{rel} is not JSON ({e}): {held}"))
    }

    /// Go's `the file "<rel>" registers a SessionStart hook whose command
    /// contains "<needle>"` (any event, any file).
    pub(crate) fn registers_hook(&self, rel: &str, event: &str, needle: &str) -> bool {
        let Some(held) = self.read(rel) else {
            return false;
        };
        let Ok(doc) = serde_json::from_str::<Value>(&held) else {
            return false;
        };
        hook_lines(&doc, event).iter().any(|l| l.contains(needle))
    }

    /// Go's `the file "<rel>" registers no MCP server named "<name>"`: a
    /// registry that is not there registers none.
    pub(crate) fn registers_mcp(&self, rel: &str, key: &str, name: &str) -> bool {
        let Some(held) = self.read(rel) else {
            return false;
        };
        let doc: Value = serde_json::from_str(&held).unwrap_or_else(|e| panic!("{rel}: {e}"));
        doc.get(key).and_then(|servers| servers.get(name)).is_some()
    }
}

/// Each hook under `hooks.<event>` of a claude settings document as one
/// line: its command, then its arguments (ctxloom's own hooks are exec form,
/// the executable and the verb separate strings).
pub(crate) fn hook_lines(doc: &Value, event: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let groups = doc["hooks"][event].as_array().cloned().unwrap_or_default();
    for group in groups {
        for hook in group["hooks"].as_array().cloned().unwrap_or_default() {
            let mut words = vec![hook["command"].as_str().unwrap_or_default().to_string()];
            for arg in hook["args"].as_array().cloned().unwrap_or_default() {
                words.push(arg.as_str().unwrap_or_default().to_string());
            }
            lines.push(words.join(" "));
        }
    }
    lines
}

/// Go's `the hook's additionalContext ...`: the context a claude hook envelope
/// on stdout carries (`hookSpecificOutput.additionalContext`), empty where the
/// hook said nothing. stdout alone is the engine's input.
pub(crate) fn additional_context(ran: &Ran) -> String {
    if ran.stdout.trim().is_empty() {
        return String::new();
    }
    let doc: Value = serde_json::from_str(ran.stdout.trim())
        .unwrap_or_else(|e| panic!("stdout is not one JSON object ({e}): {:?}", ran.stdout));
    doc["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}
