use super::*;

#[test]
fn test_a_payload_is_read_on_ctxloom_own_wire() {
    let ev = Codec
        .decode(
            "turn_start",
            r#"{"event":"pre_tool","session":"s","transcript":"t","source":"x","prompt":"p","tool":"Bash"}"#,
        )
        .unwrap();
    assert_eq!(
        (
            ev.event.as_str(),
            ev.native_session.as_str(),
            ev.tool.as_str(),
            ev.prompt.as_str()
        ),
        ("pre_tool", "s", "Bash", "p")
    );
    assert_eq!(
        Codec.decode("turn_start", "{}").unwrap().event,
        "turn_start"
    );
    assert!(Codec.decode("turn_start", "no").is_err());
}

#[test]
fn test_an_answer_names_only_what_it_carries() {
    let r = HookResponse {
        context: "c".into(),
        notice: "n".into(),
        block: true,
        reason: "r".into(),
    };
    assert_eq!(
        Codec.encode("x", &r).unwrap(),
        "{\"context\":\"c\",\"notice\":\"n\",\"block\":true,\"reason\":\"r\"}\n"
    );
    assert_eq!(Codec.encode("x", &HookResponse::default()).unwrap(), "{}\n");
    assert!(Codec.invoked_skills("anything").is_empty());
}
