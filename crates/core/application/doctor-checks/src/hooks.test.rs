//! Reading the engine's settings. Go: `cli/doctor.feature` "A hook entry an
//! older ctxloom left behind is found, and --fix removes only it".

use crate::hooks::{launches_ctxloom_mcp, registered, stale, strip};

const SETTINGS: &str = r#"{
  "permissions": {"allow": ["Bash(ls)"]},
  "hooks": {
    "SessionStart": [
      {"matcher": "", "hooks": [
        {"type": "command", "command": "'ctxloom' hook inject-context --part 1 --of 2 abc123"},
        {"type": "command", "command": "ctxloom hook session-start"}
      ]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "my-tool hook inject-context"}]}
    ]
  }
}"#;

#[test]
fn only_ctxloom_hooks_that_are_gone_are_stale() {
    let found = stale(SETTINGS);
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].event.as_str(), found[0].sub.as_str()),
        ("SessionStart", "inject-context")
    );
}

#[test]
fn a_live_hook_and_a_foreign_program_are_not_stale() {
    let live = r#"{"hooks":{"SessionStart":[{"hooks":[
        {"type":"command","command":"ctxloom hook session-start"},
        {"type":"command","command":"my-tool hook inject-context"},
        {"type":"command","command":"/usr/bin/ctxloom hook hud"}]}]}}"#;
    assert!(stale(live).is_empty());
    assert!(strip(live).is_none());
}

#[test]
fn strip_takes_exactly_the_stale_entry() {
    let (fixed, gone) = strip(SETTINGS).expect("something to strip");
    assert_eq!(gone.len(), 1);
    assert!(!fixed.contains("inject-context --part"));
    assert_eq!(fixed.matches("my-tool hook inject-context").count(), 1);
    assert_eq!(fixed.matches("Bash(ls)").count(), 1);
    assert!(fixed.contains("ctxloom hook session-start"));
    assert!(fixed.ends_with('\n'));
}

#[test]
fn a_group_and_an_event_left_empty_go() {
    let only =
        r#"{"hooks":{"SessionStart":[{"hooks":[{"command":"ctxloom hook inject-context abc"}]}]}}"#;
    let (fixed, _) = strip(only).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&fixed).unwrap();
    assert_eq!(doc["hooks"], serde_json::json!({}));
}

#[test]
fn registered_means_any_ctxloom_hook_entry() {
    assert!(registered(SETTINGS));
    assert!(!registered(
        r#"{"hooks":{"Stop":[{"hooks":[{"command":"my-tool hook x"}]}]}}"#
    ));
    assert!(!registered("not json"));
}

#[test]
fn only_a_launched_ctxloom_mcp_is_a_launch() {
    assert!(launches_ctxloom_mcp(
        r#"{"mcpServers":{"ctxloom":{"command":"ctxloom","args":["mcp","serve"]}}}"#
    ));
    assert!(!launches_ctxloom_mcp(
        r#"{"mcpServers":{"ctxloom":{"command":"ctxloom","args":["run"]}}}"#
    ));
    assert!(!launches_ctxloom_mcp(
        r#"{"mcpServers":{"other":{"command":"ctxloom","args":["mcp"]}}}"#
    ));
}
