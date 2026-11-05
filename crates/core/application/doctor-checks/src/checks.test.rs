//! The pure checks. Go: `cli/doctor.feature` and `doctor_cmd_checks_test.go`
//! (release/0.7).

use std::sync::Arc;

use auth_application::Auth;
use ctxloom_ports::ExportedVars;

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
    // Each key is needed on its own, and as a key with a value.
    let only_email = "[user]\n\temail = a@b.c\n";
    assert_eq!(checks::deps(true, Some(only_email)).status, Status::Warn);
    let bare_name = "[user]\n\tname\n\temail = a@b.c\n";
    assert_eq!(checks::deps(true, Some(bare_name)).status, Status::Warn);
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
    assert!(!check.detail.contains("entries in all"), "{}", check.detail);
    for count in [2, 3] {
        let many: Vec<_> = (0..count).map(|_| stale[0].clone()).collect();
        assert!(checks::stale_hooks(&many)
            .detail
            .ends_with(&format!("; {count} entries in all")),);
    }
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

/// The variables a launch exports.
struct Exports(&'static [&'static str]);

impl ExportedVars for Exports {
    fn is_exported(&self, name: &str) -> bool {
        self.0.contains(&name)
    }
}

const TOKEN: &[&str] = &["CLAUDE_CODE_OAUTH_TOKEN"];

fn credentials(layers: &[&str], exported: &'static [&'static str]) -> checks::Check {
    let layers: Vec<String> = layers.iter().map(ToString::to_string).collect();
    checks::credentials(&layers, &Auth::new(Arc::new(Exports(exported))))
}

// doctor.feature: "A missing agent credential is a fail row with run's fix".
#[test]
fn a_missing_claude_token_is_a_fail_row_with_runs_fix() {
    let check = credentials(&["schema_version: 8\n"], &[]);
    assert_eq!(check.marker, "DOCTOR-CHECK-CREDENTIALS-m5");
    assert_eq!(check.status, Status::Fail);
    assert!(check.detail.contains("claude-code"), "{}", check.detail);
    assert!(check
        .detail
        .contains("CLAUDE_CODE_OAUTH_TOKEN is not exported"));
    assert!(check.remedy.contains("claude setup-token"));
}

#[test]
fn an_exported_token_is_ok_and_the_row_says_it_does_not_test_acceptance() {
    let check = credentials(&["schema_version: 8\n"], TOKEN);
    assert_eq!(check.status, Status::Ok);
    assert!(
        check.detail.contains("claude-code (auth token)"),
        "{}",
        check.detail
    );
    assert!(check
        .detail
        .contains("does not test whether the engine accepts it"));
    assert_eq!(check.remedy, "");
}

// No project and no config at all is the shipped default: claude-code.
#[test]
fn with_no_config_at_all_the_shipped_default_engine_is_checked() {
    assert_eq!(credentials(&[], &[]).status, Status::Fail);
    assert_eq!(credentials(&[], TOKEN).status, Status::Ok);
}

// Go: the mode is the human's session auth (`auth:`); a login is shared in
// place and reads nothing from the environment.
#[test]
fn login_mode_needs_no_credential_from_the_environment() {
    let check = credentials(&["schema_version: 8\nauth: login\n"], &[]);
    assert_eq!(check.status, Status::Info);
    assert!(check
        .detail
        .contains("no configured engine needs a credential from you"));
}

#[test]
fn the_auth_mode_of_the_highest_layer_wins() {
    let home = "schema_version: 8\nauth: login\n";
    let project = "schema_version: 8\nauth: token\n";
    assert_eq!(credentials(&[home, project], &[]).status, Status::Fail);
    assert_eq!(credentials(&[project, home], &[]).status, Status::Info);
}

// Go: a backend is one a configured label or agent resolves to.
#[test]
fn an_authored_label_replaces_the_shipped_default_engine() {
    let config = "schema_version: 8\nllm:\n  configs:\n    local:\n      type: mock\n";
    let check = credentials(&[config], &[]);
    assert_eq!(check.status, Status::Info, "{}", check.detail);
}

#[test]
fn a_labels_engine_is_checked_whatever_the_label_is_called() {
    let config = "schema_version: 8\nllm:\n  configs:\n    work:\n      type: claude-code\n";
    assert_eq!(credentials(&[config], &[]).status, Status::Fail);
}

#[test]
fn opencode_is_keyless_without_its_optional_key_and_never_fails() {
    let config = "schema_version: 8\nllm:\n  configs:\n    oc:\n      type: opencode\n";
    let check = credentials(&[config], &[]);
    assert_eq!(check.status, Status::Ok);
    assert!(
        check.detail.contains("opencode (auth token)"),
        "{}",
        check.detail
    );
}

#[test]
fn an_agent_naming_an_engine_directly_is_checked_too() {
    let config = "schema_version: 8\nllm:\n  configs:\n    local:\n      type: mock\nagents:\n  dev:\n    llm: claude-code\n";
    assert_eq!(credentials(&[config], &[]).status, Status::Fail);
}

#[test]
fn an_agent_naming_a_label_is_checked_as_that_labels_engine() {
    let config = "schema_version: 8\nllm:\n  configs:\n    local:\n      type: mock\n    work:\n      type: claude-code\nagents:\n  dev:\n    llm: work\n";
    assert_eq!(credentials(&[config], &[]).status, Status::Fail);
}

#[test]
fn a_later_layer_overrides_a_label_of_an_earlier_one() {
    let home = "schema_version: 8\nllm:\n  configs:\n    work:\n      type: claude-code\n";
    let project = "schema_version: 8\nllm:\n  configs:\n    work:\n      type: mock\n";
    assert_eq!(credentials(&[home, project], &[]).status, Status::Info);
}

#[test]
fn a_config_that_will_not_parse_is_the_setup_markers_to_report_and_checks_the_default() {
    let check = credentials(&["schema_version: [unclosed\n"], &[]);
    assert_eq!(check.status, Status::Fail);
}
