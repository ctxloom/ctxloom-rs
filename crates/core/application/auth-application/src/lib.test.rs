//! Which credentials are exported, and what is said of each. Ported from Go's
//! `auth status` (internal/adapters/cli/auth.go, release/0.7) and
//! auth.feature; the doctor's rules are Go's `ConfiguredCredentialChecks`.

use std::sync::{Arc, Mutex};

use ctxloom_ports::ExportedVars;
use serde_json::json;

use super::*;

/// An environment that exports exactly the names it is given, and records
/// every name asked: the only thing ever asked of it.
struct Env {
    exported: Vec<&'static str>,
    asked: Mutex<Vec<String>>,
}

impl ExportedVars for Env {
    fn is_exported(&self, name: &str) -> bool {
        self.asked.lock().unwrap().push(name.to_string());
        self.exported.contains(&name)
    }
}

fn auth(exported: &[&'static str]) -> Auth {
    Auth::new(Arc::new(Env {
        exported: exported.to_vec(),
        asked: Mutex::new(Vec::new()),
    }))
}

// auth.feature: "An exported token is reported present without revealing it".
#[test]
fn an_exported_claude_token_is_reported_present_by_the_variable_it_is_read_from() {
    let rows = auth(&["CLAUDE_CODE_OAUTH_TOKEN"]).status();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].engine, "claude-code");
    assert_eq!(rows[0].mode, "token");
    assert!(rows[0].present);
    assert_eq!(rows[0].vars, ["CLAUDE_CODE_OAUTH_TOKEN"]);
    assert_eq!(rows[0].remedy, "");
}

// auth.feature: "With nothing exported, status names how to mint and export
// the token".
#[test]
fn a_missing_claude_token_is_reported_missing_with_how_to_mint_and_export_it() {
    let rows = auth(&[]).status();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].present);
    assert!(rows[0].vars.is_empty());
    assert_eq!(
        rows[0].remedy,
        "run `claude setup-token` and export CLAUDE_CODE_OAUTH_TOKEN (or store it in your secret manager)"
    );
}

// Go: opencode's endpoint key is optional, so it has a row only when exported.
#[test]
fn opencodes_optional_key_has_a_row_only_when_it_is_exported() {
    let rows = auth(&["CLAUDE_CODE_OAUTH_TOKEN", "CTXLOOM_OPENCODE_API_KEY"]).status();
    let names: Vec<&str> = rows.iter().map(|r| r.engine).collect();
    assert_eq!(names, ["claude-code", "opencode"]);
    assert!(rows[1].present);
    assert_eq!(rows[1].vars, ["CTXLOOM_OPENCODE_API_KEY"]);
    let rows = auth(&[]).status();
    assert!(rows.iter().all(|r| r.engine != "opencode"));
}

// Go's emitted shape: engine, mode, present always; vars and remedy only
// when there are some.
#[test]
fn a_row_serializes_to_gos_shape() {
    let rows = auth(&["CLAUDE_CODE_OAUTH_TOKEN"]).status();
    assert_eq!(
        serde_json::to_value(&rows).unwrap(),
        json!([{
            "engine": "claude-code",
            "mode": "token",
            "present": true,
            "vars": ["CLAUDE_CODE_OAUTH_TOKEN"],
        }])
    );
    let rows = auth(&[]).status();
    let doc = serde_json::to_value(&rows).unwrap();
    assert_eq!(doc[0]["present"], false);
    assert!(doc[0].get("vars").is_none());
    assert!(doc[0]["remedy"]
        .as_str()
        .unwrap()
        .contains("claude setup-token"));
}

// The secret appears in no output: the port can only say whether a name is
// exported, so there is no value to leak, and only credential names are asked.
#[test]
fn only_the_credential_variables_are_asked_about_and_only_whether_they_are_exported() {
    let env = Arc::new(Env {
        exported: vec!["CLAUDE_CODE_OAUTH_TOKEN"],
        asked: Mutex::new(Vec::new()),
    });
    let _ = Auth::new(env.clone()).status();
    let mut asked = env.asked.lock().unwrap().clone();
    asked.sort();
    assert_eq!(
        asked,
        ["CLAUDE_CODE_OAUTH_TOKEN", "CTXLOOM_OPENCODE_API_KEY"]
    );
}

// doctor.feature: "A missing agent credential is a fail row".
#[test]
fn a_configured_claude_without_its_token_is_not_ready_and_says_the_fix() {
    let verdicts = auth(&[]).needed(&["claude-code".to_string()], "token");
    assert_eq!(verdicts.len(), 1);
    let v = &verdicts[0];
    assert_eq!(
        (v.engine, v.mode, v.var),
        ("claude-code", "token", "CLAUDE_CODE_OAUTH_TOKEN")
    );
    assert!(!v.ready);
    assert!(v.remedy.contains("claude setup-token"));
}

#[test]
fn a_configured_claude_with_its_token_is_ready() {
    let verdicts = auth(&["CLAUDE_CODE_OAUTH_TOKEN"]).needed(&["claude-code".to_string()], "token");
    assert_eq!(verdicts.len(), 1);
    assert!(verdicts[0].ready);
}

// Go: opencode is keyless without its key, which is not a refusal.
#[test]
fn opencode_is_ready_without_its_optional_key() {
    let verdicts = auth(&[]).needed(&["opencode".to_string()], "token");
    assert_eq!(verdicts.len(), 1);
    assert!(verdicts[0].ready);
    assert_eq!(verdicts[0].engine, "opencode");
}

// Go: a login shared in place reads nothing from the environment.
#[test]
fn login_mode_needs_nothing_from_the_environment() {
    assert!(auth(&[])
        .needed(&["claude-code".to_string()], "login")
        .is_empty());
}

#[test]
fn an_engine_with_no_credential_rule_is_not_asked_about() {
    assert!(auth(&[]).needed(&["mock".to_string()], "token").is_empty());
}

#[test]
fn each_engine_is_reported_once_whatever_its_labels() {
    let engines = ["claude-code".to_string(), "claude-code".to_string()];
    assert_eq!(auth(&[]).needed(&engines, "token").len(), 1);
}

#[test]
fn a_mode_that_is_not_login_is_the_default_token() {
    assert_eq!(auth(&[]).needed(&["claude-code".to_string()], "").len(), 1);
}
