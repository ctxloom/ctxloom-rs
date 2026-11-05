//! The `session` noun as mounted.

use super::*;
use clap::FromArgMatches;

#[test]
fn test_the_session_noun_is_listed_and_its_bare_form_takes_no_verb() {
    let noun = noun();
    assert_eq!(noun.get_name(), "session");
    assert!(!noun.is_hide_set());
    assert!(noun.get_subcommands().any(|v| v.get_name() == "list"));
    assert!(noun
        .get_about()
        .is_some_and(|about| about.to_string().contains("recorded sessions")));
    assert!(noun
        .get_after_help()
        .is_some_and(|text| text.to_string().contains("ctxloom session list")));
    let matches = noun.get_matches_from(["session"]);
    assert!(Session::from_arg_matches(&matches).is_ok_and(|s| s.verb.is_none()));
}
