//! The YAML and the types Go printed for the fixtures `cfg/v8` and `cfg/layers`
//! (`config show --format text`): see `fixtures/golden/cfg-*-show-text.stdout`.

use super::*;

fn parse(text: &str) -> Value {
    angzarr_yaml::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

// Go prints a list's dashes indented under its key, two spaces a level, and
// a string that reads back as another type in double quotes.
#[test]
fn test_the_yaml_go_prints() {
    let doc = typed(
        &parse("schema_version: 8\nisolation_pids_limit: 512\nisolation_memory_limit: 4g\ncompanions: [acme, beta]\nconfig:\n  statusline: false\nagents:\n  dev:\n    llm: claude-code\n    env: [ALPHA]\n"),
        &[],
        Order::Sorted,
    );
    assert_eq!(
        to_yaml(&doc),
        "agents:\n  dev:\n    env:\n      - ALPHA\n    llm: claude-code\ncompanions:\n  - acme\n  - beta\nconfig:\n  statusline: false\nisolation_memory_limit: 4g\nisolation_pids_limit: \"512\"\nschema_version: 8\n"
    );
}

// The schema decides the types: integer, boolean, string; the limits are
// strings even when they look like numbers.
#[test]
fn test_the_schema_decides_which_scalars_are_numbers_and_booleans() {
    let doc = typed(
        &parse("delegation:\n  concurrency: 3\nsync:\n  auto_sync: false\nisolation_pids_limit: 512\neditor:\n  command: \"true\"\n"),
        &[],
        Order::Sorted,
    );
    let Typed::Map(entries) = doc else {
        panic!("a map")
    };
    let get = |k: &str| {
        entries
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
            .unwrap()
    };
    assert_eq!(
        get("delegation"),
        Typed::Map(vec![("concurrency".into(), Typed::Int(3))])
    );
    assert_eq!(
        get("sync"),
        Typed::Map(vec![("auto_sync".into(), Typed::Bool(false))])
    );
    assert_eq!(get("isolation_pids_limit"), Typed::Str("512".into()));
    assert_eq!(
        get("editor"),
        Typed::Map(vec![("command".into(), Typed::Str("true".into()))])
    );
}

// cfg-v8-section-agents-text: a section prints in the order of its record's
// fields (llm, profiles, runtime, permissions, driving, env_host, env), the
// whole document sorted.
#[test]
fn test_a_section_prints_in_the_order_of_its_fields() {
    let doc = parse("agents:\n  dev:\n    env: [ALPHA]\n    driving: oneshot\n    permissions:\n      claude-code:\n        mode: plan\n      approver: human\n    runtime: host\n    profiles: [p1]\n    llm: claude-code\n    env_host: false\n");
    let section = typed(&doc, &["agents".to_string()], Order::Fields);
    assert_eq!(
        to_yaml(&section),
        "dev:\n  llm: claude-code\n  profiles:\n    - p1\n  runtime: host\n  permissions:\n    approver: human\n    claude-code:\n      mode: plan\n  driving: oneshot\n  env_host: false\n  env:\n    - ALPHA\n"
    );
    let llm = typed(
        &parse("llm:\n  configs:\n    claude-code:\n      model: opus\n      role: primary\n      type: claude-code\n  defaults:\n    fast: claude-fast\n    primary: claude-code\n"),
        &["llm".to_string()],
        Order::Fields,
    );
    assert_eq!(
        to_yaml(&llm),
        "configs:\n  claude-code:\n    type: claude-code\n    role: primary\n    model: opus\ndefaults:\n  primary: claude-code\n  fast: claude-fast\n"
    );
}

// the empty document and a section that is not there
#[test]
fn test_an_empty_section_prints_as_braces() {
    assert_eq!(to_yaml(&Typed::Map(vec![])), "{}\n");
}

fn s(text: &str) -> Typed {
    Typed::Str(text.to_string())
}

fn entry(key: &str, value: Typed) -> (String, Typed) {
    (key.to_string(), value)
}

// Go's yaml.v3 emitter: collections of every nesting, empty ones inline
#[test]
fn test_the_emitter_nests_maps_and_lists_two_spaces_a_level() {
    use Typed::{Bool, Int, Map, Null, Seq};
    assert_eq!(to_yaml(&Seq(vec![])), "[]\n");
    assert_eq!(
        to_yaml(&Seq(vec![s("a"), Int(2), Bool(true), Null])),
        "- a\n- 2\n- true\n- null\n"
    );
    assert_eq!(
        to_yaml(&Seq(vec![Map(vec![
            entry("a", Int(1)),
            entry("b", s("x"))
        ])])),
        "- a: 1\n  b: x\n"
    );
    assert_eq!(
        to_yaml(&Seq(vec![Seq(vec![Int(1), Int(2)]), Seq(vec![s("z")])])),
        "- - 1\n  - 2\n- - z\n"
    );
    assert_eq!(
        to_yaml(&Seq(vec![Map(vec![]), Seq(vec![])])),
        "- {}\n- []\n"
    );
    assert_eq!(
        to_yaml(&Map(vec![
            entry("empty_map", Map(vec![])),
            entry("empty_list", Seq(vec![])),
            entry("list", Seq(vec![s("a"), s("b")])),
            entry("items", Seq(vec![Map(vec![entry("k", Int(1)), entry("l", Int(2))])])),
            entry("nested", Map(vec![entry("deep", Map(vec![entry("x", Bool(false))]))])),
            entry("grid", Seq(vec![Seq(vec![Int(1), Int(2)])])),
            entry("float", Typed::Float(1.5)),
        ])),
        "empty_map: {}\nempty_list: []\nlist:\n  - a\n  - b\nitems:\n  - k: 1\n    l: 2\nnested:\n  deep:\n    x: false\ngrid:\n  - - 1\n    - 2\nfloat: 1.5\n"
    );
    assert_eq!(to_yaml(&s("plain")), "plain\n");
    assert_eq!(to_yaml(&Int(7)), "7\n");
}

// a string that would read back as another type is double-quoted
#[test]
fn test_a_string_is_quoted_when_it_would_read_back_as_something_else() {
    for text in [
        "", "null", "Null", "NULL", "~", "true", "True", "TRUE", "false", "False", "FALSE", "y",
        "Y", "yes", "Yes", "YES", "n", "N", "no", "No", "NO", "on", "off", "On", "ON", "Off",
        "OFF", "12", "-3", "1.5", "1e3", "0x1f", "0o7", "-0x1", "+.inf", ".NaN", "-.INF", " a",
        "a ", "a\nb", "a\tb", "a\rb", "k: v", "a #b", "a:", "-", "- a", "?", "? a", ":", ": a",
        ",a", "[a", "]a", "{a", "}a", "#a", "&a", "*a", "!a", "|a", ">a", "'a", "\"a", "%a", "@a",
        "`a",
    ] {
        assert!(
            quote_if_needed(text).starts_with('"'),
            "{text:?} is quoted, got {}",
            quote_if_needed(text)
        );
    }
    for text in [
        "abc", "a-b", "a b", "-a", "?a", ":a", "a:b", "a#b", "x.y", "Ona",
    ] {
        assert_eq!(quote_if_needed(text), text, "{text:?} stays plain");
    }
}

// the quoting escapes what a double-quoted scalar cannot hold
#[test]
fn test_quoting_escapes_quotes_backslashes_newlines_and_tabs() {
    assert_eq!(quote_if_needed("\"q"), "\"\\\"q\"");
    assert_eq!(quote_if_needed("[\\"), "\"[\\\\\"");
    assert_eq!(quote_if_needed("a\nb"), "\"a\\nb\"");
    assert_eq!(quote_if_needed("a\tb"), "\"a\\tb\"");
    assert_eq!(quote_if_needed(""), "\"\"");
}

// the schema types a boolean and an integer; a free-form map reads what it sees
#[test]
fn test_scalars_are_typed_by_the_schema_or_by_what_they_look_like() {
    let doc = typed(
        &parse("sync:\n  auto_sync: yes\ndelegation:\n  concurrency: lots\nllm:\n  configs:\n    a:\n      type: opencode\n      model_options:\n        t: true\n        f: false\n        n: 7\n        w: word\n"),
        &[],
        Order::Sorted,
    );
    let Typed::Map(top) = doc else {
        panic!("a map")
    };
    let at = |entries: &[(String, Typed)], k: &str| -> Typed {
        entries.iter().find(|(key, _)| key == k).expect(k).1.clone()
    };
    let Typed::Map(sync) = at(&top, "sync") else {
        panic!()
    };
    assert_eq!(at(&sync, "auto_sync"), s("yes"), "not a boolean spelling");
    let Typed::Map(delegation) = at(&top, "delegation") else {
        panic!()
    };
    assert_eq!(at(&delegation, "concurrency"), s("lots"));
    let Typed::Map(llm) = at(&top, "llm") else {
        panic!()
    };
    let Typed::Map(configs) = at(&llm, "configs") else {
        panic!()
    };
    let Typed::Map(a) = at(&configs, "a") else {
        panic!()
    };
    let Typed::Map(options) = at(&a, "model_options") else {
        panic!()
    };
    assert_eq!(at(&options, "t"), Typed::Bool(true));
    assert_eq!(at(&options, "f"), Typed::Bool(false));
    assert_eq!(at(&options, "n"), Typed::Int(7));
    assert_eq!(at(&options, "w"), s("word"));
    let on = typed(&parse("sync:\n  auto_sync: true\n"), &[], Order::Sorted);
    assert_eq!(
        on,
        Typed::Map(vec![entry(
            "sync",
            Typed::Map(vec![entry("auto_sync", Typed::Bool(true))])
        )])
    );
}
