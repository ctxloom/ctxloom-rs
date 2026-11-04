//! The configuration's typed reading and the steps of loading it.

use angzarr_yaml::{parse, Value};
use ctxloom_ports::Source;

use super::{reduced_when_unreadable, stamp, Config};

fn config(text: &str) -> Config {
    let value = parse(text).expect("a document");
    Config {
        raw: value.clone(),
        value,
        authored_labels: std::collections::BTreeSet::new(),
        source: Source::Project,
    }
}

// llmconfig.PrimaryLabel: llm.defaults.primary, else the sole configured
// label, else none.
#[test]
fn test_the_primary_label_is_declared_or_the_sole_one_or_none() {
    let sole = config("llm:\n  configs:\n    only:\n      type: mock\n");
    assert_eq!(sole.primary_label(), "only");
    let two = config("llm:\n  configs:\n    a:\n      type: mock\n    b:\n      type: mock\n");
    assert_eq!(two.primary_label(), "");
    let declared = config(
        "llm:\n  configs:\n    a:\n      type: mock\n    b:\n      type: mock\n  defaults:\n    primary: b\n",
    );
    assert_eq!(declared.primary_label(), "b");
    assert_eq!(config("editor:\n  command: vim\n").primary_label(), "");
}

// config.use_distilled is on unless it says false.
#[test]
fn test_distilled_forms_are_preferred_unless_turned_off() {
    assert!(config("editor:\n  command: vim\n").use_distilled());
    assert!(config("config:\n  use_distilled: true\n").use_distilled());
    assert!(!config("config:\n  use_distilled: false\n").use_distilled());
}

// A config that fails to decode (a value of the wrong shape, in a layer or
// after the overrides) is the defaults alone.
#[test]
fn test_an_unreadable_shape_reduces_the_config_to_nothing() {
    let sound = parse("editor:\n  command: vim\n").unwrap();
    let unsound = parse("companions: acme\n").unwrap();
    assert_eq!(
        reduced_when_unreadable(sound.clone(), false),
        sound,
        "a sound config is kept"
    );
    assert_eq!(
        reduced_when_unreadable(sound, true),
        Value::Map(Vec::new()),
        "a layer's shape failure reduces it"
    );
    assert_eq!(
        reduced_when_unreadable(unsound, false),
        Value::Map(Vec::new()),
        "a merged shape failure reduces it"
    );
}

// The document says the generation this build writes, whatever it was.
#[test]
fn test_the_loaded_document_is_stamped_with_the_current_generation() {
    let mut map = parse("editor:\n  command: vim\n").unwrap();
    stamp(&mut map);
    assert_eq!(map.get("schema_version").and_then(Value::as_str), Some("8"));
    let mut nothing = Value::Null;
    stamp(&mut nothing);
    assert_eq!(
        nothing.get("schema_version").and_then(Value::as_str),
        Some("8")
    );
    assert!(matches!(nothing, Value::Map(_)));
}
