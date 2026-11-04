//! Go's `cli/materialize.feature`, `journeys/j000400_multi_engine.feature`
//! and `journeys/j000600_agent_skill.feature` (release/0.7), ported (S9,
//! surface half). Each test names its scenario.
//!
//! Not ported here, and why:
//! - "A run in a project with materialized context warns and names the
//!   remedy": it needs a running mock engine (S6);
//! - the `@live` outline of j000400: it needs a real engine;
//! - j000400's `claude-code` rows only: the engines Go also lists (codex,
//!   opencode) are not engines of this port yet.

use super::library::Project;
use super::surfaces::{team_project, COMMAND_MARKER, CONTEXT_MARKER, HOOK_COMMAND, MCP_COMMAND};

/// Go's `a bundle "demo" exists`, `a fragment "testing" in bundle "demo"
/// exists`, `a profile "dev" with bundle "demo"`.
fn dev_project() -> Project {
    let project = Project::initialized();
    project.bundle("demo");
    project.fragment("demo", "testing");
    project.profile("dev", "demo");
    project
}

// materialize.feature: Scenario: Re-materializing only the context keeps the
// commands and skills
#[test]
fn test_re_materializing_only_the_context_keeps_the_commands_and_skills() {
    let project = dev_project();
    // `ctxloom skill create demo reviewer -d SKILL-MARKER-reviewer`
    project.write(
        ".ctxloom/content/bundles/v2/demo/skills/reviewer/SKILL.md",
        "---\nname: reviewer\ndescription: SKILL-MARKER-reviewer\n---\n\nReview.\n",
    );
    project
        .run(&["materialize", "dev", "--target", "out", "--backend", "mock"])
        .succeeds();
    project
        .file_contains("out/MOCK_CONTEXT.md", "FRAGMENT-BODY-testing")
        .file_contains(
            "out/.mock/skills/reviewer/SKILL.md",
            "SKILL-MARKER-reviewer",
        );
    assert!(project.exists("out/.mock/commands/demo/example.md"));
    project
        .run(&[
            "materialize",
            "dev",
            "--target",
            "out",
            "--backend",
            "mock",
            "--surface",
            "context",
        ])
        .succeeds();
    project.file_counts("out/MOCK_CONTEXT.md", "FRAGMENT-BODY-testing", 1);
    assert!(project.exists("out/.mock/commands/demo/example.md"));
    project
        .run(&[
            "materialize",
            "--target",
            "out",
            "--backend",
            "mock",
            "--release",
            "--surface",
            "commands",
        ])
        .succeeds();
    project.file_is_absent("out/.mock/commands/demo/example.md");
    project.file_contains("out/MOCK_CONTEXT.md", "FRAGMENT-BODY-testing");
}

// materialize.feature: Scenario: Context written into the user's own file is
// released byte for byte
#[test]
fn test_context_written_into_the_users_own_file_is_released_byte_for_byte() {
    let project = dev_project();
    project.write(
        "out/docs/AGENTS.md",
        "# Our agents\n\nHand-written notes that stay ours.",
    );
    project
        .run(&[
            "materialize",
            "dev",
            "--target",
            "out",
            "--backend",
            "mock",
            "--surface",
            "context=file:docs/AGENTS.md",
        ])
        .succeeds();
    project
        .file_contains("out/docs/AGENTS.md", "Hand-written notes that stay ours.")
        .file_contains("out/docs/AGENTS.md", "FRAGMENT-BODY-testing");
    project.file_is_absent("out/MOCK_CONTEXT.md");
    project
        .run(&[
            "materialize",
            "--target",
            "out",
            "--backend",
            "mock",
            "--release",
            "--surface",
            "context",
        ])
        .succeeds();
    project.file_lacks("out/docs/AGENTS.md", "FRAGMENT-BODY-testing");
    assert_eq!(
        project.read("out/docs/AGENTS.md").as_deref(),
        Some("# Our agents\n\nHand-written notes that stay ours."),
        "the user's file is theirs, byte for byte"
    );
}

// materialize.feature: Scenario: Releasing one engine leaves the other's files
#[test]
fn test_releasing_one_engine_leaves_the_others_files() {
    let project = dev_project();
    project
        .run(&[
            "materialize",
            "dev",
            "--target",
            "out",
            "--backend",
            "mock",
            "--backend",
            "claude-code",
        ])
        .succeeds();
    project
        .file_contains("out/MOCK_CONTEXT.md", "FRAGMENT-BODY-testing")
        .file_contains("out/CLAUDE.md", "FRAGMENT-BODY-testing");
    project
        .run(&[
            "materialize",
            "--target",
            "out",
            "--backend",
            "mock",
            "--release",
        ])
        .succeeds();
    project.file_is_absent("out/MOCK_CONTEXT.md");
    project.file_contains("out/CLAUDE.md", "FRAGMENT-BODY-testing");
}

// materialize.feature: Scenario: With no target, materialize warns and refuses
// unless --yes is given
#[test]
fn test_with_no_target_materialize_warns_and_refuses_unless_yes_is_given() {
    let project = dev_project();
    project
        .run(&["materialize", "dev", "--backend", "mock"])
        .fails()
        .contains("no --target given");
    project.file_is_absent("MOCK_CONTEXT.md");
    project
        .run(&["materialize", "dev", "--backend", "mock", "--yes"])
        .succeeds();
    project.file_contains("MOCK_CONTEXT.md", "FRAGMENT-BODY-testing");
}

// j000400_multi_engine.feature: Scenario Outline: The same profile
// materializes into each engine's own native surfaces (claude-code)
#[test]
fn test_j000400_the_same_profile_materializes_into_each_engines_native_surfaces() {
    let project = team_project();
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
    project.file_contains("out-claude-code/CLAUDE.md", CONTEXT_MARKER);
    let mcp = project.json_file("out-claude-code/.mcp.json");
    assert_eq!(mcp["mcpServers"]["toolserver"]["command"], MCP_COMMAND);
    assert!(project.registers_hook(
        "out-claude-code/.claude/settings.json",
        "SessionStart",
        HOOK_COMMAND
    ));
    project.file_contains(
        "out-claude-code/.claude/commands/team-onboarding.md",
        COMMAND_MARKER,
    );
}

// j000400_multi_engine.feature: Scenario: A home-keyed engine materializes its
// native context, and reports the surfaces it does not carry
#[test]
fn test_j000400_a_home_keyed_engine_reports_the_surfaces_it_does_not_carry() {
    let project = team_project();
    let ran = project.run(&[
        "materialize",
        "team",
        "--target",
        "out-mock-launch",
        "--backend",
        "mock-launch",
    ]);
    ran.succeeds();
    project.file_contains("out-mock-launch/MOCK_CONTEXT.md", CONTEXT_MARKER);
    for (needle, what) in [
        (HOOK_COMMAND, "hook"),
        (MCP_COMMAND, "MCP server"),
        (COMMAND_MARKER, "command"),
    ] {
        for (path, bytes) in project.tree("out-mock-launch") {
            assert!(
                !String::from_utf8_lossy(&bytes).contains(needle),
                "{path} carries the shared {what}"
            );
        }
    }
    let doc = ran.json();
    let carried = doc["engines"][0]["not_carried"].to_string();
    for surface in ["mcp", "hooks", "commands"] {
        assert!(
            carried.contains(surface),
            "{surface} not named in {carried}"
        );
    }
}

// j000400_multi_engine.feature: Scenario Outline: A hand-authored context file
// survives materialization byte-for-byte
#[test]
fn test_j000400_a_hand_authored_context_file_survives_materialization_byte_for_byte() {
    for (engine, file) in [("claude-code", "CLAUDE.md"), ("mock", "MOCK_CONTEXT.md")] {
        let project = team_project();
        let authored = "# Team conventions\n\n<!-- Alice wrote this by hand; ctxloom must never touch it -->\nJ000400-HAND-AUTHORED-MARKER-6d2e73: always use tabs, never spaces\n\n## House style\n\n- tabs, not spaces\n- no trailing whitespace\n";
        let target = format!("out-{engine}");
        project.write(&format!("{target}/{file}"), authored);
        project
            .run(&[
                "materialize",
                "team",
                "--target",
                &target,
                "--backend",
                engine,
            ])
            .succeeds();
        let held = project.read(&format!("{target}/{file}")).expect("the file");
        let prefix = authored.trim_end_matches('\n');
        assert!(held.starts_with(prefix), "{file}: {held:?}");
        assert!(held.contains(CONTEXT_MARKER), "{file}: {held:?}");
    }
}

const DOCTOR_SKILL: &str = "---\nname: ctxloom-doctor\ndescription: \"DOCTOR-SKILL-MARKER-7d4e21 - Validate a ctxloom setup: dependencies, agents, version currency, hooks and MCP; run before relying on isolation.\"\n---\n\n# ctxloom doctor\n\n## DOCTOR-CHECK-DEPS-a1: required binaries on PATH\n\n## DOCTOR-CHECK-HOOKS-TRUST-d4: hooks and MCP\n";

// j000600_agent_skill.feature: Scenario: Alice's assistant gets the skill she
// wrote, guidance and script both
#[test]
fn test_j000600_alices_assistant_gets_the_skill_she_wrote_guidance_and_script_both() {
    let project = Project::initialized();
    let skills = ".ctxloom/content/bundles/v2/ops/skills";
    project.write(
        ".ctxloom/content/bundles/v2/ops/bundle.yaml",
        "schema_version: 4\nversion: 1.0.0\n",
    );
    project.write(&format!("{skills}/ctxloom-doctor/SKILL.md"), DOCTOR_SKILL);
    project.write(
        &format!("{skills}/ctxloom-doctor/scripts/run.sh"),
        "#!/bin/sh\necho DOCTOR-SCRIPT-MARKER-9c2f\n",
    );
    project.write(
        &format!("{skills}/.ctxloom-doctor.meta.yaml"),
        "executable:\n  - scripts/run.sh\n",
    );
    project.profile("clinic", "ops");
    project
        .run(&[
            "materialize",
            "clinic",
            "--target",
            "out-claude-code",
            "--backend",
            "claude-code",
        ])
        .succeeds();
    let skill = "out-claude-code/.claude/skills/ctxloom-doctor";
    for marker in [
        "DOCTOR-SKILL-MARKER-7d4e21",
        "DOCTOR-CHECK-DEPS-a1",
        "DOCTOR-CHECK-HOOKS-TRUST-d4",
    ] {
        project.file_contains(&format!("{skill}/SKILL.md"), marker);
    }
    assert!(project.is_executable(&format!("{skill}/scripts/run.sh")));
    project.file_contains(
        &format!("{skill}/scripts/run.sh"),
        "DOCTOR-SCRIPT-MARKER-9c2f",
    );
}

// Go's fsstatic suite pins the ownership record's promise for a hand-authored
// settings file: what the user wrote comes back as it was once ctxloom's
// entries are released (manage.feature: "Materializing preserves the exact
// numbers" and "Uninstall strips the harness" together).
#[test]
fn test_releasing_a_materialize_over_a_hand_written_settings_file_gives_it_back_byte_for_byte() {
    let project = Project::initialized();
    let theirs = "{\n  \"permissions\": { \"allow\": [\"Read\"] }\n}\n";
    project.write(".claude/settings.json", theirs);
    project.run(&["materialize", "--yes"]).succeeds();
    assert_ne!(
        project.read(".claude/settings.json").as_deref(),
        Some(theirs)
    );
    project
        .run(&["materialize", "--release", "--yes"])
        .succeeds();
    assert_eq!(
        project.read(".claude/settings.json").as_deref(),
        Some(theirs)
    );
}
