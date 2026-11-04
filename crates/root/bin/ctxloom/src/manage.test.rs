//! Go's `tests/acceptance/features/cli/manage.feature` (release/0.7) and
//! `journeys/j000100_adopt_and_back_out.feature`, ported (S9, surface half).
//! Each test names its scenario.
//!
//! Not ported here, and why:
//! - the Examples rows and scenarios that read the `discover` command's body
//!   ("Every engine gets ctxloom's shipped commands in its own idiom"): the
//!   text names `search_library`, an MCP tool the D8 cut list has not ruled
//!   on, so D24 holds it for the owner's rewrite;
//! - "After an uninstall, a run delivers into its session ...": it needs a
//!   running mock engine (S6);
//! - "The hook list names the specific place each hook was declared": it
//!   needs a companion's loadout read, which arrives with the companions'
//!   contributions (S6).
//!
//! plan supersedes Go (D8, D24): ctxloom's own `isolation-axes` fragment is
//! copied byte for byte from Go release/0.7 f36e8dfb2 (see
//! `setup-application`'s `owner_voice`), and is what makes a materialize of a
//! project with no content other than ctxloom's own non-empty.

use serde_json::Value;

use super::library::{entries, reports_as, Project};

const ENGINES: [&str; 2] = ["claude-code", "mock"];

fn wired(project: &Project, engine: &str) {
    project
        .run(&["manage", "install", "--engine", engine, "--no-pull"])
        .succeeds();
    project
        .run(&["materialize", "--yes", "--degraded"])
        .succeeds();
}

// manage.feature: Scenario Outline: Installing scaffolds the project and
// writes no engine file
#[test]
fn test_installing_scaffolds_the_project_and_writes_no_engine_file() {
    for engine in ENGINES {
        let project = Project::bare();
        project
            .run(&["manage", "install", "--engine", engine, "--no-pull"])
            .succeeds();
        project.file_contains(".ctxloom/config.yaml", "schema_version");
        project.file_contains(".ctxloom/config.yaml", engine);
        project.file_contains(".ctxloom/.gitignore", "ctxloom");
        for absent in [
            ".claude/settings.json",
            ".mcp.json",
            "CLAUDE.md",
            "MOCK_CONTEXT.md",
            ".mock/mcp.json",
            ".mock/commands/discover.md",
        ] {
            project.file_is_absent(absent);
        }
    }
}

// manage.feature: Scenario Outline: Alice wires an empty project for her
// engine (the context rows)
#[test]
fn test_alice_wires_an_empty_project_for_her_engine_the_context_rows() {
    for (engine, surface) in [("claude-code", "CLAUDE.md"), ("mock", "MOCK_CONTEXT.md")] {
        let project = Project::bare();
        wired(&project, engine);
        project.file_contains(surface, "Isolation: specify both axes");
        project.file_contains(".ctxloom/.gitignore", "ctxloom");
    }
}

// manage.feature: Scenario Outline: Alice wires an empty project for her
// engine (claude's settings carry the session-start hook)
#[test]
fn test_claudes_settings_carry_the_session_start_hook() {
    let project = Project::bare();
    wired(&project, "claude-code");
    project.file_contains(".claude/settings.json", "session-start");
}

// manage.feature: Scenario Outline: No engine gets ctxloom registered as an
// MCP server at rest
#[test]
fn test_no_engine_gets_ctxloom_registered_as_an_mcp_server_at_rest() {
    for (engine, surface) in [("claude-code", ".mcp.json"), ("mock", ".mock/mcp.json")] {
        let project = Project::bare();
        wired(&project, engine);
        assert!(
            !project.registers_mcp(surface, "mcpServers", "ctxloom"),
            "{surface} registers ctxloom"
        );
    }
}

// manage.feature: Scenario: A misspelled engine is refused by name, and
// nothing is scaffolded
#[test]
fn test_a_misspelled_engine_is_refused_by_name_and_nothing_is_scaffolded() {
    let project = Project::bare();
    project
        .run(&["manage", "install", "--engine", "totally-bogus-engine"])
        .fails()
        .contains("unknown engine")
        .contains("totally-bogus-engine")
        .contains("claude-code");
    project.file_is_absent(".ctxloom/config.yaml");
    project.file_is_absent(".mcp.json");
}

// manage.feature: Scenario: Config create scaffolds a config in a bare project
#[test]
fn test_config_create_scaffolds_a_config_in_a_bare_project() {
    let project = Project::bare();
    project
        .run(&["config", "create", "--engine", "claude-code"])
        .succeeds();
    assert!(project.exists(".ctxloom/config.yaml"));
}

// manage.feature: Scenario Outline: Config create records which engine the
// project uses
#[test]
fn test_config_create_records_which_engine_the_project_uses() {
    let project = Project::bare();
    project
        .run(&["config", "create", "--engine", "mock"])
        .succeeds();
    project
        .file_contains(".ctxloom/config.yaml", "llm: mock")
        .file_contains(".ctxloom/config.yaml", "type: mock")
        .file_contains(".ctxloom/config.yaml", "primary: mock");
}

// manage.feature: Scenario Outline: Check reports which surfaces are actually
// wired (the three --format rows)
#[test]
fn test_check_reports_which_surfaces_are_actually_wired() {
    for (flags, hooks, mcp) in [
        (vec![], "true", "false"),
        (vec!["--format", "json"], "true", "false"),
        (vec!["--format", "text"], "hooks=true", "mcp=false"),
    ] {
        let project = Project::initialized();
        project.run(&["materialize", "--yes"]).succeeds();
        let mut args = vec!["manage", "check"];
        args.extend(flags.iter());
        let ran = project.run(&args);
        ran.succeeds();
        if flags.contains(&"text") {
            ran.contains(hooks).contains(mcp);
        } else {
            let doc = ran.json();
            assert_eq!(
                reports_as(&doc, "backends[backend=claude-code].hooks_present"),
                hooks
            );
            assert_eq!(
                reports_as(&doc, "backends[backend=claude-code].mcp_present"),
                mcp
            );
        }
    }
}

// manage.feature: Scenario Outline: A team's hook reaches every engine's own
// hook surface
#[test]
fn test_a_teams_hook_reaches_every_engines_own_hook_surface() {
    let project = super::surfaces::team_project();
    project
        .run(&[
            "materialize",
            "team",
            "--target",
            "out-claude-code",
            "--backend",
            "claude-code",
        ])
        .succeeds();
    assert!(project.registers_hook(
        "out-claude-code/.claude/settings.json",
        "SessionStart",
        super::surfaces::HOOK_COMMAND
    ));
}

// manage.feature: Scenario: Hooks can be installed, inspected, and actually
// removed
#[test]
fn test_hooks_can_be_installed_inspected_and_actually_removed() {
    let project = Project::initialized();
    project.run(&["materialize", "--yes"]).succeeds();
    assert!(project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
    project.run(&["manage", "hooks", "check"]).succeeds();
    project
        .run(&["materialize", "--release", "--yes"])
        .succeeds();
    project.file_is_absent(".claude/settings.json");
}

// manage.feature: Scenario: Re-applying hooks does not duplicate them
#[test]
fn test_reapplying_hooks_does_not_duplicate_them() {
    let project = Project::initialized();
    project.run(&["materialize", "--yes"]).succeeds();
    project.run(&["materialize", "--yes"]).succeeds();
    project.file_counts(".claude/settings.json", "skill-mates", 1);
}

fn hooked_project() -> Project {
    let project = Project::initialized();
    let root = ".ctxloom/content/bundles/v2/hooked";
    project.write(
        &format!("{root}/bundle.yaml"),
        "schema_version: 4\nversion: 1.0.0\ndescription: ships two pre_tool hooks whose order contradicts their position\n",
    );
    // "second" is declared first and ordered last.
    project.write(
        &format!("{root}/hooks/pre_tool/a-second.yaml"),
        "type: command\ncommand: echo HOOKLIST-SECOND\n",
    );
    project.write(
        &format!("{root}/hooks/pre_tool/.a-second.meta.yaml"),
        "order: 200\n",
    );
    project.write(
        &format!("{root}/hooks/pre_tool/b-first.yaml"),
        "type: command\ncommand: echo HOOKLIST-FIRST\n",
    );
    project.write(
        &format!("{root}/hooks/pre_tool/.b-first.meta.yaml"),
        "order: 100\n",
    );
    project.profile("hooky", "hooked");
    project
}

// manage.feature: Scenario Outline: Listing hooks shows the resolved order and
// where each came from
#[test]
fn test_listing_hooks_shows_the_resolved_order_and_where_each_came_from() {
    for (flags, kind, origin) in [
        (vec![], "bundle", "ctxloom+local:hooked"),
        (vec!["--format", "json"], "bundle", "ctxloom+local:hooked"),
        (
            vec!["--format", "text"],
            "bundle ctxloom+local:hooked",
            "bundle ctxloom+local:hooked",
        ),
    ] {
        let project = hooked_project();
        let mut args = vec![
            "manage",
            "hooks",
            "list",
            "--event",
            "pre_tool",
            "--profile",
            "hooky",
        ];
        args.extend(flags.iter());
        let ran = project.run(&args);
        ran.succeeds()
            .contains("HOOKLIST-FIRST")
            .contains("HOOKLIST-SECOND");
        let text = ran.output();
        assert!(
            text.find("HOOKLIST-FIRST") < text.find("HOOKLIST-SECOND"),
            "{text}"
        );
        if flags.contains(&"text") {
            ran.contains(kind).contains(origin);
        } else {
            let doc = ran.json();
            let at = "events[event=pre_tool].hooks[command=echo HOOKLIST-FIRST]";
            assert_eq!(reports_as(&doc, &format!("{at}.source_kind")), kind);
            assert_eq!(reports_as(&doc, &format!("{at}.source")), origin);
        }
    }
}

// manage.feature: Scenario: The hook list has a machine-readable form
#[test]
fn test_the_hook_list_has_a_machine_readable_form() {
    let project = Project::initialized();
    let ran = project.run(&["manage", "hooks", "list", "--format", "json"]);
    ran.succeeds();
    let doc = ran.json();
    let events = doc["events"].as_array().expect("events");
    for wanted in ["post_file_edit", "session_start"] {
        assert!(
            events.iter().any(|e| e["event"] == wanted),
            "no {wanted} in {doc}"
        );
    }
    assert!(events
        .iter()
        .all(|e| e["event"].as_str().is_some_and(|s| !s.is_empty())));
    assert!(entries(&doc, "events") > 0);
}

// manage.feature: Scenario: An unknown event name is refused rather than
// answered empty
#[test]
fn test_an_unknown_event_name_is_refused_rather_than_answered_empty() {
    let project = Project::initialized();
    project
        .run(&["manage", "hooks", "list", "--event", "pre_toll"])
        .fails()
        .contains("pre_toll");
}

// manage.feature: Scenario: Statusline can be disabled and re-enabled
#[test]
fn test_statusline_can_be_disabled_and_re_enabled() {
    let project = Project::initialized();
    project
        .run(&["manage", "statusline", "uninstall"])
        .succeeds();
    project.run(&["materialize", "--yes"]).succeeds();
    project.file_lacks(".claude/settings.json", "hook hud");
    project.run(&["manage", "statusline", "install"]).succeeds();
    project.run(&["materialize", "--yes"]).succeeds();
    project.file_contains(".claude/settings.json", "hook hud");
}

// manage.feature: Scenario: Materializing preserves the exact numbers the
// user wrote by hand
#[test]
fn test_materializing_preserves_the_exact_numbers_the_user_wrote_by_hand() {
    let project = Project::initialized();
    project.write(
        ".claude/settings.json",
        "{\n  \"awkwardNumber\": 1234567890123456789,\n  \"nested\": { \"id\": -9223372036854775808 },\n  \"permissions\": { \"allow\": [\"Read\"], \"quota\": 18446744073709551615 }\n}\n",
    );
    project.run(&["materialize", "--yes"]).succeeds();
    assert!(project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
    for kept in [
        "1234567890123456789",
        "-9223372036854775808",
        "18446744073709551615",
        "Read",
    ] {
        project.file_contains(".claude/settings.json", kept);
    }
}

// manage.feature: Scenario: A statusline ctxloom cannot read refuses the write
// instead of replacing it
#[test]
fn test_a_statusline_ctxloom_cannot_read_refuses_the_write_instead_of_replacing_it() {
    let project = Project::initialized();
    project.write(
        ".claude/settings.json",
        "{\n  \"env\": { \"A\": \"b\" },\n  \"statusLine\": { \"type\": \"command\", \"command\": { \"exec\": \"ccusage\" } }\n}\n",
    );
    project
        .run(&["materialize", "--yes"])
        .fails()
        .contains("refusing to write settings.json");
    project.file_contains(".claude/settings.json", "ccusage");
    assert!(!project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
}

// manage.feature: Scenario: Gitignore install excludes ctxloom's private state
#[test]
fn test_gitignore_install_excludes_ctxloom_private_state() {
    let project = Project::initialized();
    project.run(&["manage", "gitignore", "install"]).succeeds();
    project.file_contains(".ctxloom/.gitignore", "/cache");
    project.file_contains(".ctxloom/.gitignore", "/state");
    project.file_is_absent(".gitignore");
}

// manage.feature: Scenario: Repeated gitignore installs do not stack comment
// headers
#[test]
fn test_repeated_gitignore_installs_do_not_stack_comment_headers() {
    let project = Project::initialized();
    project.run(&["manage", "gitignore", "install"]).succeeds();
    project.run(&["manage", "gitignore", "install"]).succeeds();
    project.file_counts(".ctxloom/.gitignore", "ctxloom's private working state", 1);
}

// manage.feature: Scenario: Uninstall strips the harness but keeps the
// project's own content
#[test]
fn test_uninstall_strips_the_harness_but_keeps_the_projects_own_content() {
    let project = Project::bare();
    wired(&project, "claude-code");
    assert!(project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
    project.file_contains(".claude/settings.json", "ctxloom hook hud");
    assert!(!project.registers_mcp(".mcp.json", "mcpServers", "ctxloom"));
    project.run(&["manage", "uninstall"]).succeeds();
    project.file_is_absent(".claude/settings.json");
    assert!(!project.registers_mcp(".mcp.json", "mcpServers", "ctxloom"));
    assert!(project.exists(".ctxloom/config.yaml"));
}

// j000100_adopt_and_back_out.feature: Scenario: Alice wires ctxloom in and her
// assistant's sessions start through it
#[test]
fn test_j000100_alice_wires_ctxloom_in_and_her_sessions_start_through_it() {
    let project = Project::bare();
    wired(&project, "claude-code");
    assert!(project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
    project.file_contains(".ctxloom/.gitignore", "/cache");
}

// j000100_adopt_and_back_out.feature: Scenario: What ctxloom reports it wired
// is what is actually there
#[test]
fn test_j000100_what_ctxloom_reports_it_wired_is_what_is_actually_there() {
    let project = Project::bare();
    wired(&project, "claude-code");
    let ran = project.run(&["manage", "check"]);
    ran.succeeds();
    let doc: Value = ran.json();
    assert_eq!(
        reports_as(&doc, "backends[backend=claude-code].hooks_present"),
        "true"
    );
}

// j000100_adopt_and_back_out.feature: Scenario: Alice verifies she can leave
// before she commits
#[test]
fn test_j000100_alice_verifies_she_can_leave_before_she_commits() {
    let project = Project::bare();
    wired(&project, "claude-code");
    assert!(project.registers_hook(
        ".claude/settings.json",
        "SessionStart",
        "hook session-start"
    ));
    project.run(&["manage", "uninstall"]).succeeds();
    project.file_is_absent(".claude/settings.json");
    project.file_is_absent("CLAUDE.md");
    assert!(project.exists(".ctxloom/config.yaml"));
    assert!(project.exists(".ctxloom/content/bundles/v2/project/profiles/default.yaml"));
}

// manage.go's `--print`: the steps install would take, taking none.
#[test]
fn test_install_print_says_what_it_would_do_and_does_nothing() {
    let project = Project::bare();
    let ran = project.run(&["manage", "install", "--engine", "mock", "--print"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(doc["ctxloom_dir_exists"], false);
    assert_eq!(doc["engine"], "mock");
    project.file_is_absent(".ctxloom/config.yaml");
    project.file_is_absent(".ctxloom/.gitignore");
}
