//! Ported from Go's `configload` tests (`layering_test.go`, `merge_test.go`,
//! `config_version_refusal_test.go`), `internal/migrations/config/internal/v8`
//! and `confload` (`TestLoad_WarnsWhenAConfigFileExistsButDefinesNoKeys`) at
//! 860336c4e.

use super::*;
use crate::config::{CURRENT, OLDEST};

angzarr_presentation::english_for!(&[crate::EN_US.1]);

fn parse(text: &str) -> Value {
    angzarr_yaml::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

fn merged(layers: &[&str]) -> Value {
    let layers: Vec<Value> = layers.iter().map(|l| parse(l)).collect();
    merge_layers(&layers)
}

fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |here, key| here.get(key))
}

fn text<'a>(v: &'a Value, path: &str) -> &'a str {
    get(v, path)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no {path} in {v:?}"))
}

fn seq(v: &Value, path: &str) -> Vec<String> {
    get(v, path)
        .and_then(Value::as_seq)
        .unwrap_or_else(|| panic!("no list {path}"))
        .iter()
        .filter_map(|e| e.as_str().map(ToString::to_string))
        .collect()
}

// layering_test.go: TestLoad_ProjectInheritsHomeKeys / _ProjectOverridesHomeKey
#[test]
fn test_the_project_inherits_the_homes_keys_and_overrides_the_ones_it_sets() {
    let v = merged(&[
        "schema_version: 8\neditor:\n  command: home-editor\nworkspace: worktree\n",
        "schema_version: 8\neditor:\n  command: project-editor\n",
    ]);
    assert_eq!(text(&v, "editor.command"), "project-editor");
    assert_eq!(text(&v, "workspace"), "worktree");
}

// layering_test.go: TestLoad_HomeAndProjectDeepMergeNestedSection
#[test]
fn test_home_and_project_deep_merge_a_nested_section() {
    let v = merged(&[
        "schema_version: 8\ndelegation:\n  concurrency: 3\n  depth: 2\n",
        "schema_version: 8\ndelegation:\n  depth: 5\n",
    ]);
    assert_eq!(text(&v, "delegation.concurrency"), "3");
    assert_eq!(text(&v, "delegation.depth"), "5");
}

// merge_test.go: TestLoad_ProjectAgentBindingReplacesHomesWholesale and
// TestAgentBindingMergeFunc_ReplacesWholesale / _AgentNamedOnlyByLowerLayerSurvives
#[test]
fn test_a_project_agent_binding_replaces_the_homes_of_the_same_name_wholesale() {
    let v = merged(&[
        "schema_version: 8\nagents:\n  coder:\n    llm: claude-code\n    profiles: [h]\n    runtime: host\n  home-only:\n    llm: claude-fast\n",
        "schema_version: 8\nagents:\n  coder:\n    profiles: [p]\n",
    ]);
    assert_eq!(seq(&v, "agents.coder.profiles"), ["p"]);
    assert!(
        get(&v, "agents.coder.llm").is_none(),
        "no field of the home's binding survives"
    );
    assert!(get(&v, "agents.coder.runtime").is_none());
    assert_eq!(
        text(&v, "agents.home-only.llm"),
        "claude-fast",
        "an agent only the lower layer names survives"
    );
}

// merge_test.go: TestAgentBindingMergeFunc_NonAgentKeysStillDeepMerge
#[test]
fn test_keys_other_than_agents_still_deep_merge() {
    let v = merged(&[
        "schema_version: 8\nllm:\n  defaults:\n    primary: a\nagents:\n  x:\n    llm: l\n",
        "schema_version: 8\nllm:\n  defaults:\n    fast: b\n",
    ]);
    assert_eq!(text(&v, "llm.defaults.primary"), "a");
    assert_eq!(text(&v, "llm.defaults.fast"), "b");
    assert_eq!(text(&v, "agents.x.llm"), "l");
}

// merge_test.go: TestLayerMergeFunc_CompanionsUnionAcrossLayers /
// _ProjectCannotRemoveAHomeCompanion / TestLoad_ProjectCompanionsAddToHomes
#[test]
fn test_companions_union_across_layers_and_a_project_cannot_remove_one() {
    let v = merged(&["companions: [acme, beta]\n", "companions: [beta, gamma]\n"]);
    assert_eq!(seq(&v, "companions"), ["acme", "beta", "gamma"]);
    let kept = merged(&["companions: [acme]\n", "companions: []\n"]);
    assert_eq!(
        seq(&kept, "companions"),
        ["acme"],
        "an empty project list does not unregister"
    );
}

// merge_with: lists the higher layer sets replace the lower's (koanf's default)
#[test]
fn test_other_lists_are_replaced_not_joined() {
    let v = merged(&["editor:\n  args: [a, b]\n", "editor:\n  args: [c]\n"]);
    assert_eq!(seq(&v, "editor.args"), ["c"]);
}

// reading.go OverlayDefaultRegistry + resources/default-config.yaml
#[test]
fn test_the_default_registry_fills_an_llm_section_that_declares_none() {
    let mut doc = parse("schema_version: 8\nllm:\n  defaults:\n    primary: nope\n");
    overlay_default_registry(&mut doc);
    assert_eq!(text(&doc, "llm.configs.claude-code.type"), "claude-code");
    assert_eq!(text(&doc, "llm.configs.claude-code.model"), "opus");
    assert_eq!(text(&doc, "llm.configs.claude-code.role"), "primary");
    assert_eq!(text(&doc, "llm.configs.claude-fast.model"), "haiku");
    assert_eq!(
        text(&doc, "llm.defaults.primary"),
        "nope",
        "a declared default is kept"
    );
    assert_eq!(
        text(&doc, "llm.defaults.fast"),
        "claude-fast",
        "a missing one is filled"
    );

    let mut bare = parse("schema_version: 8\n");
    overlay_default_registry(&mut bare);
    assert_eq!(text(&bare, "llm.defaults.primary"), "claude-code");
}

// reading.go OverlayDefaultRegistry: any llm.configs of the config's own and
// the registry is not laid under it
#[test]
fn test_a_config_that_declares_llm_configs_is_left_as_it_is() {
    let mut doc = parse("llm:\n  configs:\n    big:\n      type: mock\n      model: o1\n");
    overlay_default_registry(&mut doc);
    assert!(get(&doc, "llm.configs.claude-code").is_none());
    assert!(get(&doc, "llm.defaults").is_none());
}

// config_version_refusal_test.go: a generation this build cannot migrate is a
// note naming it, and the layer is read as written
#[test]
fn test_a_newer_older_or_unreadable_stamp_is_a_note_and_the_layer_is_read_as_written() {
    let newer = read("schema_version: 9\neditor:\n  command: vim\n").unwrap();
    assert_eq!(
        newer.notes,
        [Note::Newer {
            found: 9,
            current: u64::from(CURRENT)
        }]
    );
    assert_eq!(text(&newer.value, "editor.command"), "vim");

    let older = read("schema_version: 6\neditor:\n  command: vim\n").unwrap();
    assert_eq!(
        older.notes,
        [Note::Older {
            found: 6,
            oldest: u64::from(OLDEST)
        }]
    );

    let unstamped = read("editor:\n  command: vim\n").unwrap();
    assert_eq!(
        unstamped.notes,
        [Note::Older {
            found: 0,
            oldest: u64::from(OLDEST)
        }]
    );

    let not_int = read("schema_version: abc\n").unwrap();
    assert_eq!(not_int.notes, [Note::NotInteger]);

    let current = read("schema_version: 8\neditor:\n  command: vim\n").unwrap();
    assert!(current.notes.is_empty());
}

// v8/step_test.go: TestStep_DropsEssenceMaxChars and
// TestStep_EmptiedSectionLeavesAndAbsentKeyIsUntouched
#[test]
fn test_generation_7_drops_essence_max_chars_and_the_section_it_empties() {
    let r =
        read("schema_version: 7\nconfig:\n    use_distilled: false\n    essence_max_chars: 4096\n")
            .unwrap();
    assert!(r.notes.is_empty(), "7 is migrated, not noted");
    assert_eq!(text(&r.value, "config.use_distilled"), "false");
    assert!(get(&r.value, "config.essence_max_chars").is_none());
    assert_eq!(
        text(&r.value, "schema_version"),
        "8",
        "the stamp follows the migration"
    );

    let emptied = read("schema_version: 7\nconfig:\n    essence_max_chars: 4096\n").unwrap();
    assert!(
        get(&emptied.value, "config").is_none(),
        "a section the key alone filled leaves with it"
    );

    let untouched = read("schema_version: 7\nworkspace: worktree\n").unwrap();
    assert_eq!(text(&untouched.value, "workspace"), "worktree");
    // Not a mapping: left for the strict decode to refuse, never dropped.
    let odd = read("schema_version: 7\nconfig: [x]\n").unwrap();
    assert!(get(&odd.value, "config").and_then(Value::as_seq).is_some());
}

// confload_test.go: TestLoad_WarnsWhenAConfigFileExistsButDefinesNoKeys and
// _DoesNotWarnForAbsentOrPopulatedLayers
#[test]
fn test_a_file_with_no_keys_is_a_note() {
    for empty in ["", "# only a comment\n"] {
        let r = read(empty).unwrap();
        assert!(r.notes.contains(&Note::Keyless), "{empty:?}: {:?}", r.notes);
    }
    assert!(!read("schema_version: 8\n")
        .unwrap()
        .notes
        .contains(&Note::Keyless));
}

// configload layers.go loadConfigLayer: text that is not YAML is refused
#[test]
fn test_text_that_is_not_yaml_is_refused() {
    let err = read("::: not: [valid yaml\n\tbroken").expect_err("not YAML");
    assert!(
        matches!(err, ConfigError::Syntax { .. }),
        "{}",
        err.english()
    );
}

// merge_test.go: a null in a higher layer leaves the lower layer's value, and
// adds no key of its own
#[test]
fn test_a_null_in_a_higher_layer_neither_replaces_nor_adds() {
    let m = merged(&["workspace: worktree\n", "workspace:\nsession_reap_age:\n"]);
    assert_eq!(text(&m, "workspace"), "worktree");
    assert!(get(&m, "session_reap_age").is_none(), "{m:?}");
}
