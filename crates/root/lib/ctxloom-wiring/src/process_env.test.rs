//! The launch environment, as far as credentials go: names exported with a
//! value, and never the values.

use std::ffi::OsString;

use super::*;

fn env(vars: &[(&str, &str)]) -> ProcessEnv {
    ProcessEnv::of(
        vars.iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    )
}

#[test]
fn a_variable_with_a_value_is_exported() {
    assert!(env(&[("CLAUDE_CODE_OAUTH_TOKEN", "x")]).is_exported("CLAUDE_CODE_OAUTH_TOKEN"));
}

// auth.feature sets a variable to "" to say it is not exported.
#[test]
fn a_variable_set_to_nothing_is_not_exported() {
    assert!(!env(&[("CLAUDE_CODE_OAUTH_TOKEN", "")]).is_exported("CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn a_variable_that_is_not_set_is_not_exported() {
    assert!(!env(&[("OTHER", "x")]).is_exported("CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn the_name_is_matched_whole() {
    let env = env(&[("CLAUDE_CODE_OAUTH_TOKEN_EXTRA", "x")]);
    assert!(!env.is_exported("CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn a_value_that_is_not_unicode_still_counts_as_exported() {
    let env = ProcessEnv::of([(
        OsString::from("CLAUDE_CODE_OAUTH_TOKEN"),
        os::not_unicode("t"),
    )]);
    assert!(env.is_exported("CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn the_process_environment_is_read() {
    // PATH is set under any test runner, and is not a credential.
    assert!(ProcessEnv::from_process().is_exported("PATH"));
}
