//! Go's acceptance scenarios for the llm noun's write side (S2),
//! `tests/acceptance/features/cli/llm.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. Answers and the config's bytes are
//! Go's own (captured from a Go ctxloom run in a scratch project).
//!
//! The config write is a section merge with sorted keys: `llm.configs` is
//! re-rendered with its labels and their keys sorted, and nothing else in the
//! file moves.

use super::library::{reports_as, Project};

const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn with_flags(project: &Project, base: &[&str], flags: &[&str]) -> super::Ran {
    let mut args = base.to_vec();
    args.extend_from_slice(flags);
    project.run(&args)
}

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

const CONFIG: &str = ".ctxloom/config.yaml";

// llm.feature: Scenario: A project with no llm block still reports the
// default it resolves to
#[test]
fn test_a_project_with_no_llm_block_still_reports_the_default_it_resolves_to() {
    let project = Project::initialized();
    project
        .run(&["llm", "default"])
        .succeeds()
        .contains("claude-code");
    let doc = project.run(&["llm", "default", "--format", "json"]).json();
    assert_eq!(reports_as(&doc, "default"), "claude-code");
    assert_eq!(
        project.run(&["llm", "default", "--format", "text"]).stdout,
        "claude-code\n"
    );
}

// llm.feature: Scenario Outline: Setting the default moves the marker in the
// listing
#[test]
fn test_setting_the_default_moves_the_marker_in_the_listing() {
    for flags in FORMATS {
        let project = Project::initialized();
        let set = with_flags(&project, &["llm", "default", "claude-fast"], flags);
        set.succeeds();
        if is_text(flags) {
            assert_eq!(set.stdout, "Default LLM set to: claude-fast\n");
        } else {
            assert_eq!(reports_as(&set.json(), "status"), "set");
            assert_eq!(reports_as(&set.json(), "name"), "claude-fast");
        }
        let read = with_flags(&project, &["llm", "default"], flags);
        read.succeeds();
        if is_text(flags) {
            read.contains("claude-fast");
        } else {
            assert_eq!(reports_as(&read.json(), "default"), "claude-fast");
        }
        let listed = with_flags(&project, &["llm", "list"], flags);
        if is_text(flags) {
            listed.contains("claude-fast (default)");
        } else {
            assert_eq!(
                reports_as(&listed.json(), "[label=claude-fast].default"),
                "true"
            );
        }
        // Only the key moved: llm.defaults.primary, in a section of its own.
        project.file_contains(CONFIG, "primary: claude-fast");
        let again = with_flags(&project, &["llm", "default", "claude-fast"], flags);
        if is_text(flags) {
            assert_eq!(again.stdout, "Default LLM is already claude-fast\n");
        } else {
            assert_eq!(reports_as(&again.json(), "status"), "unchanged");
        }
    }
}

// llm.feature: Scenario: An unknown label is refused, and the old default
// stands
#[test]
fn test_an_unknown_label_is_refused_and_the_old_default_stands() {
    let project = Project::initialized();
    let before = project.read(CONFIG);
    project
        .run(&["llm", "default", "nosuchengine"])
        .fails()
        .contains("unknown LLM")
        .contains("claude-code");
    assert_eq!(project.read(CONFIG), before);
    project
        .run(&["llm", "default"])
        .succeeds()
        .contains("claude-code");
}

// llm.feature: Scenario Outline: A labeled engine config is created and read
// back out of the project file
#[test]
fn test_a_labeled_engine_config_is_created_and_read_back_out_of_the_project_file() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = with_flags(
            &project,
            &["llm", "create", "big", "--type", "mock", "--model", "o1"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(ran.stdout, "Created llm \"big\" (type: mock, model: o1)\n");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "label"), "big");
            assert_eq!(reports_as(&doc, "type"), "mock");
            assert_eq!(reports_as(&doc, "model"), "o1");
            assert_eq!(doc["permissions"], serde_json::json!({}));
        }
        project
            .file_contains(CONFIG, "big")
            .file_contains(CONFIG, "o1");
        assert_eq!(
            project.read(CONFIG).as_deref(),
            Some("schema_version: 9\nllm:\n  configs:\n    big:\n      model: o1\n      type: mock\n")
        );
        let listed = with_flags(&project, &["llm", "list"], flags);
        if !is_text(flags) {
            assert_eq!(reports_as(&listed.json(), "[label=big].label"), "big");
        }
    }
}

// llm.feature: Scenario: Create refuses a label that already resolves,
// including a built-in backend
#[test]
fn test_create_refuses_a_label_that_already_resolves_including_a_built_in_backend() {
    let project = Project::initialized();
    project
        .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
        .succeeds();
    let before = project.read(CONFIG);
    project
        .run(&["llm", "create", "big", "--type", "mock"])
        .fails()
        .contains("already exists")
        .contains("ctxloom llm edit big");
    project
        .run(&["llm", "create", "claude-code", "--type", "mock"])
        .fails()
        .contains("already exists");
    project.run(&["llm", "list"]).succeeds().contains("big");
    project.file_contains(CONFIG, "o1");
    assert_eq!(project.read(CONFIG), before);
}

// llm.feature: Scenario: Edit refuses a label nothing defines, and creates
// nothing
#[test]
fn test_edit_refuses_a_label_nothing_defines_and_creates_nothing() {
    let project = Project::initialized();
    let before = project.read(CONFIG);
    project
        .run(&["llm", "edit", "nosuchlabel", "--model", "o1-pro"])
        .fails()
        .contains("no llm named")
        .contains("ctxloom llm create nosuchlabel");
    assert_eq!(project.read(CONFIG), before);
    project
        .run(&["llm", "list"])
        .succeeds()
        .lacks("nosuchlabel");
}

// llm.feature: Scenario Outline: An edit naming only --model leaves the
// backend type intact
#[test]
fn test_an_edit_naming_only_model_leaves_the_backend_type_intact() {
    for flags in FORMATS {
        let project = Project::initialized();
        project
            .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
            .succeeds();
        let ran = with_flags(
            &project,
            &["llm", "edit", "big", "--model", "o1-pro"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Updated llm \"big\" (type: mock, model: o1-pro)\n"
            );
        } else {
            assert_eq!(reports_as(&ran.json(), "model"), "o1-pro");
            assert_eq!(reports_as(&ran.json(), "type"), "mock");
        }
        project
            .file_contains(CONFIG, "o1-pro")
            .file_contains(CONFIG, "mock");
    }
}

// llm.feature: Scenario Outline: Setting the permission posture records it in
// the project config
#[test]
fn test_setting_the_permission_posture_records_it_in_the_project_config() {
    for flags in FORMATS {
        let project = Project::initialized();
        project
            .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
            .succeeds();
        let ran = with_flags(
            &project,
            &["llm", "edit", "big", "--permissions", "bypass"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("permissions: mode=bypass");
        } else {
            assert_eq!(reports_as(&ran.json(), "permissions.mode"), "bypass");
        }
        project.file_contains(CONFIG, "mode: bypass");
    }
    // The label's engine refuses a mode it does not take, and writes nothing.
    let project = Project::initialized();
    project
        .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
        .succeeds();
    let before = project.read(CONFIG);
    project
        .run(&["llm", "edit", "big", "--permissions", "acceptEdits"])
        .fails()
        .contains("acceptEdits");
    assert_eq!(project.read(CONFIG), before);
    // An empty mode clears it.
    project
        .run(&["llm", "edit", "big", "--permissions", "plan"])
        .succeeds();
    project
        .run(&["llm", "edit", "big", "--permissions", ""])
        .succeeds();
    project.file_lacks(CONFIG, "mode:");
}

// operations.SetLLM (documented condition): a built-in backend becomes an
// explicit entry by an edit; a type nobody knows is refused; --set reads its
// value as YAML, checks the entry against the config schema, and refuses what
// the schema does; --home writes the user-global config and not the project's.
#[test]
fn test_edit_turns_a_built_in_into_an_entry_and_set_and_home_do_their_part() {
    let project = Project::initialized();
    project
        .run(&[
            "llm",
            "edit",
            "claude-code",
            "--model",
            "sonnet",
            "--format",
            "text",
        ])
        .succeeds()
        .contains("Updated llm \"claude-code\" (type: claude-code, model: sonnet)");
    project.file_contains(CONFIG, "claude-code:\n      model: sonnet");
    let before = project.read(CONFIG);
    project
        .run(&["llm", "create", "bad", "--type", "nosuch"])
        .fails()
        .contains("unknown type \"nosuch\"; known: claude-code, mock, mock-launch, mock-lossy, mock-noskills, opencode");
    assert_eq!(project.read(CONFIG), before);
    let home_before = project.read_home(CONFIG);
    project
        .run(&[
            "llm",
            "create",
            "qwen",
            "--home",
            "--type",
            "opencode",
            "--model",
            "qwen3.8-27b",
            "--set",
            "context_window=131072",
            "--set",
            "provider.output_limit=8192",
            "--set",
            "provider.base_url=http://192.168.1.20:8081/v1",
        ])
        .succeeds();
    assert_eq!(
        project.read(CONFIG),
        before,
        "--home wrote the project's config"
    );
    let home = project.read_home(CONFIG).expect("the home config");
    assert!(
        home.starts_with(home_before.as_deref().unwrap_or_default()),
        "{home}"
    );
    assert!(home.contains("qwen:\n"), "{home}");
    assert!(home.contains("context_window: 131072"), "{home}");
    assert!(
        home.contains("base_url: http://192.168.1.20:8081/v1"),
        "{home}"
    );
    assert!(
        home.contains("editor:"),
        "the rest of the home config was kept: {home}"
    );
    project
        .run(&["llm", "edit", "qwen", "--home", "--set", "no_such_key=1"])
        .fails()
        .contains("no_such_key");
    assert_eq!(project.read_home(CONFIG).as_deref(), Some(home.as_str()));
    project.run(&["llm", "list", "--format", "json"]).succeeds();
    project
        .run(&["llm", "remove", "qwen", "--home", "--yes"])
        .succeeds();
    assert!(!project
        .read_home(CONFIG)
        .unwrap_or_default()
        .contains("qwen"));
}

// llm.feature: Scenario: There is no way to put a credential into an entry
#[test]
fn test_there_is_no_way_to_put_a_credential_into_an_entry() {
    let project = Project::initialized();
    project
        .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
        .succeeds();
    project
        .run(&["llm", "edit", "big", "--env-file", "secrets.env"])
        .fails()
        .contains("--env-file");
}

// llm.feature: Scenario Outline: A label the team wrote is told apart from an
// engine ctxloom supplied (the writes: the label, the default, the refusal)
#[test]
fn test_a_label_the_team_wrote_is_told_apart_from_an_engine_ctxloom_supplied() {
    for flags in FORMATS {
        let project = Project::initialized();
        project
            .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
            .succeeds();
        project.run(&["llm", "default", "mock"]).succeeds();
        let listed = with_flags(&project, &["llm", "list"], flags);
        listed.succeeds();
        if is_text(flags) {
            listed
                .contains("[configured]")
                .contains("[built-in]")
                .contains("(default)");
        } else {
            let doc = listed.json();
            assert_eq!(reports_as(&doc, "[label=big].authored"), "true");
            assert_eq!(reports_as(&doc, "[label=claude-code].authored"), "false");
            assert_eq!(reports_as(&doc, "[label=mock].default"), "true");
            assert_eq!(reports_as(&doc, "[label=mock].authored"), "false");
        }
        project
            .run(&["llm", "remove", "claude-code"])
            .fails()
            .contains("not defined in config.yaml");
    }
}

// llm.feature: Scenario Outline: Bare remove reports and destroys nothing
#[test]
fn test_bare_llm_remove_reports_and_destroys_nothing() {
    for flags in FORMATS {
        let project = Project::initialized();
        project
            .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
            .succeeds();
        let before = project.read(CONFIG);
        let ran = with_flags(&project, &["llm", "remove", "big"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Would remove llm \"big\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom llm remove big --yes\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "applied"), "false");
            assert_eq!(reports_as(&doc, "apply"), "ctxloom llm remove big --yes");
        }
        let listed = project.run(&["llm", "list", "--format", "json"]).json();
        assert_eq!(reports_as(&listed, "[label=big].label"), "big");
        project.file_contains(CONFIG, "big");
        assert_eq!(project.read(CONFIG), before);
    }
}

// llm.feature: Scenario Outline: --yes takes the entry out of the project file
#[test]
fn test_yes_takes_the_entry_out_of_the_project_file() {
    for flags in FORMATS {
        let project = Project::initialized();
        project
            .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
            .succeeds();
        project
            .run(&["llm", "create", "small", "--type", "mock", "--model", "o2"])
            .succeeds();
        project.file_contains(CONFIG, "big");
        let ran = with_flags(&project, &["llm", "remove", "big", "--yes"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(ran.stdout, "Removed llm \"big\"\n");
        } else {
            assert_eq!(reports_as(&ran.json(), "status"), "removed");
            assert_eq!(reports_as(&ran.json(), "label"), "big");
        }
        project.file_lacks(CONFIG, "big");
        // Exactly the target.
        project
            .file_contains(CONFIG, "small:")
            .file_contains(CONFIG, "o2");
        project.run(&["llm", "list"]).succeeds().lacks("big");
    }
}

// llm.feature: Scenario: Removing a label config.yaml never declared is
// refused
#[test]
fn test_removing_a_label_config_yaml_never_declared_is_refused() {
    let project = Project::initialized();
    let before = project.read(CONFIG);
    project
        .run(&["llm", "remove", "claude-code"])
        .fails()
        .contains("not defined in config.yaml");
    project
        .run(&["llm", "remove", "claude-code", "--yes"])
        .fails()
        .contains("llm \"claude-code\" not found in config.yaml");
    assert_eq!(project.read(CONFIG), before);
    project
        .run(&["llm", "list"])
        .succeeds()
        .contains("claude-code");
}
