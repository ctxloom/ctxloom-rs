use angzarr_yaml::Value;

use super::*;

fn scalar(s: &str) -> Value {
    Value::Scalar(s.to_string())
}

fn map(entries: &[(&str, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    )
}

fn seq(items: &[&str]) -> Value {
    Value::Seq(items.iter().map(|i| scalar(i)).collect())
}

// plan §10 S2: sorted keys
#[test]
fn test_sorted_orders_every_mapping_by_bytes_and_keeps_list_order() {
    let value = map(&[
        ("zeta", seq(&["b", "a"])),
        (
            "alpha",
            map(&[("y", scalar("1")), ("B", scalar("2")), ("x", scalar("3"))]),
        ),
    ]);
    assert_eq!(
        sorted(&value),
        map(&[
            (
                "alpha",
                map(&[("B", scalar("2")), ("x", scalar("3")), ("y", scalar("1"))])
            ),
            ("zeta", seq(&["b", "a"])),
        ])
    );
    // A mapping inside a list is sorted too; scalars and null are as they are.
    let nested = Value::Seq(vec![map(&[("b", Value::Null), ("a", scalar("1"))])]);
    assert_eq!(
        sorted(&nested),
        Value::Seq(vec![map(&[("a", scalar("1")), ("b", Value::Null)])])
    );
    assert_eq!(sorted(&scalar("x")), scalar("x"));
}

// Go's fresh document: keys sorted, block style (config_save.go)
#[test]
fn test_render_sorted_writes_a_fresh_document_sorted_in_block_style() {
    let doc = map(&[
        ("schema_version", scalar("8")),
        (
            "llm",
            map(&[("defaults", map(&[("primary", scalar("a"))]))]),
        ),
        (
            "agents",
            map(&[("dev", map(&[("profiles", seq(&["p", "q"]))]))]),
        ),
    ]);
    assert_eq!(
        render_sorted(&doc),
        "agents:\n  dev:\n    profiles:\n      - p\n      - q\nllm:\n  defaults:\n    primary: a\nschema_version: 8\n"
    );
}

// config_save.go: a first write has no existing bytes
#[test]
fn test_a_section_set_on_nothing_is_a_document_of_that_section() {
    let value = map(&[("b", scalar("2")), ("a", scalar("1"))]);
    assert_eq!(
        set_section("", "agents", &value).unwrap(),
        "agents:\n  a: 1\n  b: 2\n"
    );
    // A file of comments alone has no mapping to put it in; it gets one.
    assert_eq!(
        set_section("# only a comment\n", "agents", &value).unwrap(),
        "# only a comment\nagents:\n  a: 1\n  b: 2\n"
    );
    // No final line break in the file: one is supplied before the new key.
    assert_eq!(
        set_section("schema_version: 8", "agents", &value).unwrap(),
        "schema_version: 8\nagents:\n  a: 1\n  b: 2\n"
    );
}

// config_save.go: only the section whose content changed is re-encoded; the
// comments and the order of the file survive
#[test]
fn test_a_section_set_replaces_only_its_own_lines() {
    let text = "# my config\nschema_version: 8\nzzz: kept # not modelled\nllm:\n  configs:\n    zeta:\n      type: mock\n    alpha:\n      type: mock\n# trailing\nauth: token\n";
    let value = map(&[(
        "configs",
        map(&[
            (
                "zeta",
                map(&[("type", scalar("mock")), ("model", scalar("z"))]),
            ),
            ("alpha", map(&[("type", scalar("mock"))])),
        ]),
    )]);
    assert_eq!(
        set_section(text, "llm", &value).unwrap(),
        "# my config\nschema_version: 8\nzzz: kept # not modelled\nllm:\n  configs:\n    alpha:\n      type: mock\n    zeta:\n      model: z\n      type: mock\n# trailing\nauth: token\n"
    );
}

#[test]
fn test_a_section_set_appends_a_new_key_at_the_end() {
    let text = "schema_version: 8\n# note\nllm:\n  defaults:\n    primary: a\n";
    assert_eq!(
        set_section(text, "companions", &seq(&["acme"])).unwrap(),
        "schema_version: 8\n# note\nllm:\n  defaults:\n    primary: a\ncompanions:\n  - acme\n"
    );
    assert_eq!(
        set_section(text, "default_agent", &scalar("dev")).unwrap(),
        "schema_version: 8\n# note\nllm:\n  defaults:\n    primary: a\ndefault_agent: dev\n"
    );
}

#[test]
fn test_setting_what_is_there_changes_no_byte() {
    let text = "# c\nschema_version: 8\nagents:\n  dev: # mine\n    llm: x\n";
    let value = map(&[("dev", map(&[("llm", scalar("x"))]))]);
    assert_eq!(set_section(text, "agents", &value).unwrap(), text);
}

// a value the writer must quote reads back as it was
#[test]
fn test_a_value_that_needs_quotes_is_quoted() {
    let value = map(&[("note", scalar("a: b # c")), ("list", seq(&["x y", "7"]))]);
    let out = set_section("", "agents", &value).unwrap();
    let back = angzarr_yaml::parse(&out).unwrap();
    assert_eq!(back.get("agents"), Some(&sorted(&value)));
}

// readExistingConfig: a file that will not parse is refused, never rewritten
#[test]
fn test_a_corrupt_file_is_refused_not_healed() {
    for corrupt in [
        "llm: [unclosed\n",
        "::: not: [valid yaml\n\tbroken",
        "a: 1\n\tb: tab\n",
    ] {
        let err = set_section(corrupt, "agents", &scalar("x")).unwrap_err();
        assert!(
            matches!(err, ConfigError::Syntax { .. }),
            "{corrupt:?}: {err:?}"
        );
        let err = remove_section(corrupt, "agents").unwrap_err();
        assert!(
            matches!(err, ConfigError::Syntax { .. }),
            "{corrupt:?}: {err:?}"
        );
    }
}

#[test]
fn test_a_root_that_is_not_a_mapping_is_refused() {
    for text in ["- a\n- b\n", "just a scalar\n"] {
        assert_eq!(
            set_section(text, "agents", &scalar("x")).unwrap_err(),
            ConfigError::NotAMapping,
            "{text:?}"
        );
        assert_eq!(
            remove_section(text, "agents").unwrap_err(),
            ConfigError::NotAMapping,
            "{text:?}"
        );
    }
}

#[test]
fn test_removing_a_section_takes_its_lines_and_leaves_the_rest() {
    let text = "# c\nschema_version: 8\n# about agents\nagents:\n  dev:\n    llm: x\nauth: token\n";
    assert_eq!(
        remove_section(text, "agents").unwrap(),
        "# c\nschema_version: 8\nauth: token\n"
    );
    // Not there: unchanged, byte for byte.
    assert_eq!(remove_section(text, "companions").unwrap(), text);
    assert_eq!(remove_section("", "companions").unwrap(), "");
    assert_eq!(
        remove_section("# only\n", "companions").unwrap(),
        "# only\n"
    );
}

#[test]
fn test_a_section_is_read_where_it_is() {
    let text = "# c\nagents:\n  dev:\n    llm: x\nauth: token\n";
    assert_eq!(
        section(text, "agents").unwrap(),
        Some(map(&[("dev", map(&[("llm", scalar("x"))]))]))
    );
    assert_eq!(section(text, "auth").unwrap(), Some(scalar("token")));
    assert_eq!(section(text, "companions").unwrap(), None);
    assert_eq!(section("", "agents").unwrap(), None);
    assert!(matches!(
        section("a: [b\n", "a"),
        Err(ConfigError::Syntax { .. })
    ));
    assert_eq!(section("- a\n", "a").unwrap_err(), ConfigError::NotAMapping);
}

// config_save.go: a first write is a document of the generation this reads
#[test]
fn test_a_file_with_no_key_is_stamped_and_one_with_keys_is_left() {
    assert_eq!(stamped("").unwrap(), "schema_version: 8\n");
    assert_eq!(stamped("# only\n").unwrap(), "# only\nschema_version: 8\n");
    let held = "schema_version: 7\nagents: {}\n";
    assert_eq!(stamped(held).unwrap(), held);
    assert_eq!(stamped("auth: token\n").unwrap(), "auth: token\n");
    assert!(stamped("a: [b\n").is_err());
}
