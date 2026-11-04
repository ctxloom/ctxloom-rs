//! Acceptance: the hidden `hook` commands, through the real binary. Ported
//! from Go's tests/acceptance/features/cli/session_hooks.feature
//! (release/0.7): each scenario is named in a comment.
//!
//! These cover the machine contract: a hook warns and exits 0 on what it can
//! foresee, and stdout is the engine's input, so the stdout assertions parse
//! stdout ALONE, never the combined stream.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{binary, Env, Ran};

impl Env {
    /// Run `ctxloom args` with `stdin` on its standard input and `vars` in its
    /// environment (this home, and nothing else of the caller's).
    pub(crate) fn run_hook(&self, args: &[&str], stdin: &str, vars: &[(&str, &str)]) -> Ran {
        let mut command = Command::new(binary());
        command
            .args(args)
            .env_clear()
            .env("HOME", self.home())
            .env("USERPROFILE", self.home())
            .current_dir(self.home())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in ["PATH", "SystemRoot"] {
            if let Some(value) = std::env::var_os(name) {
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
}

/// Every path under `dir`, relative to it, sorted.
fn tree(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, root: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            found.push(
                path.strip_prefix(root)
                    .expect("under the root")
                    .to_path_buf(),
            );
            if path.is_dir() {
                walk(&path, root, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

const HUD_PAYLOAD: &str =
    r#"{"model":{"display_name":"Opus 5"},"context_window":{"used_percentage":42}}"#;

// session_hooks.feature: "The statusline reports the engine's session and
// ctxloom's own": a HUD that rendered only its own half would still look
// plausible.
#[test]
fn test_hud_reports_the_engines_half_and_ctxloom_s_own() {
    let env = Env::new();
    let ran = env.run_hook(
        &["hook", "hud"],
        HUD_PAYLOAD,
        &[("CTXLOOM_SESSION_HARP", "brisk-copper-moth")],
    );
    assert_eq!(ran.code, 0, "{}", ran.said());
    for want in ["Opus 5", "42%", "brisk-copper-moth"] {
        assert!(ran.stdout.contains(want), "{want:?} in {}", ran.said());
    }
    assert_eq!(
        ran.stderr, "",
        "a statusline that printed a diagnostic would corrupt the bar"
    );
}

// Go's line, on the wire: no trailing newline (fmt.Print), the glyph and the
// separator.
#[test]
fn test_hud_prints_gos_line_without_a_newline() {
    let env = Env::new();
    let ran = env.run_hook(
        &["hook", "hud"],
        HUD_PAYLOAD,
        &[("CTXLOOM_SESSION_HARP", "brisk-copper-moth")],
    );
    assert!(
        ran.stdout
            .starts_with("\u{1b}[36mOpus 5\u{1b}[0m \u{2502} "),
        "{:?}",
        ran.stdout
    );
    assert!(
        ran.stdout.contains("\u{2301} brisk-copper-moth"),
        "{:?}",
        ran.stdout
    );
    assert!(!ran.stdout.ends_with('\n'), "{:?}", ran.stdout);
}

// Go: runHookHud "Fault tolerant: output minimal HUD on parse error".
#[test]
fn test_hud_on_garbage_is_the_sentinel_and_exit_0() {
    let env = Env::new();
    let ran = env.run_hook(&["hook", "hud"], "not json", &[]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.stdout, "ctxloom");
}

// D5 (2026-11-02): the HUD hook records no context sample. Go wrote
// context-metrics.jsonl under the session's directory from here.
#[test]
fn test_hud_records_nothing() {
    let env = Env::new();
    let before = tree(env.home());
    let ran = env.run_hook(
        &["hook", "hud"],
        HUD_PAYLOAD,
        &[("CTXLOOM_SESSION_HARP", "brisk-copper-moth")],
    );
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(tree(env.home()), before, "the home is as it was");
}

// session_hooks.feature: "With no active harp the hook stays silent on stdout
// and warns instead".
#[test]
fn test_session_bind_with_no_harp_is_silent_on_stdout_and_warns() {
    let env = Env::new();
    let ran = env.run_hook(
        &["hook", "session-bind", "--engine", "claude-code"],
        r#"{"session_id":"vendor-session-42","hook_event_name":"SessionStart"}"#,
        &[],
    );
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.stdout, "", "nothing truthful to emit");
    assert!(ran.stderr.contains("no usable harp"), "{}", ran.said());
}

// The warning rides the diagnostic channel in the format the process chose:
// an explicit json format makes it an envelope, still on stderr.
#[test]
fn test_session_bind_warning_is_an_envelope_under_json() {
    let env = Env::new();
    let ran = env.run_hook(
        &[
            "--format",
            "json",
            "hook",
            "session-bind",
            "--engine",
            "claude-code",
        ],
        "{}",
        &[],
    );
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.stdout, "");
    assert_eq!(ran.err_json()["code"], "hook-cli-no-usable-harp");
}

// Go: UnknownHookEngineError: refused, not rounded to the default.
#[test]
fn test_session_bind_with_an_unknown_engine_is_refused() {
    let env = Env::new();
    let ran = env.run_hook(
        &[
            "--format",
            "json",
            "hook",
            "session-bind",
            "--engine",
            "no-such-engine",
        ],
        "{}",
        &[("CTXLOOM_SESSION_HARP", "brisk-copper-moth")],
    );
    assert_ne!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.stdout, "");
    assert_eq!(ran.err_json()["code"], "hook-cli-unknown-engine");
}

// session_hooks.feature: "The retired turn-changed callback is not a command".
#[test]
fn test_the_retired_turn_changed_hook_is_not_a_command() {
    let env = Env::new();
    let ran = env.run_hook(
        &["hook", "turn-changed"],
        r#"{"hook_event_name":"Stop"}"#,
        &[],
    );
    assert_eq!(ran.code, 2, "{}", ran.said());
}

// Go: the hook namespace is hidden ("Machine callback ... not for direct
// use"), but answers -h.
#[test]
fn test_hook_is_hidden_from_the_root_help_and_answers_help() {
    let env = Env::new();
    let root = env.run(&["-h"]);
    assert_eq!(root.code, 0, "{}", root.said());
    assert!(
        !root
            .stdout
            .lines()
            .any(|l| l.trim_start().starts_with("hook")),
        "{}",
        root.stdout
    );
    let hook = env.run(&["hook", "-h"]);
    assert_eq!(hook.code, 0, "{}", hook.said());
    assert!(hook.stdout.contains("session-bind"), "{}", hook.stdout);
    assert!(hook.stdout.contains("hud"), "{}", hook.stdout);
}
