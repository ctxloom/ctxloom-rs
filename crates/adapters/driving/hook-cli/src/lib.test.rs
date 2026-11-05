//! The `hook` noun as mounted.

use super::*;

#[test]
fn test_the_hook_noun_is_hidden_and_has_the_engine_callbacks() {
    let noun = noun();
    assert_eq!(noun.get_name(), "hook");
    assert!(noun.is_hide_set());
    let verbs: Vec<&str> = noun.get_subcommands().map(Command::get_name).collect();
    assert_eq!(
        verbs,
        ["session-bind", "hud", "session-start", "skill-mates"]
    );
    assert!(noun
        .get_about()
        .is_some_and(|about| about.to_string().contains("not for people")));
}
