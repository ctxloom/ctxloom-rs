use super::*;

fn decode(payload: &str) -> HookEvent {
    Codec.decode("session_start", payload).unwrap()
}

#[test]
fn test_a_payload_is_read_into_the_neutral_event() {
    let ev = decode(
        r#"{"session_id":"s1","hook_event_name":"PostToolUse","transcript_path":"/t.jsonl","tool_name":"Skill","tool_input":{"skill":"admit","args":""},"prompt":"p","source":"startup"}"#,
    );
    assert_eq!(ev.event, "post_tool");
    assert_eq!(ev.native_session, "s1");
    assert_eq!(ev.transcript, "/t.jsonl");
    assert_eq!(ev.skill, "admit");
    assert_eq!(ev.tool, "Skill");
    assert_eq!(ev.prompt, "p");
    assert_eq!(ev.source, "startup");
}

#[test]
fn test_the_registered_event_is_used_when_the_payload_names_none() {
    assert_eq!(decode("{}").event, "session_start");
}

#[test]
fn test_every_native_event_has_its_unified_name() {
    for (native, unified) in [
        ("PreToolUse", "pre_tool"),
        ("SessionEnd", "session_end"),
        ("Stop", "turn_end"),
        ("UserPromptSubmit", "turn_start"),
        ("PermissionRequest", "permission_ask"),
        ("Unlisted", "Unlisted"),
    ] {
        let payload = format!(r#"{{"hook_event_name":"{native}"}}"#);
        assert_eq!(decode(&payload).event, unified);
    }
}

#[test]
fn test_only_the_skill_tool_runs_a_skill() {
    let other = decode(r#"{"tool_name":"Bash","tool_input":{"skill":"admit"}}"#);
    assert_eq!(other.skill, "");
    let empty = decode(r#"{"tool_name":"Skill","tool_input":{}}"#);
    assert_eq!(empty.skill, "");
}

#[test]
fn test_the_edited_file_is_the_file_path_or_the_notebook_path() {
    assert_eq!(
        decode(r#"{"tool_input":{"file_path":"a.md"}}"#).path,
        "a.md"
    );
    assert_eq!(
        decode(r#"{"tool_input":{"notebook_path":"n.ipynb"}}"#).path,
        "n.ipynb"
    );
    assert_eq!(decode(r#"{"tool_input":{"command":"ls"}}"#).path, "");
}

#[test]
fn test_a_payload_that_is_no_json_is_refused() {
    assert!(matches!(
        Codec.decode("session_start", "nope"),
        Err(SurfaceError::Payload { .. })
    ));
}

#[test]
fn test_context_rides_hook_specific_output_named_for_the_native_event() {
    let r = HookResponse {
        context: "hello".into(),
        ..HookResponse::default()
    };
    assert_eq!(
        Codec.encode("session_start", &r).unwrap(),
        "{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"hello\"}}\n"
    );
    assert!(Codec
        .encode("post_tool", &r)
        .unwrap()
        .contains("PostToolUse"));
    assert!(Codec
        .encode("turn_start", &r)
        .unwrap()
        .contains("UserPromptSubmit"));
}

#[test]
fn test_a_notice_rides_system_message_and_nothing_is_an_empty_object() {
    let r = HookResponse {
        notice: "n".into(),
        ..HookResponse::default()
    };
    assert_eq!(
        Codec.encode("session_start", &r).unwrap(),
        "{\"systemMessage\":\"n\"}\n"
    );
    assert_eq!(
        Codec
            .encode("session_start", &HookResponse::default())
            .unwrap(),
        "{}\n"
    );
}

#[test]
fn test_context_on_an_event_that_cannot_carry_it_is_refused() {
    let r = HookResponse {
        context: "c".into(),
        ..HookResponse::default()
    };
    assert!(matches!(
        Codec.encode("pre_tool", &r),
        Err(SurfaceError::Unsupported { .. })
    ));
}

#[test]
fn test_a_block_is_a_decision_only_where_claude_lets_it() {
    let r = HookResponse {
        block: true,
        reason: "no".into(),
        ..HookResponse::default()
    };
    assert_eq!(
        Codec.encode("turn_start", &r).unwrap(),
        "{\"decision\":\"block\",\"reason\":\"no\"}\n"
    );
    assert!(Codec.encode("session_start", &r).is_err());
}

#[test]
fn test_the_skills_a_transcript_invoked_are_its_main_threads() {
    let main = r#"{"type":"assistant","isSidechain":false,"message":{"content":[{"type":"text","text":"x"},{"type":"tool_use","name":"Skill","input":{"skill":"one"}},{"type":"tool_use","name":"Bash","input":{"skill":"no"}}]}}"#;
    let side = r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"two"}}]}}"#;
    let again = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"one"}}]}}"#;
    let user = r#"{"type":"user","message":{"content":[{"type":"tool_use","name":"Skill","input":{"skill":"three"}}]}}"#;
    let transcript = format!("{main}\nnot json\n{side}\n{again}\n{user}\n");
    assert_eq!(Codec.invoked_skills(&transcript), vec!["one".to_string()]);
}
