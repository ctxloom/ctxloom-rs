//! The pure checks. Go: `cli/doctor.feature` and `doctor_cmd_checks_test.go`
//! (release/0.7).

use crate::checks::{self, Status};
use crate::hooks::Stale;

fn files(path: &str, text: &str) -> Vec<(String, String)> {
    vec![(path.to_string(), text.to_string())]
}

// doctor.feature: "A blanket ignore rule is reported as a warning".
#[test]
fn a_blanket_ignore_rule_warns_and_names_the_install() {
    for rule in [".ctxloom/", ".ctxloom", "/.ctxloom/", ".ctxloom/*"] {
        let check = checks::gitignore(Some(&format!("target/\n{rule}\n")));
        assert_eq!(check.status, Status::Warn, "{rule}");
        assert!(check.detail.contains("blanket"));
        assert_eq!(check.remedy, "ctxloom manage gitignore install");
    }
}

// doctor.feature: "The current granular ignore patterns keep the check quiet".
#[test]
fn the_granular_patterns_and_no_gitignore_are_quiet() {
    let granular = checks::gitignore(Some(".ctxloom/cache/\n.ctxloom/sessions/\n"));
    assert_eq!(granular.status, Status::Ok);
    assert_eq!(checks::gitignore(None).status, Status::Ok);
}

// doctor.feature: "Doctor names a settings file whose ctxloom entry launches a
// server that does not exist" (written by hand: no record, so by hand).
#[test]
fn a_launched_ctxloom_mcp_server_is_named_with_the_by_hand_remedy() {
    let bad =
        r#"{"mcpServers":{"ctxloom":{"command":"/usr/local/bin/ctxloom","args":["mcp","serve"]}}}"#;
    let check = checks::mcp_invocation(&files(".mcp.json", bad));
    assert_eq!(check.status, Status::Warn);
    assert!(check.detail.contains(".mcp.json") && check.detail.contains("entry from"));
    assert!(check.remedy.contains("by hand"));
    assert!(!check
        .remedy
        .contains("ctxloom materialize --release --surface mcp"));
}

// doctor.feature: "Doctor stays quiet about a settings file that names the
// session's endpoint".
#[test]
fn an_endpoint_url_launches_nothing() {
    let good = r#"{"mcpServers":{"ctxloom":{"type":"http","url":"http://127.0.0.1:4242/mcp"}}}"#;
    assert_eq!(
        checks::mcp_invocation(&files(".mcp.json", good)).status,
        Status::Ok
    );
    let other = r#"{"mcpServers":{"ctxloom":{"command":"other","args":["mcp","serve"]}}}"#;
    assert_eq!(
        checks::mcp_invocation(&files(".mcp.json", other)).status,
        Status::Ok
    );
    assert_eq!(
        checks::mcp_invocation(&files(".mcp.json", "not json")).status,
        Status::Ok
    );
}

// doctor.feature: "The delivered-versus-ingested boundary is stated, not
// claimed as checked": info, never ok.
#[test]
fn the_ingestion_limit_is_info_and_says_whose_process_it_is() {
    let check = checks::ingestion();
    assert_eq!(check.status, Status::Info);
    assert!(check.detail.contains("a process ctxloom does not own"));
}

// doctor.feature: "A config that will not parse does not stop the diagnosis".
#[test]
fn a_broken_config_is_a_warning_not_a_failure() {
    let check = checks::setup_marker(true, Some("schema_version: [unclosed\n  - : :\n"));
    assert_eq!(check.status, Status::Warn);
    assert_eq!(
        checks::setup_marker(true, Some("schema_version: 8\n")).status,
        Status::Ok
    );
    assert_eq!(checks::setup_marker(false, None).status, Status::Warn);
    assert_eq!(checks::setup_marker(true, None).status, Status::Warn);
}

// doctor.feature: --deps "binaries on PATH and git identity".
#[test]
fn deps_want_git_and_an_identity() {
    assert_eq!(checks::deps(false, None).status, Status::Warn);
    assert_eq!(checks::deps(true, None).status, Status::Warn);
    let identity = "[core]\n\tbare = false\n[user]\n\tname = A\n\temail = a@b.c\n";
    assert_eq!(checks::deps(true, Some(identity)).status, Status::Ok);
    let half = "[user]\n\tname = A\n";
    assert_eq!(checks::deps(true, Some(half)).status, Status::Warn);
    // The keys count only inside [user].
    let elsewhere = "[alias]\n\tname = x\n\temail = y\n";
    assert_eq!(checks::deps(true, Some(elsewhere)).status, Status::Warn);
}

#[test]
fn an_agent_with_no_llm_resolves_to_nothing() {
    let bad = "schema_version: 8\nagents:\n  coder:\n    profiles: [p]\n";
    assert_eq!(checks::agents(Some(bad)).status, Status::Warn);
    let good = "schema_version: 8\nagents:\n  coder:\n    llm: claude-code\n";
    assert_eq!(checks::agents(Some(good)).status, Status::Ok);
    assert_eq!(
        checks::agents(Some("schema_version: 8\n")).status,
        Status::Ok
    );
}

#[test]
fn the_stale_hooks_row_names_the_event_the_subcommand_and_the_fix() {
    let stale = vec![(
        ".claude/settings.json".to_string(),
        Stale {
            event: "SessionStart".to_string(),
            sub: "inject-context".to_string(),
        },
    )];
    let check = checks::stale_hooks(&stale);
    assert_eq!(check.status, Status::Warn);
    assert!(check
        .detail
        .contains("SessionStart runs `ctxloom hook inject-context`"));
    assert_eq!(check.remedy, "ctxloom doctor --fix");
    assert_eq!(checks::stale_hooks(&[]).status, Status::Ok);
}

#[test]
fn the_version_row_says_the_version() {
    assert!(checks::version("1.2.3").detail.contains("1.2.3"));
}

#[test]
fn registered_hooks_are_ok_and_none_is_info() {
    let on = files(
        ".claude/settings.json",
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"ctxloom hook session-start"}]}]}}"#,
    );
    assert_eq!(checks::hooks(&on).status, Status::Ok);
    assert_eq!(checks::hooks(&[]).status, Status::Info);
}
