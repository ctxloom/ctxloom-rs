//! The hook verbs, whose stdout is the engine's input.

use angzarr_presentation::Format;

use super::*;
use crate::tests::{Fake, NoSessions, Wire};

/// The process facts the hooks read, with `harp` as the session's.
fn context(harp: &str) -> Context {
    Context {
        version: "1".to_string(),
        lock_protocol: 1,
        stdout_is_terminal: false,
        session_harp: harp.to_string(),
    }
}

// A hook's stdout is the engine's input: the statusline hook answers with the
// bare name when the payload is not a status document.
#[test]
fn test_the_hud_hook_writes_its_statusline_to_stdout() {
    let hook = hook_cli::Hook {
        engine: None,
        verb: hook_cli::HookVerb::Hud,
    };
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut services = Wire(Fake { answer: Ok(0) }, NoSessions);
    hook_verb(
        &hook,
        "not json",
        Format::Text,
        &context(""),
        &mut services,
        &catalog,
        &mut out,
        &mut err,
    )
    .expect("answers");
    assert_eq!(String::from_utf8(out).unwrap(), "ctxloom");
    assert!(err.is_empty());
}

// The session's harp is the process's `CTXLOOM_SESSION_HARP`, handed in by
// the composition root, and shown in the statusline.
#[test]
fn test_the_hud_hook_shows_the_harp_the_context_names() {
    let hook = hook_cli::Hook {
        engine: None,
        verb: hook_cli::HookVerb::Hud,
    };
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let run = |harp: &str| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut services = Wire(Fake { answer: Ok(0) }, NoSessions);
        hook_verb(
            &hook,
            r#"{"model":{"display_name":"M"}}"#,
            Format::Text,
            &context(harp),
            &mut services,
            &catalog,
            &mut out,
            &mut err,
        )
        .expect("answers");
        String::from_utf8(out).unwrap()
    };
    assert!(run("jolly-mute-magma").contains("jolly-mute-magma"));
    assert!(!run("").contains('\u{2301}'));
}
