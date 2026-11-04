//! The session-bind hook. Go: `harpmarker` (marker_test.go), session_bind.go
//! and session_hooks.feature ("Binding a session records the harp both in the
//! transcript and in the index"; "With no active harp the hook stays silent on
//! stdout and warns instead").

use angzarr_diagnostic::Diagnostic;

use super::{marker, session_bind, session_start_reply, Binding};
use crate::{HookError, HookWarning};

/// A binding that remembers what it was asked.
#[derive(Default)]
struct Bound {
    calls: Vec<(String, String, String)>,
    fail: Option<HookError>,
}

impl Binding for Bound {
    fn bind(
        &mut self,
        harp: &str,
        native_id: &str,
        transcript_path: &str,
    ) -> Result<(), Box<dyn Diagnostic>> {
        self.calls.push((
            harp.to_string(),
            native_id.to_string(),
            transcript_path.to_string(),
        ));
        match self.fail.clone() {
            Some(error) => Err(Box::new(error)),
            None => Ok(()),
        }
    }
}

const PAYLOAD: &str = r#"{"session_id":"vendor-session-42","transcript_path":"/tmp/vendor/transcript.jsonl","hook_event_name":"SessionStart"}"#;

// Go: harpmarker TestFormat.
#[test]
fn the_marker_names_the_harp() {
    assert_eq!(
        marker("plump-loose-sash").as_deref(),
        Some(r#"<ctxloom name="plump-loose-sash" kind="harp" />"#)
    );
    assert_eq!(marker(""), None);
}

// Go: "A '\"' closes the name attribute early, so the element reads back as a
// DIFFERENT, possibly real harp; a '>' closes the element before kind=\"harp\"
// is reached ... a marker that names the wrong harp is worse than no marker".
#[test]
fn a_harp_the_marker_cannot_carry_has_no_marker() {
    assert_eq!(marker("a\"b"), None);
    assert_eq!(marker("a>b"), None);
}

// Go: claude's Encode of a SessionStart context.
#[test]
fn the_reply_is_claudes_session_start_envelope_on_one_line() {
    let reply = session_start_reply(r#"<ctxloom name="x" kind="harp" />"#);
    assert!(reply.ends_with('\n'));
    assert_eq!(reply.matches('\n').count(), 1);
    let value: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": r#"<ctxloom name="x" kind="harp" />"#,
        }})
    );
}

// session_hooks.feature: two effects from one invocation: the marker, and the
// bind.
#[test]
fn a_bind_emits_the_marker_and_binds_the_payloads_session() {
    let mut bound = Bound::default();
    let outcome = session_bind(
        Some("claude-code"),
        PAYLOAD,
        "brisk-copper-moth",
        &mut bound,
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
    assert!(value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .contains("brisk-copper-moth"));
    assert!(outcome.warnings.is_empty());
    assert_eq!(
        bound.calls,
        [(
            "brisk-copper-moth".to_string(),
            "vendor-session-42".to_string(),
            "/tmp/vendor/transcript.jsonl".to_string()
        )]
    );
}

// Go: firingEngine: an entry without --engine (written by an earlier ctxloom,
// or by hand) is the registry's default engine.
#[test]
fn no_engine_is_claude_code() {
    let mut bound = Bound::default();
    session_bind(None, PAYLOAD, "brisk-copper-moth", &mut bound).unwrap();
    assert_eq!(bound.calls.len(), 1);
}

// Go: UnknownHookEngineError: "an explicit --engine naming no registered
// engine is refused, never rounded to the default".
#[test]
fn an_engine_that_is_none_is_refused_and_nothing_is_bound() {
    let mut bound = Bound::default();
    let error = session_bind(
        Some("no-such-engine"),
        PAYLOAD,
        "brisk-copper-moth",
        &mut bound,
    )
    .unwrap_err();
    assert_eq!(error.code(), "hook-cli-unknown-engine");
    assert!(bound.calls.is_empty());
}

// session_hooks.feature: "With no usable harp there is nothing truthful to
// emit, so the hook must emit NOTHING on stdout rather than a marker naming an
// empty harp ... The warning is what turns an untraceable session into a
// diagnosable one."
#[test]
fn with_no_harp_stdout_is_empty_and_a_warning_says_why() {
    let mut bound = Bound::default();
    let outcome = session_bind(
        Some("claude-code"),
        r#"{"session_id":"vendor-session-42","hook_event_name":"SessionStart"}"#,
        "",
        &mut bound,
    )
    .unwrap();
    assert_eq!(outcome.stdout, "");
    assert_eq!(
        outcome.warnings,
        [HookWarning::NoUsableHarp {
            harp: String::new()
        }]
    );
    assert!(bound.calls.is_empty(), "no harp: nothing is bound");
}

// Go: bindSessionFromPayload: a payload the codec cannot decode is an error
// naming the harp, after the marker was written ("a bind that fails below
// still leaves the marker on stdout"): the bind did not happen.
#[test]
fn a_payload_that_does_not_decode_is_an_error_naming_the_harp() {
    let mut bound = Bound::default();
    let error = session_bind(
        Some("claude-code"),
        "not json",
        "brisk-copper-moth",
        &mut bound,
    )
    .unwrap_err();
    assert_eq!(error.code(), "hook-cli-payload");
    assert!(error.args().iter().any(|(name, _)| *name == "harp"));
    assert!(bound.calls.is_empty());
}

// Go: a hook payload "that carried no session identifier at all" binds
// nothing, and is not an error (BindSession's both-empty no-op is the
// application's); the call is still made, with both empty.
#[test]
fn a_payload_naming_no_session_still_reaches_the_binding() {
    let mut bound = Bound::default();
    session_bind(Some("claude-code"), "{}", "brisk-copper-moth", &mut bound).unwrap();
    assert_eq!(
        bound.calls,
        [(
            "brisk-copper-moth".to_string(),
            String::new(),
            String::new()
        )]
    );
}

// Go: the index refusing the bind is an error the process exits non-zero on.
#[test]
fn a_binding_that_fails_fails_the_hook() {
    let mut bound = Bound {
        fail: Some(HookError::Payload {
            harp: "x".to_string(),
            detail: "refused".to_string(),
        }),
        ..Bound::default()
    };
    assert!(session_bind(
        Some("claude-code"),
        PAYLOAD,
        "brisk-copper-moth",
        &mut bound
    )
    .is_err());
}
