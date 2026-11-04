//! Plain tests for what post_tool_hooks.feature does not cover (the feature
//! runs in `gherkin_surfaces.test.rs`): `hook skill-mates`.
//!
//! plan supersedes Go (8.4, "`--engine` on every callback"): the engine whose
//! codec reads the payload and the transcript is the one the hook was
//! registered under (`--engine`), not one looked up from the session index,
//! so the "session index records harp ... on engine" step is not a
//! precondition here.

use super::library::Project;
use super::surfaces::additional_context;

const HARP: &str = "brisk-copper-moth";

/// Go's `steps_turn_transcript.go`: one user prompt, one assistant message.
const TRANSCRIPT: &str = concat!(
    r#"{"type":"user","isSidechain":false,"cwd":"/repo","sessionId":"s","version":"2.1.44","message":{"role":"user","content":[{"type":"text","text":"go"}]},"uuid":"u1","timestamp":"2026-08-22T10:00:00.000Z"}"#,
    "\n",
    r#"{"type":"assistant","isSidechain":false,"cwd":"/repo","sessionId":"s","version":"2.1.44","message":{"model":"m","id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"The queue is admitted."}],"stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}},"uuid":"a1","timestamp":"2026-08-22T10:00:02.000Z"}"#,
    "\n"
);

/// The two linked skills of the scenario, a profile `ops` over their bundle
/// and the default agent bound to it.
fn nightly_project() -> Project {
    let project = Project::initialized();
    let root = ".ctxloom/content/bundles/v2/nightly";
    project.write(
        &format!("{root}/bundle.yaml"),
        "schema_version: 4\nversion: \"1.0\"\n",
    );
    project.write(
        &format!("{root}/skills/admit/SKILL.md"),
        "---\nname: admit\ndescription: Admit work to the nightly queue.\n---\n\nAdmit the queue.\n",
    );
    project.write(
        &format!("{root}/skills/unattended/SKILL.md"),
        "---\nname: unattended\ndescription: Work the admitted queue.\n---\n\nWork the queue.\n",
    );
    for skill in ["admit", "unattended"] {
        project.write(
            &format!("{root}/skills/.{skill}.meta.yaml"),
            "tags:\n  - ctxloom:link_id=nightly\n",
        );
    }
    project.profile("ops", "nightly");
    project
        .run(&[
            "agent",
            "create",
            "default",
            "--llm",
            "claude-code",
            "--profiles",
            "ops",
        ])
        .succeeds();
    project.run(&["agent", "default", "default"]).succeeds();
    project.write("turn.jsonl", TRANSCRIPT);
    project
}

fn payload(project: &Project, tool: &str, input: &str) -> String {
    format!(
        r#"{{"session_id":"s","hook_event_name":"PostToolUse","transcript_path":"{}/turn.jsonl","cwd":"/repo","tool_name":"{tool}","tool_input":{input},"tool_response":"ok"}}"#,
        project.dir().display()
    )
}

// post_tool_hooks.feature (a documented condition of skillsInvoked): a mate the
// session has already invoked is not named again.
#[test]
fn test_a_mate_the_session_already_invoked_is_not_named_again() {
    let project = nightly_project();
    let invoked = r#"{"type":"assistant","isSidechain":false,"message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Skill","input":{"skill":"unattended"}}]}}"#;
    project.write("turn.jsonl", &format!("{TRANSCRIPT}{invoked}\n"));
    let ran = project.run_input(
        &["hook", "skill-mates", "--engine", "claude-code"],
        &payload(&project, "Skill", r#"{"skill":"admit","args":""}"#),
        &[("CTXLOOM_SESSION_HARP", HARP)],
    );
    ran.succeeds();
    assert_eq!(additional_context(&ran), "");
}

// session_hooks.feature (the engine codec seam): an engine the hooks do not
// know is refused by name, not decoded with another's codec.
#[test]
fn test_a_hook_under_an_unknown_engine_is_refused_by_name() {
    let project = Project::initialized();
    project
        .run_input(&["hook", "skill-mates", "--engine", "nosuch"], "{}", &[])
        .fails()
        .contains("nosuch");
}
