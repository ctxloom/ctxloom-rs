//! The exit ladder (D28, `exit_codes_test.go`): 0, 1, 2 for a refusal, 3 for
//! fatal findings; reached through the test-only commands.

use super::run;

#[test]
fn test_the_exit_ladder() {
    for (args, code) in [
        (&["test", "sample"][..], 0),
        (&["test", "refuse"], 2),
        (&["test", "findings"], 3),
    ] {
        let ran = run(args);
        assert_eq!(ran.code, code, "{args:?}: {}", ran.said());
    }
}

#[test]
fn test_a_refusal_is_said_on_stderr_and_answers_nothing() {
    // A refusal is an error: it is said on stderr, and stdout holds answers only.
    let ran = run(&["--format", "text", "test", "refuse"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert_eq!(ran.stdout, "", "{}", ran.said());
    assert_ne!(ran.stderr, "", "{}", ran.said());
}

#[test]
fn test_a_usage_error_is_a_refusal_too() {
    for args in [&["frobnicate"][..], &["--format", "yaml", "version"]] {
        let ran = run(args);
        assert_eq!(ran.code, 2, "{args:?}: {}", ran.said());
    }
}
