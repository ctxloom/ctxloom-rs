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

/// A record with the edge values: nothing in a file, empty bytes, escapes and
/// non-ASCII text, a number kept as written, a null matcher, nesting.
fn edge() -> Record {
    let mut record = Record::default();
    record.files.insert("empty".into(), FileRecord::default());
    record.files.insert(
        "a/\"b\".txt".into(),
        FileRecord {
            existed: false,
            original: vec![],
            written: vec![0x0a, 0xab, 0x00],
            claims: vec![
                Owned {
                    engine: "e\"\\\n\u{e9}\u{1f600}".into(),
                    kind: Kind::Mcp,
                    claim: Claim::Whole {
                        path: "a/\"b\".txt".into(),
                        bytes: vec![],
                        executable: false,
                    },
                },
                Owned {
                    engine: "e".into(),
                    kind: Kind::Settings,
                    claim: Claim::Json {
                        path: "a/\"b\".txt".into(),
                        ops: vec![
                            JsonOp::Set {
                                path: vec![],
                                value: Json::object([(
                                    "n",
                                    Json::Array(vec![
                                        Json::Number("1.50e+3".into()),
                                        Json::Bool(true),
                                        Json::Null,
                                        Json::object::<&str>([]),
                                    ]),
                                )]),
                            },
                            JsonOp::HookEntry {
                                event: "E".into(),
                                matcher: None,
                                value: Json::text("tab\there"),
                            },
                        ],
                    },
                },
                Owned {
                    engine: "e".into(),
                    kind: Kind::Context,
                    claim: Claim::Section {
                        path: "a/\"b\".txt".into(),
                        text: String::new(),
                    },
                },
            ],
        },
    );
    record
}

#[test]
fn test_the_record_on_disk_is_these_exact_bytes() {
    assert_eq!(sample().encode("/work/project"), SAMPLE_BYTES);
    assert_eq!(edge().encode("/t \"q\""), edge_bytes());
}

#[test]
fn test_the_persisted_bytes_read_back_as_the_record() {
    assert_eq!(Record::decode("x", SAMPLE_BYTES).unwrap(), sample());
    assert_eq!(Record::decode("x", &edge_bytes()).unwrap(), edge());
}

#[test]
fn test_the_decoder_keeps_its_leniencies() {
    let ok = |text: &str| Record::decode("x", text).unwrap();
    // `existed` and `executable` are true only when they are `true`; the
    // target and unknown members are ignored; a matcher that is no string
    // is none; hex may be upper case.
    let record = ok(r#"{"schema_version":"1","target":5,"extra":1,"files":{"f":{
        "existed":"yes","original":"AB","written":"","extra":null,"claims":[
        {"engine":"e","kind":"hooks","x":1,"claim":{"type":"json","ops":[
          {"op":"hook","event":"E","matcher":7,"value":null}]}},
        {"engine":"e","kind":"skills","claim":{"type":"whole","bytes":"0aFF","executable":1}}
        ]}}}"#);
    let file = &record.files["f"];
    assert!(!file.existed);
    assert_eq!(file.original, vec![0xab]);
    assert_eq!(
        file.claims[0].claim,
        Claim::Json {
            path: "f".into(),
            ops: vec![JsonOp::HookEntry {
                event: "E".into(),
                matcher: None,
                value: Json::Null
            }]
        }
    );
    assert_eq!(
        file.claims[1].claim,
        Claim::Whole {
            path: "f".into(),
            bytes: vec![10, 255],
            executable: false
        }
    );
    let held = r#"{"schema_version":"1","files":{"f":{"existed":true,"original":"","written":"","claims":[]}}}"#;
    assert!(ok(held).files["f"].existed);
}

#[test]
fn test_the_decoder_keeps_its_refusals() {
    let claim = |c: &str| {
        format!(
            r#"{{"schema_version":"1","files":{{"f":{{"original":"","written":"","claims":[{{"engine":"e","kind":"mcp","claim":{c}}}]}}}}}}"#
        )
    };
    let mut bad = vec![
        r#"{"schema_version":1,"files":{}}"#.to_string(),
        r#"{"files":{}}"#.to_string(),
        r#"{"schema_version":"1","files":[]}"#.to_string(),
        r#"{"schema_version":"1","files":{"f":{"original":"","written":""}}}"#.to_string(),
        r#"{"schema_version":"1","files":{"f":{"original":"0","written":"","claims":[]}}}"#.to_string(),
        r#"{"schema_version":"1","files":{"f":{"original":"","claims":[]}}}"#.to_string(),
        r#"{"schema_version":"1","files":{"f":{"original":"","written":"","claims":[{"engine":"e","kind":"nope","claim":{"type":"section","text":""}}]}}}"#.to_string(),
    ];
    for c in [
        r#"{"type":"other"}"#,
        r#"{"type":"json","ops":[{"op":"set","path":["a"]}]}"#,
        r#"{"type":"json","ops":[{"op":"hook","event":"E","value":null}]}"#,
        r#"{"type":"json","ops":[{"op":"zap","path":[],"value":null}]}"#,
        r#"{"type":"whole","bytes":"zz"}"#,
        r#"{"type":"section"}"#,
    ] {
        bad.push(claim(c));
    }
    for bad in bad {
        assert!(
            matches!(Record::decode("x", &bad), Err(SurfaceError::Record { .. })),
            "{bad}"
        );
    }
}

/// What the first encoder wrote for `sample()`: persisted, so it must not move.
const SAMPLE_BYTES: &str = r#"{
  "schema_version": "1",
  "target": "/work/project",
  "files": {
    ".claude/settings.json": {
      "existed": true,
      "original": "7b7d0a",
      "written": "7b2261223a20317d",
      "claims": [
        {
          "engine": "claude-code",
          "kind": "hooks",
          "claim": {
            "type": "json",
            "ops": [
              {
                "op": "set",
                "path": [
                  "a"
                ],
                "value": 1
              },
              {
                "op": "set-command",
                "path": [
                  "statusLine"
                ],
                "value": null
              },
              {
                "op": "push",
                "path": [
                  "p",
                  "deny"
                ],
                "value": "Bash"
              },
              {
                "op": "hook",
                "event": "PostToolUse",
                "matcher": "Skill",
                "value": {
                  "command": "ctxloom"
                }
              },
              {
                "op": "hook",
                "event": "SessionStart",
                "matcher": null,
                "value": null
              }
            ]
          }
        },
        {
          "engine": "claude-code",
          "kind": "context",
          "claim": {
            "type": "section",
            "text": "text\n"
          }
        },
        {
          "engine": "mock",
          "kind": "skills",
          "claim": {
            "type": "whole",
            "bytes": "00ff07",
            "executable": true
          }
        }
      ]
    }
  }
}
"#;

/// The same for `edge()`.
const EDGE_TEXT: &str = r#"{
  "schema_version": "1",
  "target": "/t \"q\"",
  "files": {
    "a/\"b\".txt": {
      "existed": false,
      "original": "",
      "written": "0aab00",
      "claims": [
        {
          "engine": "e\"\\\n@U@",
          "kind": "mcp",
          "claim": {
            "type": "whole",
            "bytes": "",
            "executable": false
          }
        },
        {
          "engine": "e",
          "kind": "settings",
          "claim": {
            "type": "json",
            "ops": [
              {
                "op": "set",
                "path": [],
                "value": {
                  "n": [
                    1.50e+3,
                    true,
                    null,
                    {}
                  ]
                }
              },
              {
                "op": "hook",
                "event": "E",
                "matcher": null,
                "value": "tab\there"
              }
            ]
          }
        },
        {
          "engine": "e",
          "kind": "context",
          "claim": {
            "type": "section",
            "text": ""
          }
        }
      ]
    },
    "empty": {
      "existed": false,
      "original": "",
      "written": "",
      "claims": []
    }
  }
}
"#;

/// `EDGE_TEXT` with its non-ASCII engine name (the source stays ASCII).
fn edge_bytes() -> String {
    EDGE_TEXT.replace("@U@", "\u{e9}\u{1f600}")
}
