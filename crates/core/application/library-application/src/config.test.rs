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
// after the overrides) is the defaults alone. Go parity, not a defect: Go's
// Config.UnmarshalYAML (internal/core/config/config.go:449) returns the
// decode error before fromDoc, so nothing of the document is applied, and
// Builder.Decode (reading.go:64) only warns; an override's own key vanishes
// with the rest.
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

// ---------------------------------------------------------------- characterization
//
// Pinned before the config intake may move (A0): what loading says of the
// overrides it was given, in which words, and which of them stop it.

mod loading {
    use super::super::{load, Flags};
    use crate::error::Notice;
    use crate::fakes::Setup;
    use ctxloom_ports::ConfigFiles;

    const STAMPED: &str = "schema_version: 8\neditor:\n  command: vim\n";

    fn loaded(
        layers: &[&str],
        environ: &[(&str, &str)],
        flags: &Flags,
    ) -> (angzarr_yaml::Value, Vec<String>) {
        let (_, mem) = Setup {
            layers,
            ..Setup::default()
        }
        .fixture();
        let environ: Vec<(String, String)> = environ
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let mut notices = Vec::new();
        let files: &dyn ConfigFiles = mem.as_ref();
        let config = load(files, &environ, flags, &mut notices).expect("loaded");
        let said = notices
            .into_iter()
            .map(|n| match n {
                Notice::Finding { text, remedy } => format!("{text} | {remedy}"),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        (config.raw, said)
    }

    fn flags_with(sets: &[&str]) -> Flags {
        Flags {
            config_set: sets.iter().map(ToString::to_string).collect(),
            ..Flags::default()
        }
    }

    #[test]
    fn test_a_blank_config_set_is_said_and_the_rest_still_apply() {
        let (value, said) = loaded(
            &[STAMPED],
            &[],
            &flags_with(&["=blank", "editor.command=nano", "bare"]),
        );
        assert_eq!(
            said,
            [
                "config override resolution: --config-set \"=blank\": empty path before \"=\" | ",
                "config override resolution: --config-set \"bare\": expected <path>=<value> | ",
            ]
        );
        assert_eq!(
            value
                .get("editor")
                .and_then(|e| e.get("command"))
                .and_then(angzarr_yaml::Value::as_str),
            Some("nano")
        );
    }

    #[test]
    fn test_an_unknown_override_warns_in_its_words_and_is_set_anyway() {
        let (value, said) = loaded(
            &[STAMPED],
            &[("CTXLOOM_CONFIG_NO_SUCH_KEY", "x"), ("HOME", "/h")],
            &Flags::default(),
        );
        assert_eq!(
            said,
            ["CTXLOOM_CONFIG_NO_SUCH_KEY does not match any known config key (resolved as NO.SUCH.KEY); setting it anyway | "]
        );
        assert!(value.get("NO").is_some(), "applied anyway");
    }

    #[test]
    fn test_a_value_the_schema_refuses_is_said_and_the_config_is_not_dropped() {
        let (value, said) = loaded(
            &[STAMPED],
            &[("CTXLOOM_CONFIG_NON_TTY_FORMAT", "yaml")],
            &Flags::default(),
        );
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].starts_with(
                "config override resolution: CTXLOOM_CONFIG_NON_TTY_FORMAT: env set \"non_tty_format\""
            ),
            "{said:?}"
        );
        assert_eq!(
            value
                .get("non_tty_format")
                .and_then(angzarr_yaml::Value::as_str),
            Some("yaml"),
            "set all the same"
        );
    }

    #[test]
    fn test_no_override_leaves_the_layers_and_says_nothing() {
        let (value, said) = loaded(&[STAMPED], &[("HOME", "/h")], &Flags::default());
        assert!(said.is_empty());
        assert_eq!(
            value,
            angzarr_yaml::parse(STAMPED).expect("a document"),
            "stamped as it was"
        );
    }

    #[test]
    fn test_the_flag_beats_the_environment_beats_the_layers() {
        let (value, _) = loaded(
            &[STAMPED],
            &[("CTXLOOM_CONFIG_EDITOR_COMMAND", "emacs")],
            &Flags::default(),
        );
        let command = |v: &angzarr_yaml::Value| {
            v.get("editor")
                .and_then(|e| e.get("command"))
                .and_then(angzarr_yaml::Value::as_str)
                .map(ToString::to_string)
        };
        assert_eq!(command(&value).as_deref(), Some("emacs"));
        let (value, _) = loaded(
            &[STAMPED],
            &[("CTXLOOM_CONFIG_EDITOR_COMMAND", "emacs")],
            &flags_with(&["editor.command=nano"]),
        );
        assert_eq!(command(&value).as_deref(), Some("nano"));
    }
}
