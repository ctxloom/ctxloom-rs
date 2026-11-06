//! Go's acceptance scenarios for the agent noun's write side (S2),
//! `tests/acceptance/features/cli/agent.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. Answers and the config's bytes are
//! Go's own (captured from a Go ctxloom run in a scratch project).
//!
//! Not here: running an agent (`--engine-home` measured on a spawned engine,
//! S6), `agent show`'s headless posture (S1's read side).
//!
//! A config write is a section merge: the `agents` section is replaced whole
//! with its keys sorted, every other section stays as the file has it, and a
//! file that does not parse is never rewritten (the config tests carry that).

use super::library::{contains_value, entries, reports_as, Project};

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

/// Go's `a profile "dev" exists`.
fn with_dev() -> Project {
    let project = Project::initialized();
    project.bundle("dev-base");
    project.profile("dev", "dev-base");
    project
}

// agent.feature: Scenario Outline: Creating a binding records every axis, and
// the listing reads each one back
#[test]
fn test_creating_a_binding_records_every_axis_and_the_listing_reads_each_one_back() {
    for flags in FORMATS {
        let project = with_dev();
        let ran = with_flags(
            &project,
            &[
                "agent",
                "create",
                "developer",
                "--llm",
                "claude-code",
                "--profiles",
                "dev",
                "--runtime",
                "container-rootless",
            ],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Created agent \"developer\" (llm: claude-code, profiles: dev, runtime: container-rootless)\n"
            );
        } else {
            assert_eq!(reports_as(&ran.json(), "name"), "developer");
        }
        project.file_contains(CONFIG, "developer");
        let listed = with_flags(&project, &["agent", "list"], flags);
        listed.succeeds();
        if is_text(flags) {
            listed
                .contains("llm: claude-code")
                .contains("profiles: dev")
                .contains("runtime: container-rootless");
        } else {
            let doc = listed.json();
            assert_eq!(reports_as(&doc, "[name=developer].llm"), "claude-code");
            assert!(contains_value(&doc, "[name=developer].profiles", "dev"));
            assert_eq!(
                reports_as(&doc, "[name=developer].runtime"),
                "container-rootless"
            );
        }
    }
}

// operations.SetAgent (documented condition): the entry as Go writes it, keys
// sorted in every map, the rest of the file untouched; the entry in json.
#[test]
fn test_a_binding_is_written_with_sorted_keys_and_the_rest_of_the_file_is_kept() {
    let project = with_dev();
    project.write(
        CONFIG,
        "# my config\nschema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme-bin\n",
    );
    project
        .run(&[
            "agent",
            "create",
            "dev2",
            "--llm",
            "claude-code",
            "--profiles",
            "dev,dev-base",
            "--runtime",
            "container-rootless",
            "--permissions",
            "plan",
            "--surface",
            "context=system-prompt",
            "--root",
            "mcp=project-root",
            "--env-host=false",
            "--env",
            "A",
            "--env",
            "B",
            "--engine-home",
            "session",
        ])
        .succeeds();
    assert_eq!(
        project.read(CONFIG).as_deref(),
        Some(concat!(
            "# my config\nschema_version: 9\ncompanions:\n  acme:\n    command:\n      - acme-bin\n",
            "agents:\n",
            "  dev2:\n",
            "    engine_home: session\n",
            "    env:\n      - A\n      - B\n",
            "    env_host: false\n",
            "    llm: claude-code\n",
            "    permissions:\n      claude-code:\n        mode: plan\n",
            "    profiles:\n      - dev\n      - dev-base\n",
            "    roots:\n      mcp: project-root\n",
            "    runtime: container-rootless\n",
            "    surfaces:\n      context: system-prompt\n",
        ))
    );
    let doc = project
        .run(&[
            "agent",
            "create",
            "dev3",
            "--profiles",
            "dev",
            "--format",
            "json",
        ])
        .json();
    assert_eq!(reports_as(&doc, "name"), "dev3");
    assert!(contains_value(&doc, "profiles", "dev"));
    assert_eq!(doc["permissions"], serde_json::json!({}));
    let text = project.run(&[
        "agent",
        "edit",
        "dev2",
        "--runtime",
        "host",
        "--format",
        "text",
    ]);
    assert_eq!(
        text.stdout,
        "Updated agent \"dev2\" (llm: claude-code, profiles: dev, dev-base, runtime: host, permissions: claude-code={mode=plan}, engine_home: session, env_host: false, env: A, B)\n"
    );
}

// agent.feature: Scenario Outline: Create refuses an existing name, and leaves
// the live binding untouched
#[test]
fn test_create_refuses_an_existing_name_and_leaves_the_live_binding_untouched() {
    for flags in FORMATS {
        let project = with_dev();
        project.bundle("ops-base");
        project.profile("ops", "ops-base");
        project
            .run(&[
                "agent",
                "create",
                "developer",
                "--llm",
                "claude-code",
                "--profiles",
                "dev",
            ])
            .succeeds();
        let before = project.read(CONFIG);
        project
            .run(&["agent", "create", "developer", "--profiles", "ops"])
            .fails()
            .contains("already exists")
            .contains("ctxloom agent edit developer");
        assert_eq!(project.read(CONFIG), before);
        let shown = with_flags(&project, &["agent", "show", "developer"], flags);
        shown.succeeds();
        if !is_text(flags) {
            let doc = shown.json();
            assert!(contains_value(&doc, "definition.profiles", "dev"));
            assert!(!contains_value(&doc, "resolved.profiles", "ops"));
        }
    }
}

// agent.feature: Scenario Outline: Edit refuses a name no agent has, and
// creates nothing
#[test]
fn test_edit_refuses_a_name_no_agent_has_and_creates_nothing() {
    for flags in FORMATS {
        let project = with_dev();
        let before = project.read(CONFIG);
        project
            .run(&["agent", "edit", "nosuchagent", "--profiles", "dev"])
            .fails()
            .contains("no agent named")
            .contains("ctxloom agent create nosuchagent");
        assert_eq!(project.read(CONFIG), before);
        let listed = with_flags(&project, &["agent", "list"], flags);
        listed.succeeds();
        if is_text(flags) {
            listed.contains("No agents defined");
        } else {
            assert_eq!(entries(&listed.json(), "$"), 0);
        }
    }
}

// agent.feature: Scenario Outline: An edit naming only --runtime leaves every
// other axis intact
#[test]
fn test_an_edit_naming_only_runtime_leaves_every_other_axis_intact() {
    for flags in FORMATS {
        let project = with_dev();
        project.bundle("ops-base");
        project.profile("ops", "ops-base");
        project
            .run(&[
                "agent",
                "create",
                "developer",
                "--llm",
                "claude-code",
                "--profiles",
                "dev,ops",
                "--runtime",
                "host",
                "--permissions",
                "plan",
            ])
            .succeeds();
        let ran = with_flags(
            &project,
            &[
                "agent",
                "edit",
                "developer",
                "--runtime",
                "container-rootless",
            ],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Updated agent")
                .contains("runtime: container-rootless")
                .contains("llm: claude-code")
                .contains("profiles: dev, ops")
                .contains("permissions: claude-code={mode=plan}");
        } else {
            assert_eq!(reports_as(&ran.json(), "name"), "developer");
        }
        let listed = project.run(&["agent", "list", "--format", "json"]).json();
        assert_eq!(
            reports_as(&listed, "[name=developer].runtime"),
            "container-rootless"
        );
        assert_eq!(reports_as(&listed, "[name=developer].llm"), "claude-code");
        assert!(contains_value(&listed, "[name=developer].profiles", "dev"));
        assert!(contains_value(&listed, "[name=developer].profiles", "ops"));
        assert_eq!(
            reports_as(&listed, "[name=developer].permissions.claude-code.mode"),
            "plan"
        );
    }
}

// agent.feature: Scenario Outline: Setting a surface delivery preference
// records it in the binding
#[test]
fn test_setting_a_surface_delivery_preference_records_it_in_the_binding() {
    for flags in FORMATS {
        let project = with_dev();
        project
            .run(&[
                "agent",
                "create",
                "developer",
                "--llm",
                "claude-code",
                "--profiles",
                "dev",
            ])
            .succeeds();
        let ran = with_flags(
            &project,
            &[
                "agent",
                "edit",
                "developer",
                "--surface",
                "context=system-prompt",
            ],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Updated agent");
        } else {
            assert_eq!(reports_as(&ran.json(), "name"), "developer");
        }
        project.file_contains(CONFIG, "context: system-prompt");
    }
}

// agent.feature: Scenario: Selecting a root for a surface kind records it in
// the binding
#[test]
fn test_selecting_a_root_for_a_surface_kind_records_it_in_the_binding() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "developer",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
        ])
        .succeeds();
    project
        .run(&["agent", "edit", "developer", "--root", "mcp=project-root"])
        .succeeds();
    project.file_contains(CONFIG, "mcp: project-root");
}

// agent.feature: Scenario: A root the engine does not offer is refused, and
// the binding is left alone
#[test]
fn test_a_root_the_engine_does_not_offer_is_refused_and_the_binding_is_left_alone() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "developer",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
        ])
        .succeeds();
    let before = project.read(CONFIG);
    project
        .run(&["agent", "edit", "developer", "--root", "context=nowhere"])
        .fails()
        .contains("context=nowhere");
    project.file_lacks(CONFIG, "roots:");
    assert_eq!(project.read(CONFIG), before);
}

// agent.feature: Scenario: There is no flag that gives an agent an auth mode
#[test]
fn test_there_is_no_flag_that_gives_an_agent_an_auth_mode() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "developer",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
        ])
        .succeeds();
    project
        .run(&["agent", "edit", "developer", "--auth", "login"])
        .fails()
        .contains("--auth");
    project.file_lacks(CONFIG, "auth:");
}

// operations.SetAgent (documented conditions): what an agent write refuses
// and writes nothing for: an engine nothing defines, a runtime that is not
// an axis, a permission mode its engine does not take, a binding with neither
// an engine nor a profile, an env that would not do what it says.
#[test]
fn test_an_agent_write_refuses_what_it_cannot_honour_and_writes_nothing() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "dev2",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
        ])
        .succeeds();
    let before = project.read(CONFIG);
    for (args, said) in [
        (vec!["--llm", "nosuch"], "unknown engine \"nosuch\"; valid engines: "),
        (vec!["--runtime", "weird"], "unknown runtime axis \"weird\" (known: host|container-rootless|container-rootful)"),
        (vec!["--permissions", "weird"], "mode \"weird\" is not a claude mode (known: default|acceptEdits|plan|bypassPermissions)"),
        (vec!["--permissions", "bypass"], "mode \"bypass\" is declared as mode: bypassPermissions"),
        (vec!["--engine-home", "weird"], "weird"),
        (vec!["--llm", "", "--profiles", ""], "an agent needs an llm or at least one profile"),
        (vec!["--env", "A=1"], "A=1"),
        (vec!["--env", "A"], "env"),
        (vec!["--root", "context=bogus"], "\"bogus\" is not a root (session-home, project-root, work-dir)"),
        (vec!["--surface", "context=nosuch"], "context=nosuch"),
    ] {
        let mut all = vec!["agent", "edit", "dev2"];
        all.extend(args.clone());
        project.run(&all).fails().contains(said);
        assert_eq!(project.read(CONFIG), before, "{args:?} wrote something");
    }
    // A permission mode needs the engine it is checked against.
    project
        .run(&[
            "agent",
            "create",
            "noengine",
            "--profiles",
            "dev",
            "--permissions",
            "plan",
        ])
        .fails()
        .contains("set --llm in the same command");
    project
        .run(&["agent", "create", "empty"])
        .fails()
        .contains("an agent needs an llm or at least one profile");
    assert_eq!(project.read(CONFIG), before);
}

// agent.feature: Scenario: Create records the declared host environment, and
// show reads it back
#[test]
fn test_create_records_the_declared_host_environment_and_show_reads_it_back() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "developer",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
            "--env-host=false",
            "--env",
            "ALPHA_TOKEN",
            "--env",
            "BETA_TOKEN",
        ])
        .succeeds();
    project
        .run(&["agent", "show", "developer", "--format", "text"])
        .succeeds()
        .contains("Env host: false")
        .contains("ALPHA_TOKEN")
        .contains("BETA_TOKEN");
}

// agent.feature: Scenario: An edit passing an empty --env clears the names
// and keeps env_host
#[test]
fn test_an_edit_passing_an_empty_env_clears_the_names_and_keeps_env_host() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "developer",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
            "--env-host=false",
            "--env",
            "ALPHA_TOKEN",
        ])
        .succeeds();
    project
        .run(&["agent", "edit", "developer", "--env", ""])
        .succeeds();
    project
        .run(&["agent", "show", "developer", "--format", "text"])
        .succeeds()
        .contains("Env host: false")
        .lacks("ALPHA_TOKEN");
}

// agent.feature: Scenario: --engine-home session moves the engine off Alice's
// own home onto a per-session one (the binding's half: the flag writes the
// key; the spawned engine is S6's)
#[test]
fn test_engine_home_session_is_written_into_the_binding() {
    let project = with_dev();
    project
        .run(&[
            "agent",
            "create",
            "isolated",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
        ])
        .succeeds();
    project
        .run(&["agent", "edit", "isolated", "--engine-home", "session"])
        .succeeds();
    project.file_contains(CONFIG, "engine_home: session");
}

// agent.feature: Scenario: With nothing bound, the report says so; naming an
// agent binds it
#[test]
fn test_with_nothing_bound_the_report_says_so_and_naming_an_agent_binds_it() {
    let project = with_dev();
    project
        .run(&["agent", "default", "--format", "text"])
        .succeeds()
        .contains("No default agent set.");
    project
        .run(&["agent", "create", "developer", "--profiles", "dev"])
        .succeeds();
    project
        .run(&["agent", "default", "developer", "--format", "text"])
        .succeeds();
    project.file_contains(CONFIG, "default_agent: developer");
    project
        .run(&["agent", "default", "--format", "text"])
        .succeeds()
        .contains("Default agent: developer");
    // Go's answer, and the format it does not honour yet.
    assert_eq!(
        project.run(&["agent", "default", "developer"]).stdout,
        "Set default agent to \"developer\".\n"
    );
    project
        .run(&["agent", "default", "--format", "json"])
        .fails()
        .contains("does not support it yet");
}

// agent.feature: Scenario: Naming an agent that does not exist yet warns, and
// still binds it
#[test]
fn test_naming_an_agent_that_does_not_exist_yet_warns_and_still_binds_it() {
    let project = with_dev();
    project
        .run(&["agent", "default", "ghost", "--format", "text"])
        .succeeds()
        .contains("not defined yet");
    project.file_contains(CONFIG, "default_agent: ghost");
}

// agent.feature: Scenario Outline: Bare remove reports and destroys nothing
#[test]
fn test_bare_agent_remove_reports_and_destroys_nothing() {
    for flags in FORMATS {
        let project = with_dev();
        project
            .run(&["agent", "create", "developer", "--profiles", "dev"])
            .succeeds();
        let before = project.read(CONFIG);
        let ran = with_flags(&project, &["agent", "remove", "developer"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Would remove agent \"developer\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom agent remove developer --yes\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "applied"), "false");
            assert_eq!(
                reports_as(&doc, "apply"),
                "ctxloom agent remove developer --yes"
            );
        }
        assert_eq!(project.read(CONFIG), before, "a preview wrote the config");
        let listed = project.run(&["agent", "list", "--format", "json"]).json();
        assert_eq!(reports_as(&listed, "[name=developer].name"), "developer");
    }
}

// agent.feature: Scenario Outline: --yes takes the binding out of the config
// it was written to
#[test]
fn test_yes_takes_the_binding_out_of_the_config_it_was_written_to() {
    for flags in FORMATS {
        let project = with_dev();
        project
            .run(&["agent", "create", "developer", "--profiles", "dev"])
            .succeeds();
        project
            .run(&["agent", "create", "other", "--profiles", "dev"])
            .succeeds();
        project.file_contains(CONFIG, "developer");
        let ran = with_flags(&project, &["agent", "remove", "developer", "--yes"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(ran.stdout, "Removed agent \"developer\"\n");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "removed");
            assert_eq!(reports_as(&doc, "name"), "developer");
        }
        project.file_lacks(CONFIG, "developer");
        // Exactly the target.
        project.file_contains(CONFIG, "other:");
        let listed = project.run(&["agent", "list", "--format", "json"]).json();
        assert_eq!(entries(&listed, "$"), 1);
    }
    let project = with_dev();
    project
        .run(&["agent", "create", "only", "--profiles", "dev"])
        .succeeds();
    project
        .run(&["agent", "remove", "only", "--yes"])
        .succeeds();
    project
        .run(&["agent", "list", "--format", "text"])
        .succeeds()
        .contains("No agents defined");
}

// agent.feature: Scenario: Removing a binding nobody defined fails rather
// than reporting success
#[test]
fn test_removing_a_binding_nobody_defined_fails_rather_than_reporting_success() {
    let project = with_dev();
    project
        .run(&["agent", "remove", "nosuchagent", "--yes"])
        .fails()
        .contains("nosuchagent");
    project
        .run(&["agent", "remove", "nosuchagent"])
        .fails()
        .contains("nosuchagent");
}
