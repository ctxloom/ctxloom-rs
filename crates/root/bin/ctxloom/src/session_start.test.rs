//! The surface rows of Go's `cli/session_hooks.feature` (release/0.7): `hook
//! session-start`. Each test names its scenario. (`session-bind`, `hud` and
//! the retired `turn-changed` are S4's and are tested in `hook.test.rs`;
//! `stamp-plan` is taskloom's, D17.)

use super::library::Project;
use super::surfaces::additional_context;

// session_hooks.feature: Scenario: The SessionStart hook never hands the
// engine the project's context
#[test]
fn test_the_session_start_hook_never_hands_the_engine_the_projects_context() {
    let project = Project::initialized();
    project.write(
        ".ctxloom/cache/context/abc123.md",
        "HOOK-MUST-NOT-DELIVER-THIS-EXACT-TEXT and nothing else in this project says so.\n",
    );
    let ran = project.run_input(
        &["hook", "session-start", "--engine", "claude-code"],
        r#"{"session_id":"vendor-session-1","hook_event_name":"SessionStart","source":"startup"}"#,
        &[],
    );
    ran.succeeds();
    assert_eq!(additional_context(&ran), "");
    ran.lacks("HOOK-MUST-NOT-DELIVER-THIS-EXACT-TEXT");
}

// Go's `operations.AgentSetupNudge` (agents.go, release/0.7): a project with
// profiles but no agents is nudged, on the user-notice channel of the
// envelope (`systemMessage`), never as model context.
#[test]
fn test_the_session_start_hook_nudges_a_project_with_profiles_and_no_agents() {
    let project = Project::initialized();
    project.profile("dev", "demo");
    let ran = project.run_input(
        &["hook", "session-start", "--engine", "claude-code"],
        r#"{"session_id":"s","hook_event_name":"SessionStart","source":"startup"}"#,
        &[],
    );
    ran.succeeds();
    let doc: serde_json::Value = serde_json::from_str(ran.stdout.trim()).expect("one JSON object");
    assert!(
        doc["systemMessage"]
            .as_str()
            .is_some_and(|m| m.contains("profiles but no agents configured")),
        "{}",
        ran.said()
    );
    assert_eq!(additional_context(&ran), "");
}

// Go's `AgentSetupNudge`: once an agent exists the nudge goes silent.
#[test]
fn test_the_session_start_hook_is_silent_once_an_agent_exists() {
    let project = Project::initialized();
    project.profile("dev", "demo");
    project
        .run(&[
            "agent",
            "create",
            "default",
            "--llm",
            "claude-code",
            "--profiles",
            "dev",
        ])
        .succeeds();
    let ran = project.run_input(
        &["hook", "session-start", "--engine", "claude-code"],
        r#"{"session_id":"s","hook_event_name":"SessionStart","source":"startup"}"#,
        &[],
    );
    ran.succeeds();
    assert!(!ran.stdout.contains("systemMessage"), "{}", ran.said());
}
