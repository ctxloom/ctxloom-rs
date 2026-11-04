//! `hook session-bind`: the sole path for recording the harp-to-engine-session
//! mapping. Claude Code (and other engines with SessionStart hooks) fire it
//! once per TRANSCRIPT, not once per session: /clear rotates the transcript
//! to a new UUID under the same live process and the hook fires again. Go:
//! `session_bind.go`, `harpmarker` (release/0.7).

use angzarr_diagnostic::Diagnostic;

use crate::{HookError, HookWarning};

/// The engine whose codec this crate holds.
pub const CLAUDE_CODE: &str = "claude-code";

/// What binds a session: the application's, behind the hook.
pub trait Binding {
    /// Record that `harp`'s engine session is `native_id`, its transcript
    /// `transcript_path` (either may be empty).
    fn bind(
        &mut self,
        harp: &str,
        native_id: &str,
        transcript_path: &str,
    ) -> Result<(), Box<dyn Diagnostic>>;
}

/// What the hook answered: the engine's input, and what it warned of.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Outcome {
    pub stdout: String,
    pub warnings: Vec<HookWarning>,
}

/// The harp self-id marker, `<ctxloom name="..." kind="harp" />`: the only
/// index-independent statement of which harp owns a transcript. `None` for
/// nothing it can say truthfully: an empty harp, or one the attribute syntax
/// cannot carry (a `"` or `>` would make it read back as a different harp).
pub fn marker(harp: &str) -> Option<String> {
    if harp.is_empty() || harp.contains(['"', '>']) {
        return None;
    }
    Some(format!(r#"<ctxloom name="{harp}" kind="harp" />"#))
}

/// Claude's answer to a SessionStart hook that adds `context`: one JSON line,
/// `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":...}}`.
pub fn session_start_reply(context: &str) -> String {
    let reply = serde_json::json!({"hookSpecificOutput": {
        "hookEventName": "SessionStart",
        "additionalContext": context,
    }});
    format!("{reply}\n")
}

/// The fields of claude's hook payload the bind reads: every payload carries
/// the engine's session and transcript.
#[derive(serde::Deserialize, Default)]
struct ClaudePayload {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    transcript_path: String,
}

/// The hook: emit the marker, then bind what `input` (the engine's
/// session_start payload) names to `harp`.
///
/// With no usable harp there is nothing truthful to emit: stdout stays empty,
/// a warning says so, and nothing is bound. A payload the engine's codec
/// cannot read is an error after the marker is out, as Go's is: the bind did
/// not happen, and saying so is how an operator learns why a harp never got
/// captured.
pub fn session_bind(
    engine: Option<&str>,
    input: &str,
    harp: &str,
    binding: &mut impl Binding,
) -> Result<Outcome, Box<dyn Diagnostic>> {
    match engine {
        None | Some(CLAUDE_CODE) => {}
        Some(other) => {
            return Err(Box::new(HookError::UnknownEngine {
                engine: other.to_string(),
                known: CLAUDE_CODE.to_string(),
            }))
        }
    }
    let Some(marker) = marker(harp) else {
        return Ok(Outcome {
            stdout: String::new(),
            warnings: vec![HookWarning::NoUsableHarp {
                harp: harp.to_string(),
            }],
        });
    };
    // The marker is out before the bind, so a bind that fails still leaves it.
    let stdout = session_start_reply(&marker);
    let payload: ClaudePayload =
        serde_json::from_str(input).map_err(|e| -> Box<dyn Diagnostic> {
            Box::new(HookError::Payload {
                harp: harp.to_string(),
                detail: e.to_string(),
            })
        })?;
    binding.bind(harp, &payload.session_id, &payload.transcript_path)?;
    Ok(Outcome {
        stdout,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
#[path = "bind.test.rs"]
mod tests;
