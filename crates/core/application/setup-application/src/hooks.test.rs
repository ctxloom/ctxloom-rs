//! The engines' callbacks (cli/session_hooks.feature, cli/post_tool_hooks.feature).

use super::*;
use crate::fakes::{world, CONFIG};

// session_hooks.feature: the hook answers through the codec of the engine it
// was registered under; an engine it does not know is refused by name.
#[test]
fn test_a_hook_under_an_unknown_engine_is_refused_by_name() {
    let w = world(CONFIG);
    let err = w.setup.session_start("nosuch", "{}").unwrap_err();
    let SetupError::UnknownHookEngine { engine, known } = err else {
        panic!()
    };
    assert_eq!(
        (engine.as_str(), known.as_str()),
        ("nosuch", "claude-code, mock")
    );
}

// Go's session-start: a payload it cannot read is an error (an engine shows a
// failed hook, which is the truth).
#[test]
fn test_a_payload_that_does_not_read_fails_the_hook() {
    let w = world(CONFIG);
    assert!(w.setup.session_start("claude-code", "not json").is_err());
    assert!(w.setup.skill_mates("claude-code", "not json").is_err());
}

// session_hooks.feature: Scenario: The SessionStart hook never hands the
// engine the project's context - what it hands is a notice, and only when the
// project has profiles and no agents.
#[test]
fn test_session_start_nudges_only_a_project_with_profiles_and_no_agents() {
    let none = world("schema_version: 8\n");
    let out = none.setup.session_start("claude-code", "{}").unwrap();
    assert!(
        out.starts_with("context=;notice=ctxloom: this project has profiles but no agents"),
        "{out}"
    );
    assert!(!out.contains("FRAGMENT-BODY"));
    let agents = world(CONFIG);
    assert_eq!(
        agents.setup.session_start("claude-code", "{}").unwrap(),
        "context=;notice=\n"
    );
}

// post_tool_hooks.feature: Scenario: Completing one skill of a linked pair
// names the other
#[test]
fn test_completing_one_skill_names_the_linked_skill_not_yet_invoked() {
    let w = world(CONFIG);
    let out = w
        .setup
        .skill_mates("claude-code", r#"{"skill":"admit"}"#)
        .unwrap();
    assert_eq!(
        out,
        "context=Skill admit completed; linked skills not yet invoked this session: unattended;notice=\n"
    );
}

// post_tool_hooks.feature: the transcript is where "already invoked" comes
// from, so nothing is persisted to answer it.
#[test]
fn test_a_mate_the_transcript_shows_invoked_is_not_named() {
    let w = world(CONFIG);
    w.store
        .state()
        .files
        .insert("/t/turn.jsonl".to_string(), b"unattended\n".to_vec());
    let out = w
        .setup
        .skill_mates(
            "claude-code",
            r#"{"skill":"admit","transcript":"/t/turn.jsonl"}"#,
        )
        .unwrap();
    assert_eq!(out, "context=;notice=\n");
}

// post_tool_hooks.feature: Scenario: A tool call that is not a skill is passed
// over in silence
#[test]
fn test_a_call_that_ran_no_skill_is_silent() {
    let w = world(CONFIG);
    assert_eq!(
        w.setup.skill_mates("claude-code", "{}").unwrap(),
        "context=;notice=\n"
    );
}

// A skill in no group, or a skill the profile does not deliver, has no mates.
#[test]
fn test_a_skill_in_no_group_has_no_mates() {
    let w = world(CONFIG);
    assert_eq!(
        w.setup
            .skill_mates("claude-code", r#"{"skill":"nosuch"}"#)
            .unwrap(),
        "context=;notice=\n"
    );
}
