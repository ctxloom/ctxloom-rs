//! `config.feature` (the read half: `show`, the bare noun, the sections),
//! and the strict decode of a config: schema 8 and older, an unknown key
//! named, a layer that cannot be parsed refused by name.
//!
//! WITHHELD, and why. `config.feature` has two scenarios about "a config value
//! of the wrong type" (`session_reap_age: 5`, "expected string", refused; and
//! `--degraded`, warned). Go reads YAML's own types, so `5` is a number where
//! a string was expected. D15 puts every config through `angzarr-yaml`, which
//! keeps each scalar as its text (it cannot tell `5` from `"5"`), so this
//! build cannot raise that refusal. The two scenarios are not ported; the
//! question is in the S1 report.

use super::library::{contains_value, reports_as, Project};

const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn with_flags(project: &Project, base: &[&str], flags: &[&str]) -> super::Ran {
    let mut args = base.to_vec();
    args.extend_from_slice(flags);
    project.run(&args)
}

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

// config.feature: Scenario Outline: Show renders the whole configuration document
#[test]
fn test_show_renders_the_whole_configuration_document() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = with_flags(&project, &["config", "show"], flags);
        ran.succeeds().contains("claude-code");
        if is_text(flags) {
            ran.contains("llm:")
                .contains("configs:")
                .contains("defaults:");
        } else {
            let doc = ran.json();
            assert_eq!(
                reports_as(&doc, "llm.configs.claude-code.type"),
                "claude-code"
            );
            assert_eq!(
                reports_as(&doc, "llm.configs.claude-fast.type"),
                "claude-code"
            );
            assert_eq!(reports_as(&doc, "llm.defaults.primary"), "claude-code");
        }
    }
}

// config.feature: Scenario Outline: Bare config shows the document instead of
// printing help
#[test]
fn test_bare_config_shows_the_document_instead_of_printing_help() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = with_flags(&project, &["config"], flags);
        ran.succeeds().lacks("Available Commands:");
        if is_text(flags) {
            ran.contains("llm:").contains("defaults:");
        } else {
            let doc = ran.json();
            assert_eq!(
                reports_as(&doc, "llm.configs.claude-code.type"),
                "claude-code"
            );
            assert_eq!(reports_as(&doc, "llm.defaults.primary"), "claude-code");
        }
    }
}

// config.feature: Scenario: The machine-readable rendering carries the
// document, not an empty object
#[test]
fn test_the_machine_readable_rendering_carries_the_document_not_an_empty_object() {
    let project = Project::initialized();
    let ran = project.run(&["--format", "json", "config", "show"]);
    ran.succeeds().contains("claude-code");
    assert!(ran.json().as_object().is_some_and(|o| !o.is_empty()));
}

// config.feature: Scenario Outline: The llm section carries the engine
// registry and nothing above it
#[test]
fn test_the_llm_section_carries_the_engine_registry_and_nothing_above_it() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = with_flags(&project, &["config", "show", "llm"], flags);
        ran.succeeds()
            .contains("claude-code")
            .lacks("editor:")
            .lacks("llm:");
        if is_text(flags) {
            ran.contains("configs:").contains("defaults:");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "configs.claude-code.type"), "claude-code");
            assert_eq!(reports_as(&doc, "defaults.primary"), "claude-code");
        }
    }
}

// config.feature: Scenario: A config carrying an inline profiles block is
// refused, naming the key
#[test]
fn test_a_config_carrying_an_inline_profiles_block_is_refused_naming_the_key() {
    let project = Project::initialized();
    project.bundle("demo");
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\nprofiles:\n  definitions:\n    dev:\n      bundles:\n        - demo\n",
    );
    project
        .run(&["config", "show", "llm"])
        .fails()
        .contains("unknown key `profiles`");
}

// config.feature: Scenario: An unknown section is refused and the real ones
// are named
#[test]
fn test_an_unknown_section_is_refused_and_the_real_ones_are_named() {
    let project = Project::initialized();
    project
        .run(&["config", "show", "nonsense"])
        .fails()
        .contains("nonsense")
        .contains("config")
        .contains("llm");
}

// config.feature: Scenario: A misspelled subcommand fails instead of printing help
#[test]
fn test_a_misspelled_subcommand_fails_instead_of_printing_help() {
    let project = Project::initialized();
    project
        .run(&["config", "shwo"])
        .fails()
        .contains("unknown command")
        .contains("shwo");
}

// fault_tolerance.feature (the config half): a key the schema does not
// describe is refused naming it, with the keys there are; --degraded warns
// and carries on without it.
#[test]
fn test_an_unknown_key_is_refused_naming_it_and_degraded_carries_on() {
    let project = Project::initialized();
    project.write(".ctxloom/config.yaml", "schema_version: 8\nzzz: 1\n");
    let refused = project.run(&["bundle", "list"]);
    refused
        .fails()
        .contains("unknown key `zzz`")
        .contains("known keys at the top level: agents, auth, companions");
    project
        .run(&["--degraded", "bundle", "list"])
        .succeeds()
        .contains("warning");
}

// internal/migrations/config/internal/v8/step_test.go: TestStep_DropsEssenceMaxChars
// and TestStep_EmptiedSectionLeavesAndAbsentKeyIsUntouched, through the
// binary: a schema 7 config loads, and shows as 8 without the key.
#[test]
fn test_a_schema_7_config_migrates_in_memory_dropping_essence_max_chars() {
    let project = Project::initialized();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 7\nconfig:\n  use_distilled: false\n  essence_max_chars: 4096\n",
    );
    let ran = project.run(&["config", "show", "--format", "json"]);
    ran.succeeds().lacks("essence_max_chars");
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "schema_version"), "8");
    assert_eq!(reports_as(&doc, "config.use_distilled"), "false");
    // The file is not touched: persisting an upgrade is --write-upgrades'.
    let kept = std::fs::read_to_string(project.dir().join(".ctxloom/config.yaml")).unwrap();
    assert!(kept.contains("essence_max_chars"), "{kept}");

    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 7\nconfig:\n  essence_max_chars: 4096\n",
    );
    let emptied = project.run(&["config", "show", "--format", "json"]);
    emptied.succeeds();
    assert!(emptied.json().get("config").is_none(), "{}", emptied.stdout);
}

// config_version_refusal_test.go (the stamp): owner override of Go, D15,
// 2026-11-03. Go warned about a layer of another generation and read it as
// written; a config newer than this binary reads, older than it migrates,
// unstamped or stamped with no number is REFUSED, naming the file and the
// generation, with exit 2 and a fix.
#[test]
fn test_a_config_of_another_generation_is_refused() {
    for (config, said) in [
        (
            "schema_version: 9\neditor:\n  command: vim\n",
            "schema_version 9: written by a newer format than this binary reads",
        ),
        (
            "schema_version: 6\neditor:\n  command: vim\n",
            "schema_version 6: older than the oldest format this binary can migrate (the oldest is 7)",
        ),
        (
            "editor:\n  command: vim\n",
            "schema_version 0: older than the oldest format this binary can migrate",
        ),
        (
            "schema_version: abc\n",
            "format version cannot be read: schema_version is not an integer",
        ),
    ] {
        let project = Project::initialized();
        project.write(".ctxloom/config.yaml", config);
        let ran = project.run(&["config", "show", "editor", "--format", "json"]);
        assert_eq!(ran.code, 2, "{config:?}: {}", ran.said());
        ran.contains(said);
        assert!(ran.stdout.is_empty(), "no answer is given: {}", ran.stdout);
        project
            .run(&["config", "show", "--format", "text"])
            .contains("  fix: ");
    }
}

// cli/config.feature, configload/merge.go: layering is home < project, an
// agent binding replaces the home's of the same name wholesale, companions
// merge by name, a scalar of the project wins.
#[test]
fn test_layers_merge_home_under_project() {
    let project = Project::from_fixture("cfg/layers");
    let doc = project
        .run(&["config", "show", "--format", "json"])
        .succeeds()
        .json();
    assert_eq!(reports_as(&doc, "editor.command"), "project-editor");
    assert!(contains_value(&doc, "editor.args", "-h"), "{doc}");
    assert_eq!(reports_as(&doc, "workspace"), "worktree");
    assert_eq!(reports_as(&doc, "agents.proj.llm"), "claude-code");
    assert!(contains_value(&doc, "agents.proj.profiles", "x"), "{doc}");
    assert!(contains_value(&doc, "agents.home.profiles", "h"), "{doc}");
    assert_eq!(
        doc["companions"],
        serde_json::json!(["acme", "beta", "gamma"])
    );
}

// confload/overlay.go: --config-set <path>=<value> beats the files, and
// CTXLOOM_CONFIG_<PATH> beats them too; the flag beats the variable.
#[test]
fn test_overrides_beat_the_files_and_the_flag_beats_the_variable() {
    let project = Project::initialized();
    let env = [("CTXLOOM_CONFIG_EDITOR_COMMAND", "nano")];
    let from_env = project.run_with(&env, &["config", "show", "editor", "--format", "json"]);
    assert_eq!(reports_as(&from_env.succeeds().json(), "command"), "nano");
    let both = project.run_with(
        &env,
        &[
            "--config-set",
            "editor.command=vim",
            "config",
            "show",
            "editor",
            "--format",
            "json",
        ],
    );
    assert_eq!(reports_as(&both.succeeds().json(), "command"), "vim");
    // An entry with no "=" is warned about and the rest is read.
    project
        .run(&[
            "--config-set",
            "noequals",
            "config",
            "show",
            "--format",
            "json",
        ])
        .succeeds()
        .contains("expected <path>=<value>");
}

// configload/appdir.go + docs/environment.md: a project with no .ctxloom of
// its own anywhere up the path reads the home's config alone, and nothing is
// created.
#[test]
fn test_without_a_project_the_home_config_is_the_whole_configuration() {
    let project = Project::bare();
    project.write_home(
        ".ctxloom/config.yaml",
        "schema_version: 8\neditor:\n  command: nano\n",
    );
    let ran = project.run_in(
        project.dir(),
        &["config", "show", "editor", "--format", "json"],
    );
    assert_eq!(reports_as(&ran.succeeds().json(), "command"), "nano");
    assert!(
        !project.dir().join(".ctxloom").exists(),
        "reading creates nothing"
    );
}
