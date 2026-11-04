//! Errors (`pkg/clifmt`): text is `ctxloom: <message>` then `  fix:
//! <remedy>`; under an explicit structured format the envelope `{code, args,
//! message}` is on stderr and stdout holds answers only.

use serde_json::Value;

use super::run;

/// `value` is an envelope: exactly `code`, `args` and `message`.
fn assert_envelope(value: &Value, what: &str) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{what}: {value}"));
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["args", "code", "message"], "{what}: {value}");
    assert!(
        value["code"].as_str().is_some_and(|c| !c.is_empty()),
        "{what}"
    );
    assert!(value["args"].is_object(), "{what}: {value}");
    assert!(
        value["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{what}"
    );
}

#[test]
fn test_a_refusal_under_json_is_an_envelope_on_stderr() {
    let ran = run(&["--format", "json", "test", "refuse"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert_eq!(ran.stdout, "", "stdout holds answers only");
    assert_envelope(&ran.err_json(), "a refusal");
}

#[test]
fn test_a_usage_error_under_json_is_an_envelope_on_stderr() {
    for args in [
        &["--format", "json", "frobnicate"][..],
        &["--format=json", "frobnicate"],
    ] {
        let ran = run(args);
        assert_eq!(ran.code, 2, "{args:?}: {}", ran.said());
        assert_eq!(ran.stdout, "", "{args:?}");
        assert_envelope(&ran.err_json(), "a usage error");
    }
}

#[test]
fn test_the_envelope_s_message_is_the_text_error_s() {
    let json = run(&["--format", "json", "test", "refuse"]);
    let text = run(&["--format", "text", "test", "refuse"]);
    let message = json.err_json()["message"]
        .as_str()
        .expect("a message")
        .to_string();
    assert_eq!(
        text.stderr.lines().next(),
        Some(format!("ctxloom: {message}").as_str()),
        "{}",
        text.said()
    );
}

#[test]
fn test_a_refusal_under_text_is_the_message_then_its_fix() {
    let ran = run(&["--format", "text", "test", "refuse"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert!(ran.stderr.starts_with("ctxloom: "), "{}", ran.said());
    assert!(
        ran.stderr.lines().any(|l| l.starts_with("  fix: ")),
        "{}",
        ran.said()
    );
    assert!(serde_json::from_str::<Value>(&ran.stderr).is_err());
}

#[test]
fn test_off_a_terminal_an_error_is_the_envelope_with_no_explicit_format() {
    // D28: json off a terminal. No --format; stdout is a pipe here.
    for args in [&["test", "refuse"][..], &["frobnicate"]] {
        let ran = run(args);
        assert_eq!(ran.code, 2, "{args:?}: {}", ran.said());
        assert_eq!(ran.stdout, "", "{args:?}");
        assert_envelope(&ran.err_json(), "an error off a terminal");
    }
}
