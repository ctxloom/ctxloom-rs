use super::*;

#[test]
fn test_a_kind_is_found_by_the_name_it_reports() {
    for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.name()), Some(kind));
    }
    assert_eq!(Kind::parse("nothing"), None);
}

#[test]
fn test_the_kinds_are_listed_in_delivery_order() {
    let mut sorted = Kind::ALL;
    sorted.sort();
    assert_eq!(sorted, Kind::ALL);
}

#[test]
fn test_a_ctxloom_callback_is_the_bare_executable_running_hook() {
    let callback = Hook {
        command: "ctxloom".into(),
        args: vec!["hook".into(), "skill-mates".into()],
        ..Hook::default()
    };
    assert!(callback.is_callback());
    let other = Hook {
        command: "ctxloom".into(),
        args: vec!["profile".into(), "list".into()],
        ..Hook::default()
    };
    assert!(!other.is_callback());
    let shell = Hook {
        command: "echo hook".into(),
        ..Hook::default()
    };
    assert!(!shell.is_callback());
}

#[test]
fn test_a_claim_names_the_file_it_is_on() {
    let section = Claim::Section {
        path: "CLAUDE.md".into(),
        text: String::new(),
    };
    let json = Claim::Json {
        path: ".mcp.json".into(),
        ops: Vec::new(),
    };
    let whole = Claim::Whole {
        path: "a/b.md".into(),
        bytes: Vec::new(),
        executable: false,
    };
    assert_eq!(section.path(), "CLAUDE.md");
    assert_eq!(json.path(), ".mcp.json");
    assert_eq!(whole.path(), "a/b.md");
}
