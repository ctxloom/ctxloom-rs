//! Usage: an unknown verb is an error that suggests and lists; `-h` works at
//! every depth; there is no `help` command (`config.feature`,
//! `group_node.go`).

use super::run;

/// The subcommands of `ctxloom test`, which the unknown-verb error lists.
const TEST_VERBS: [&str; 7] = [
    "refuse",
    "findings",
    "sample",
    "diagnostics",
    "session-id",
    "run-id",
    "open-store",
];

#[test]
fn test_an_unknown_verb_fails_with_did_you_mean_and_lists_the_subcommands() {
    let ran = run(&["frobnicate"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert_eq!(ran.stdout, "");
    assert!(ran.stderr.contains("Did you mean"), "{}", ran.said());
    assert!(ran.stderr.contains("version"), "{}", ran.said());
}

#[test]
fn test_an_unknown_verb_is_an_error_even_with_help() {
    for args in [&["frobnicate", "--help"][..], &["frobnicate", "-h"]] {
        let ran = run(args);
        assert_eq!(ran.code, 2, "{args:?}: {}", ran.said());
        assert_eq!(ran.stdout, "", "{args:?}");
        assert!(
            ran.stderr.contains("Did you mean"),
            "{args:?}: {}",
            ran.said()
        );
    }
}

#[test]
fn test_an_unknown_verb_below_a_namespace_lists_that_namespace_s_verbs() {
    let ran = run(&["test", "sampel"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    assert!(ran.stderr.contains("Did you mean"), "{}", ran.said());
    for verb in TEST_VERBS {
        assert!(ran.stderr.contains(verb), "{verb}: {}", ran.said());
    }
}

#[test]
fn test_h_works_at_depth() {
    for (args, usage) in [
        (&["-h"][..], "ctxloom"),
        (&["version", "-h"], "ctxloom version"),
        (&["test", "-h"], "ctxloom test"),
        (&["test", "sample", "-h"], "ctxloom test sample"),
    ] {
        let ran = run(args);
        assert_eq!(ran.code, 0, "{args:?}: {}", ran.said());
        assert!(ran.stdout.contains("Usage:"), "{args:?}: {}", ran.said());
        assert!(ran.stdout.contains(usage), "{args:?}: {}", ran.said());
        assert_eq!(ran.stderr, "", "{args:?}");
    }
}

#[test]
fn test_there_is_no_help_command() {
    for args in [&["help"][..], &["test", "help"], &["help", "version"]] {
        let ran = run(args);
        assert_eq!(ran.code, 2, "{args:?}: {}", ran.said());
        assert_eq!(ran.stdout, "", "{args:?}");
    }
    // Nor is one listed: no line of the commands starts with the word.
    let listing = run(&["-h"]);
    assert_eq!(listing.code, 0, "{}", listing.said());
    let listed = listing
        .stdout
        .lines()
        .any(|line| line.split_whitespace().next() == Some("help"));
    assert!(!listed, "{}", listing.stdout);
}
