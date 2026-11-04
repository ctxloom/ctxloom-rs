use super::*;

fn hook(command: &str) -> Json {
    Json::object([
        ("type", Json::text("command")),
        ("command", Json::text(command)),
    ])
}

fn entry(event: &str, matcher: Option<&str>, command: &str) -> JsonOp {
    JsonOp::HookEntry {
        event: event.into(),
        matcher: matcher.map(str::to_string),
        value: hook(command),
    }
}

fn json_claim(ops: Vec<JsonOp>) -> Claim {
    Claim::Json {
        path: "settings.json".into(),
        ops,
    }
}

fn text_of(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap()
}

#[test]
fn test_a_section_is_appended_after_the_users_text_and_cut_out_again() {
    let claim = Claim::Section {
        path: "CLAUDE.md".into(),
        text: "ours".into(),
    };
    let mut left = Vec::new();
    let out = text_of(
        render(
            "CLAUDE.md",
            Some(b"# Theirs\n\nnotes"),
            &[&claim],
            &mut left,
        )
        .unwrap(),
    );
    assert_eq!(out, "# Theirs\n\nnotes\n\nours\n");
    let back = text_of(unrender("CLAUDE.md", out.as_bytes(), &[&claim]).unwrap());
    assert_eq!(back, "# Theirs\n\nnotes");
}

#[test]
fn test_a_section_in_an_empty_or_missing_file_is_the_whole_of_it() {
    let claim = Claim::Section {
        path: "CLAUDE.md".into(),
        text: "ours\n".into(),
    };
    let mut left = Vec::new();
    assert_eq!(
        text_of(render("f", None, &[&claim], &mut left).unwrap()),
        "ours\n"
    );
    assert_eq!(text_of(unrender("f", b"ours\n", &[&claim]).unwrap()), "");
}

#[test]
fn test_a_section_the_user_moved_text_around_is_still_cut_out() {
    let claim = Claim::Section {
        path: "f".into(),
        text: "ours".into(),
    };
    let back = text_of(unrender("f", b"top\n\nours\n\nbottom\n", &[&claim]).unwrap());
    assert_eq!(back, "top\n\nbottom\n");
}

#[test]
fn test_a_whole_file_is_its_claim() {
    let claim = Claim::Whole {
        path: "f".into(),
        bytes: b"all".to_vec(),
        executable: false,
    };
    let mut left = Vec::new();
    assert_eq!(
        render("f", Some(b"theirs"), &[&claim], &mut left).unwrap(),
        b"all"
    );
}

#[test]
fn test_set_creates_the_containers_and_keeps_the_rest() {
    let claim = json_claim(vec![JsonOp::Set {
        path: vec!["env".into(), "A".into()],
        value: Json::text("1"),
    }]);
    let mut left = Vec::new();
    let out = text_of(
        render(
            "settings.json",
            Some(br#"{"big": 18446744073709551615}"#),
            &[&claim],
            &mut left,
        )
        .unwrap(),
    );
    assert!(out.contains("18446744073709551615"), "{out}");
    assert!(out.contains("\"A\": \"1\""), "{out}");
}

#[test]
fn test_push_finds_a_value_the_user_already_has_and_takes_only_its_own() {
    let claim = json_claim(vec![
        JsonOp::Push {
            path: vec!["permissions".into(), "deny".into()],
            value: Json::text("Bash"),
        },
        JsonOp::Push {
            path: vec!["permissions".into(), "deny".into()],
            value: Json::text("Edit"),
        },
    ]);
    let mut left = Vec::new();
    let base = br#"{"permissions": {"deny": ["Bash"]}}"#;
    let out = render("settings.json", Some(base), &[&claim], &mut left).unwrap();
    let doc = Json::parse(&text_of(out.clone())).unwrap();
    assert_eq!(
        doc.get("permissions")
            .unwrap()
            .get("deny")
            .unwrap()
            .pretty()
            .matches("Bash")
            .count(),
        1
    );
    // Cut back out of a file somebody changed, both values go by value (the
    // record of the unchanged case restores the bytes instead).
    let back = text_of(unrender("settings.json", &out, &[&claim]).unwrap());
    assert!(!back.contains("Edit"), "{back}");
}

#[test]
fn test_a_hook_entry_joins_the_group_of_its_matcher_and_is_not_doubled() {
    let claim = json_claim(vec![
        entry("PostToolUse", Some("Skill"), "ctxloom"),
        entry("PostToolUse", Some("Skill"), "ctxloom"),
        entry("PostToolUse", None, "other"),
        entry("SessionStart", None, "ctxloom"),
    ]);
    let mut left = Vec::new();
    let out = text_of(render("settings.json", None, &[&claim], &mut left).unwrap());
    assert_eq!(out.matches("\"ctxloom\"").count(), 2, "{out}");
    let doc = Json::parse(&out).unwrap();
    let groups = doc
        .get("hooks")
        .unwrap()
        .get("PostToolUse")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(groups.len(), 2);
}

#[test]
fn test_taking_hooks_out_leaves_the_users_groups_and_removes_empty_ones() {
    let claim = json_claim(vec![
        entry("PostToolUse", Some("Skill"), "ctxloom"),
        entry("SessionStart", None, "ctxloom"),
    ]);
    let mut left = Vec::new();
    let base = br#"{"hooks": {"PostToolUse": [{"matcher": "Skill", "hooks": [{"type": "command", "command": "mine"}]}]}}"#;
    let out = render("settings.json", Some(base), &[&claim], &mut left).unwrap();
    let back = Json::parse(&text_of(
        unrender("settings.json", &out, &[&claim]).unwrap(),
    ))
    .unwrap();
    let text = back.pretty();
    assert!(text.contains("mine"), "{text}");
    assert!(!text.contains("ctxloom"), "{text}");
    assert!(back.get("hooks").unwrap().get("SessionStart").is_none());
}

#[test]
fn test_a_command_entry_of_the_users_is_left_and_one_it_cannot_read_is_refused() {
    let value = Json::object([
        ("type", Json::text("command")),
        ("command", Json::text("ctxloom hook hud")),
    ]);
    let claim = json_claim(vec![JsonOp::SetCommand {
        path: vec!["statusLine".into()],
        value,
    }]);
    let mut left = Vec::new();
    let mine = br#"{"statusLine": {"type": "command", "command": "ccusage"}}"#;
    let out = text_of(render("settings.json", Some(mine), &[&claim], &mut left).unwrap());
    assert!(
        out.contains("ccusage") && !out.contains("hook hud"),
        "{out}"
    );
    assert_eq!(left.len(), 1);

    let unreadable = br#"{"statusLine": {"type": "command", "command": {"exec": "ccusage"}}}"#;
    let refused = render("a/settings.json", Some(unreadable), &[&claim], &mut left).unwrap_err();
    assert!(matches!(refused, SurfaceError::Refused { ref file, .. } if file == "settings.json"));
}

#[test]
fn test_a_command_entry_already_ours_is_replaced() {
    let value = Json::object([
        ("type", Json::text("command")),
        ("command", Json::text("ctxloom hook hud")),
    ]);
    let claim = json_claim(vec![JsonOp::SetCommand {
        path: vec!["statusLine".into()],
        value,
    }]);
    let mut left = Vec::new();
    let held =
        br#"{"statusLine": {"type": "command", "command": "ctxloom hook hud", "padding": 0}}"#;
    let out = text_of(render("settings.json", Some(held), &[&claim], &mut left).unwrap());
    assert!(!out.contains("padding") && left.is_empty(), "{out}");
}

#[test]
fn test_a_file_that_is_not_json_is_refused_by_name() {
    let claim = json_claim(vec![]);
    let mut left = Vec::new();
    for bad in [&b"{ nope"[..], b"[1]", &[0xff, 0xfe][..]] {
        let err = render("dir/settings.json", Some(bad), &[&claim], &mut left).unwrap_err();
        assert!(
            matches!(err, SurfaceError::Refused { ref file, .. } if file == "settings.json"),
            "{err:?}"
        );
    }
}

#[test]
fn test_a_path_through_a_value_that_is_not_an_object_is_refused() {
    let claim = json_claim(vec![JsonOp::Set {
        path: vec!["env".into(), "A".into()],
        value: Json::Null,
    }]);
    let mut left = Vec::new();
    assert!(render(
        "settings.json",
        Some(br#"{"env": 3}"#),
        &[&claim],
        &mut left
    )
    .is_err());
}

#[test]
fn test_set_values_are_taken_back_only_when_they_are_still_ours() {
    let claim = json_claim(vec![JsonOp::Set {
        path: vec!["env".into(), "A".into()],
        value: Json::text("1"),
    }]);
    let ours = unrender(
        "settings.json",
        br#"{"env": {"A": "1"}, "keep": true}"#,
        &[&claim],
    )
    .unwrap();
    assert_eq!(
        Json::parse(&text_of(ours)).unwrap().pretty(),
        "{\n  \"keep\": true\n}"
    );
    let theirs = unrender("settings.json", br#"{"env": {"A": "2"}}"#, &[&claim]).unwrap();
    assert!(text_of(theirs).contains("\"2\""));
}

#[test]
fn test_cutting_a_pushed_value_out_removes_that_value_and_no_other() {
    let claim = json_claim(vec![JsonOp::Push {
        path: vec!["deny".into()],
        value: Json::text("Edit"),
    }]);
    let held = br#"{"deny": ["Bash", "Edit", "Read"]}"#;
    let back = text_of(unrender("settings.json", held, &[&claim]).unwrap());
    let doc = Json::parse(&back).unwrap();
    assert_eq!(
        doc.get("deny").unwrap(),
        &Json::Array(vec![Json::text("Bash"), Json::text("Read")])
    );
}
