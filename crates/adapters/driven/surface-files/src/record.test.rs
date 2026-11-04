use super::*;

fn sample() -> Record {
    let mut record = Record::default();
    record.files.insert(
        ".claude/settings.json".to_string(),
        FileRecord {
            existed: true,
            original: b"{}\n".to_vec(),
            written: b"{\"a\": 1}".to_vec(),
            claims: vec![
                Owned {
                    engine: "claude-code".into(),
                    kind: Kind::Hooks,
                    claim: Claim::Json {
                        path: ".claude/settings.json".into(),
                        ops: vec![
                            JsonOp::Set {
                                path: vec!["a".into()],
                                value: Json::Number("1".into()),
                            },
                            JsonOp::SetCommand {
                                path: vec!["statusLine".into()],
                                value: Json::Null,
                            },
                            JsonOp::Push {
                                path: vec!["p".into(), "deny".into()],
                                value: Json::text("Bash"),
                            },
                            JsonOp::HookEntry {
                                event: "PostToolUse".into(),
                                matcher: Some("Skill".into()),
                                value: Json::object([("command", Json::text("ctxloom"))]),
                            },
                            JsonOp::HookEntry {
                                event: "SessionStart".into(),
                                matcher: None,
                                value: Json::Null,
                            },
                        ],
                    },
                },
                Owned {
                    engine: "claude-code".into(),
                    kind: Kind::Context,
                    claim: Claim::Section {
                        path: ".claude/settings.json".into(),
                        text: "text\n".into(),
                    },
                },
                Owned {
                    engine: "mock".into(),
                    kind: Kind::Skills,
                    claim: Claim::Whole {
                        path: ".claude/settings.json".into(),
                        bytes: vec![0, 255, 7],
                        executable: true,
                    },
                },
            ],
        },
    );
    record
}

#[test]
fn test_a_record_reads_back_as_it_was_written() {
    let record = sample();
    let text = record.encode("/work/project");
    assert_eq!(Record::decode("x", &text).unwrap(), record);
}

#[test]
fn test_a_record_of_another_schema_is_refused() {
    let text = sample().encode("/t").replace("\"1\"", "\"9\"");
    assert!(matches!(
        Record::decode("x", &text),
        Err(SurfaceError::Record { .. })
    ));
}

#[test]
fn test_text_that_is_no_record_is_refused() {
    for bad in [
        "",
        "[]",
        "{\"schema_version\": \"1\"}",
        "{\"schema_version\": \"1\", \"files\": {\"a\": {}}}",
    ] {
        assert!(Record::decode("x", bad).is_err(), "{bad:?}");
    }
}

#[test]
fn test_bytes_are_written_as_hex_and_read_back() {
    assert_eq!(hex(&[0, 15, 255]), "000fff");
    assert_eq!(unhex("000fff"), Some(vec![0, 15, 255]));
    assert_eq!(unhex("0"), None);
    assert_eq!(unhex("zz"), None);
}
