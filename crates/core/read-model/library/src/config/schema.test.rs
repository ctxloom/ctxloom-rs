//! The config schema (`resources/schema/input/config-schema.json` at
//! 860336c4e) as the load words it, on the cases the Go acceptance and
//! `configload` tests refuse: the messages are Go's.

use super::*;

fn parse(text: &str) -> Value {
    angzarr_yaml::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

fn texts(doc: &str) -> Vec<String> {
    validate(&parse(doc))
        .iter()
        .map(|v| v.text("config.yaml"))
        .collect()
}

// the fixture `cfg/v8`: every key of the schema, correctly used
#[test]
fn test_a_config_using_every_key_correctly_has_no_violation() {
    let doc = "schema_version: 8\neditor:\n  command: vim\n  args: [-u, NONE]\nconfig:\n  use_distilled: false\n  silence_unsupported: true\n  statusline: false\nsync:\n  auto_sync: false\nworkspace: worktree\nruntime: host\ndelegation:\n  concurrency: 3\n  depth: 2\n  idle_timeout: 5m\nshell_timeout:\n  default: 30s\n  max: 1h\nisolation_images:\n  claude-code: example.com/img:1\nisolation_base: ctxloom\nisolation_memory_limit: 4g\nisolation_pids_limit: 512\noutput_dir: /var/out\nisolation_engines: [claude-code]\ncompanions: [acme, beta]\nui:\n  prefix_key: ctrl-b\n  list_window: 7d\nsession_reap_age: 30d\nsession_purge_age: 90d\nsession_transcript_retention: 365d\nnon_tty_format: json\nauth: login\nagents:\n  dev:\n    llm: claude-code\n    profiles: [p1]\n    runtime: host\n    driving: oneshot\n    env_host: false\n    env: [ALPHA]\n    permissions:\n      approver: human\n      claude-code:\n        mode: plan\ndefault_agent: dev\nllm:\n  configs:\n    claude-code:\n      type: claude-code\n      model: opus\n  defaults:\n    primary: claude-code\n";
    assert_eq!(texts(doc), Vec::<String>::new());
}

// fault_tolerance / config.feature: an unknown key at the top level is named,
// with the keys there are
#[test]
fn test_an_unknown_top_level_key_is_named_with_the_keys_there_are() {
    assert_eq!(
        texts("schema_version: 8\nzzz: 1\n"),
        ["unknown key `zzz` in config.yaml: ctxloom does not know it (known keys at the top level: agents, auth, companions, config, default_agent, delegation, editor, isolation_base, isolation_devcontainer_service, isolation_engines, isolation_images, isolation_memory_limit, isolation_pids_limit, llm, non_tty_format, output_dir, permissions, runtime, schema_version, session_purge_age, session_reap_age, session_transcript_retention, shell_timeout, sync, ui, workspace)"]
    );
    // The inline profiles block, the native mcp block and the retired turn cap
    // are keys the schema does not describe.
    for key in ["profiles", "mcp", "agent_turn_cap"] {
        let said = texts(&format!("schema_version: 8\n{key}:\n  x: 1\n"));
        assert!(
            said[0].starts_with(&format!("unknown key `{key}` in config.yaml")),
            "{said:?}"
        );
    }
}

// agent.feature: A binding still carrying auth is refused, naming the key
#[test]
fn test_an_unknown_nested_key_is_named_by_its_path_and_where_it_would_be_legal() {
    assert_eq!(
        texts("schema_version: 8\nagents:\n  developer:\n    profiles: [dev]\n    auth: login\n"),
        ["unknown key `agents.developer.auth` in config.yaml: ctxloom does not know it (known keys at `agents.developer`: driving, engine_home, env, env_host, llm, may_delegate, permissions, profiles, roots, runtime, surfaces)"]
    );
    // Go: the project's permissions take only the neutral keys.
    assert_eq!(
        texts("schema_version: 8\npermissions:\n  mode: plan\n"),
        ["unknown key `permissions.mode` in config.yaml: ctxloom does not know it (known keys at `permissions`: approval_timeout, approver, network, sandbox)"]
    );
}

// the fixtures `cfg/bad-enum` and `cfg/bad-delegation`, and the Go messages
#[test]
fn test_a_value_the_schema_refuses_is_worded_as_go_words_it() {
    assert_eq!(
        texts("schema_version: 8\nruntime: laptop\n"),
        ["config.yaml: `runtime` breaks the schema: value must be one of \"host\", \"container-rootless\", \"container-rootful\""]
    );
    assert_eq!(
        texts("schema_version: 8\ndelegation:\n  concurrency: 0\n"),
        ["config.yaml: `delegation.concurrency` breaks the schema: must be >= 1 but found 0"]
    );
    assert_eq!(
        texts("schema_version: 8\nui:\n  prefix_key: xx\n"),
        ["config.yaml: `ui.prefix_key` breaks the schema: does not match pattern '^ctrl[-+].$'"]
    );
    assert_eq!(
        texts("schema_version: 8\nsession_reap_age: soon\n"),
        ["config.yaml: `session_reap_age` breaks the schema: does not match pattern '^[0-9]+(d|w|h|m|s)$'"]
    );
    assert_eq!(
        texts("schema_version: 8\nisolation_pids_limit: lots\n"),
        ["config.yaml: `isolation_pids_limit` breaks the schema: matches none of the shapes the schema allows here"]
    );
    assert_eq!(
        texts("schema_version: 8\nllm:\n  configs:\n    x:\n      type: nonesuch\n"),
        ["config.yaml: `llm.configs.x` breaks the schema: matches none of the shapes the schema allows here"]
    );
}

// the wrong-shape messages: what the key holds, and what it should
#[test]
fn test_a_value_of_the_wrong_shape_says_what_it_holds_and_what_was_expected() {
    assert_eq!(
        texts("schema_version: 8\ndelegation:\n  concurrency: abc\n"),
        ["config.yaml: `delegation.concurrency` holds a string, expected integer"]
    );
    assert_eq!(
        texts("schema_version: 8\nconfig:\n  use_distilled: maybe\n"),
        ["config.yaml: `config.use_distilled` holds a string, expected boolean"]
    );
    assert_eq!(
        texts("schema_version: 8\neditor:\n  args: x\n"),
        ["config.yaml: `editor.args` holds a string, expected array"]
    );
    assert_eq!(
        texts("schema_version: 8\ncompanions: foo\n"),
        ["config.yaml: `companions` holds a string, expected array"]
    );
    assert_eq!(
        texts("schema_version: 8\nagents:\n  d:\n    profiles: dev\n"),
        ["config.yaml: `agents.d.profiles` holds a string, expected array"]
    );
    assert_eq!(
        texts("schema_version: 8\nagents: [x]\n"),
        ["config.yaml: `agents` holds a array, expected object"]
    );
    assert!(validate(&parse("schema_version: 8\nagents: [x]\n"))[0].is_shape());
    assert!(!validate(&parse("schema_version: 8\nruntime: laptop\n"))[0].is_shape());
}

// the schema's four duration and age patterns, and the one that is any of two shapes
#[test]
fn test_durations_ages_and_limits_take_the_shapes_the_schema_gives_them() {
    assert!(texts("delegation:\n  idle_timeout: 1h30m\n").is_empty());
    assert!(texts("delegation:\n  idle_timeout: 1.5h\n").is_empty());
    assert_eq!(texts("delegation:\n  idle_timeout: soon\n").len(), 1);
    assert!(texts("ui:\n  list_window: all\n").is_empty());
    assert!(texts("ui:\n  list_window: 0\n").is_empty());
    assert!(texts("ui:\n  list_window: 12h\n").is_empty());
    assert_eq!(texts("ui:\n  list_window: lots\n").len(), 1);
    assert!(texts("isolation_memory_limit: unlimited\n").is_empty());
    assert!(texts("isolation_pids_limit: 0\n").is_empty());
    assert!(texts("isolation_pids_limit: unlimited\n").is_empty());
    assert!(texts("non_tty_format: json\n").is_empty());
    assert!(texts("non_tty_format: text\n").is_empty());
    assert_eq!(texts("non_tty_format: pdf\n").len(), 1);
}

// plan supersedes Go: D28 (json and text only); Go's schema also allows yaml, toml and markdown
#[test]
fn test_a_dropped_output_format_is_not_a_non_tty_format() {
    for dropped in ["yaml", "toml", "markdown"] {
        assert_eq!(
            texts(&format!("non_tty_format: {dropped}\n")).len(),
            1,
            "{dropped} is dropped"
        );
    }
}

// overlay.go KnownPath: the schema's paths, a label under a map is free
#[test]
fn test_known_paths() {
    let p = |s: &str| s.split('.').map(ToString::to_string).collect::<Vec<_>>();
    assert!(known_path(&p("editor.command")));
    assert!(known_path(&p("llm.defaults.primary")));
    assert!(known_path(&p("agents.MyCoder.runtime")));
    assert!(known_path(&p(
        "agents.anything.permissions.claude-code.mode"
    )));
    assert!(known_path(&p("default_agent")));
    assert!(!known_path(&p("nonsense.x")));
    assert!(!known_path(&p("editor.nope")));
}

// overlay.go ValidateAt: an override value is checked at its path
#[test]
fn test_an_override_value_is_checked_at_its_path() {
    let p = |s: &str| s.split('.').map(ToString::to_string).collect::<Vec<_>>();
    assert!(validate_at(&p("runtime"), &Value::Scalar("host".into())).is_ok());
    let err = validate_at(&p("runtime"), &Value::Scalar("laptop".into())).expect_err("laptop");
    assert!(err.contains("value must be one of"), "{err}");
    assert!(validate_at(&p("delegation.concurrency"), &Value::Scalar("0".into())).is_err());
    assert!(validate_at(
        &p("companions"),
        &Value::Seq(vec![Value::Scalar("a".into())])
    )
    .is_ok());
}

// config.go resolveConfigSection: the sections, in the order `config show
// nonsense` names them
#[test]
fn test_the_sections_are_the_top_level_keys_in_go_s_order() {
    assert_eq!(SECTIONS[0], "schema_version");
    assert_eq!(SECTIONS[1], "llm");
    assert_eq!(SECTIONS.len(), 26);
    assert_eq!(SECTIONS[25], "auth");
}

fn llm_texts(body: &str) -> Vec<String> {
    texts(&format!(
        "llm:\n  configs:\n    a:\n      {}",
        body.replace("\n    ", "\n      ")
    ))
}

// config-schema.json, llm.configs: the shape an entry's `type` names, and what
// the shapes require of each other (a table of the entries and the refusals)
#[test]
fn test_an_llm_entry_is_judged_by_the_shape_its_type_names() {
    let none = "config.yaml: `llm.configs.a` breaks the schema: matches none of the shapes the schema allows here";
    let unknown = |key: &str| {
        format!("unknown key `llm.configs.a.{key}` in config.yaml: ctxloom does not know it (known keys at `llm.configs.a`: args, binary_path, context_window, mock_control, model, model_options, permissions, provider, role, share, snapshot, title_generation, type)")
    };
    for ok in [
        "type: claude-code\n    binary_path: x\n    args: [a]\n",
        "type: opencode\n    model: m\n",
        "type: opencode\n    model: m\n    context_window: 4\n    provider:\n      base_url: https://x\n      output_limit: 5\n      header_timeout_ms: 0\n      chunk_timeout_ms: 1\n    model_options:\n      a: 1\n    share: auto\n    snapshot: true\n",
        "type: mock\n    mock_control:\n      a: b\n",
        "type: mock\n    context_window: 1\n",
    ] {
        assert_eq!(llm_texts(ok), Vec::<String>::new(), "{ok:?}");
    }
    for (body, want) in [
        (
            "type: opencode\n",
            vec!["config.yaml: `llm.configs.a` breaks the schema: missing properties: 'model'".to_string()],
        ),
        ("model: m\n", vec![none.to_string()]),
        ("binary_path: x\n", vec![none.to_string()]),
        ("type: bogus\n    binary_path: x\n", vec![none.to_string()]),
        ("type: claude-code\n    provider:\n      base_url: x\n", vec![none.to_string()]),
        ("type: claude-code\n    bogus: 1\n", vec![unknown("bogus")]),
        ("type: bogus\n    zzz: x\n", vec![unknown("zzz")]),
        ("zzz: x\n", vec![unknown("zzz")]),
        (
            "type: opencode\n    model: m\n    provider:\n      base_url: https://x\n      output_limit: 5\n",
            vec!["config.yaml: `llm.configs.a` breaks the schema: properties 'context_window' required, if 'provider' property exists".to_string()],
        ),
        (
            "type: opencode\n    model: m\n    context_window: 4\n    provider:\n      base_url: https://x\n",
            vec!["config.yaml: `llm.configs.a.provider` breaks the schema: missing properties: 'output_limit'".to_string()],
        ),
        (
            "type: mock\n    context_window: 0\n",
            vec!["config.yaml: `llm.configs.a.context_window` breaks the schema: must be >= 1 but found 0".to_string()],
        ),
        (
            "type: opencode\n    model: m\n    context_window: 1\n    provider:\n      base_url: https://x\n      output_limit: 0\n      header_timeout_ms: -1\n      chunk_timeout_ms: 0\n",
            vec![
                "config.yaml: `llm.configs.a.provider.output_limit` breaks the schema: must be >= 1 but found 0".to_string(),
                "config.yaml: `llm.configs.a.provider.header_timeout_ms` breaks the schema: must be >= 0 but found -1".to_string(),
                "config.yaml: `llm.configs.a.provider.chunk_timeout_ms` breaks the schema: must be >= 1 but found 0".to_string(),
            ],
        ),
        (
            "type: mock\n    role: primary\n    permissions:\n      network: maybe\n",
            vec!["config.yaml: `llm.configs.a.permissions.network` holds a string, expected boolean".to_string()],
        ),
    ] {
        assert_eq!(llm_texts(body), want, "{body:?}");
    }
    assert_eq!(
        texts("llm:\n  configs:\n    a: nope\n"),
        ["config.yaml: `llm.configs.a` holds a string, expected object"]
    );
}

// the string patterns the schema writes, each accepting and refusing
#[test]
fn test_each_string_pattern_accepts_and_refuses() {
    let cases: [(Pat, &[&str], &[&str]); 7] = [
        (
            Pat::PrefixKey,
            &["ctrl-b", "ctrl+x"],
            &["xxxx-b", "ctrlxb", "ctrl-bb", "ctrl-", "ctrl-\n", "Ctrl-b"],
        ),
        (
            Pat::ListWindow,
            &["all", "0", "12h", "1d", "3w"],
            &["none", "00", "h", "12", "1y"],
        ),
        (
            Pat::Age,
            &["1d", "30m", "9s", "12w", "5h"],
            &["d", "12", "12x", "", "1.5d"],
        ),
        (
            Pat::PidsLimit,
            &["512", "  8 ", "unlimited", "UNLIMITED"],
            &["", " ", "a1", "-1", "1.5"],
        ),
        (
            Pat::Companion,
            &["acme", "a1.b_c-d", "1x"],
            &["", "-a", ".a", "a b", "a/b"],
        ),
        (Pat::NoEquals, &["a", "a b"], &["", "a=b", "a\nb"]),
        (
            Pat::Url,
            &["http://x", "https://x"],
            &["ftp://x", "http:/", "", "https:/x"],
        ),
    ];
    for (pat, good, bad) in cases {
        for s in good {
            assert!(pat.matches(s), "{pat:?} accepts {s:?}");
        }
        for s in bad {
            assert!(!pat.matches(s), "{pat:?} refuses {s:?}");
        }
    }
}

// the memory limit: a number, a binary or decimal unit, or `unlimited`
#[test]
fn test_a_memory_limit_is_a_number_with_a_unit_or_unlimited() {
    for ok in [
        "4g",
        "4G",
        "4GiB",
        "4 g",
        " 4g ",
        "1.5gb",
        "unlimited",
        "UNLIMITED",
        "512",
        "512b",
        "512B",
        "2kb",
        "3TIB",
    ] {
        assert!(Pat::MemoryLimit.matches(ok), "{ok:?}");
    }
    for bad in [
        "2Ki", "g", "4x", "4gbb", "4.", "4.g", "", "ib", "4 ii", "4bb", "-4g", "4gi b",
    ] {
        assert!(!Pat::MemoryLimit.matches(bad), "{bad:?}");
    }
}

// overlay.go KnownPath: where the schema describes a path
#[test]
fn test_the_schema_describes_exactly_the_paths_it_lists() {
    let path = |p: &[&str]| -> Vec<String> { p.iter().map(ToString::to_string).collect() };
    for known in [
        &["companions", "0"][..],
        &["editor", "command"],
        &["llm", "configs", "x", "binary_path"],
        &["llm", "configs", "x", "provider", "base_url"],
        &["llm", "configs", "x", "model_options", "any", "thing"],
        &["agents", "dev", "env", "0"],
    ] {
        assert!(known_path(&path(known)), "{known:?}");
    }
    for unknown in [
        &["companions", "x"][..],
        &["editor", "bogus"],
        &["agents", "dev", "nope"],
        &["llm", "configs", "x", "nope"],
    ] {
        assert!(!known_path(&path(unknown)), "{unknown:?}");
    }
}

// the validator's names for what a scalar reads as
#[test]
fn test_a_scalar_reads_as_the_type_the_validator_names() {
    for (text, kind) in [
        ("1", "number"),
        ("-1", "number"),
        ("1.5", "number"),
        ("1e3", "number"),
        (".5", "number"),
        ("true", "boolean"),
        ("false", "boolean"),
        ("True", "string"),
        ("abc", "string"),
        ("NaN", "string"),
        ("inf", "string"),
    ] {
        assert_eq!(type_of_scalar(text), kind, "{text}");
    }
}
