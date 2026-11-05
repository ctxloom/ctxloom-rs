//! Running git: which variables are scrubbed, and what a failure says.

use super::{environment, is_local, local_variables, run};

fn names(list: &[&str]) -> Option<Vec<String>> {
    Some(list.iter().map(ToString::to_string).collect())
}

#[test]
fn git_names_its_repository_local_variables() {
    let local = local_variables().as_ref().expect("git answers");
    assert!(local.iter().any(|n| n == "GIT_DIR"), "{local:?}");
}

#[test]
fn a_variable_git_calls_local_is_scrubbed_and_others_are_not() {
    let local = names(&["GIT_DIR"]);
    assert!(is_local("GIT_DIR", &local));
    assert!(!is_local("HOME", &local));
    assert!(!is_local("GIT_SSH", &local), "only what git listed");
}

#[test]
fn when_git_cannot_be_asked_every_git_variable_is_scrubbed() {
    assert!(is_local("GIT_INDEX_FILE", &None));
    assert!(!is_local("HOME", &None));
}

#[test]
fn indexed_config_variables_are_scrubbed_only_with_their_count() {
    let counted = names(&["GIT_CONFIG_COUNT"]);
    assert!(is_local("GIT_CONFIG_KEY_0", &counted));
    assert!(is_local("GIT_CONFIG_VALUE_3", &counted));
    assert!(!is_local("GIT_CONFIG_OTHER", &counted));
    assert!(!is_local("FOO", &counted));
    assert!(!is_local("GIT_CONFIG_KEY_0", &names(&["GIT_DIR"])));
}

#[test]
fn the_environment_keeps_the_callers_and_never_prompts() {
    let env = environment();
    let has = |k: &str, v: &str| env.iter().any(|(key, val)| key == k && val == v);
    assert!(env.iter().any(|(k, _)| k == "PATH"));
    assert!(has("GIT_TERMINAL_PROMPT", "0"));
    assert!(has("GIT_ASKPASS", ""));
}

#[test]
fn a_silent_failure_is_named_by_the_command_not_its_flags() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a"), "1").unwrap();
    std::fs::write(dir.path().join("b"), "2").unwrap();
    let said = run(dir.path(), &["diff", "--no-index", "--quiet", "a", "b"]).unwrap_err();
    assert_eq!(said, "git diff: exit status: 1");
}

#[test]
fn a_directory_that_cannot_be_entered_is_told_apart_from_a_missing_git() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    let said = run(&file, &["status"]).unwrap_err();
    assert!(said.starts_with("git status: "), "{said}");
    let gone = run(&dir.path().join("gone"), &["status"]).unwrap_err();
    assert!(gone.contains("not found"), "{gone}");
}
