//! The setup commands' shape: nouns, flags and arguments.

use clap::Command;

use super::*;

fn parse(args: &[&str]) -> clap::ArgMatches {
    let mut line = vec!["ctxloom"];
    line.extend(args);
    nouns()
        .into_iter()
        .fold(Command::new("ctxloom"), Command::subcommand)
        .try_get_matches_from(line)
        .expect("parses")
}

#[test]
fn the_nouns_are_auth_clean_doctor_and_init() {
    let nouns = nouns();
    let names: Vec<&str> = nouns.iter().map(Command::get_name).collect();
    assert_eq!(names, vec!["auth", "clean", "doctor", "init"]);
    for noun in &nouns {
        assert!(noun.get_about().is_some(), "{} has help", noun.get_name());
    }
}

#[test]
fn clean_takes_yes_long_or_short() {
    for form in ["--yes", "-y"] {
        let m = parse(&["clean", form]);
        assert!(m.subcommand_matches("clean").unwrap().get_flag("yes"));
    }
    assert!(!parse(&["clean"])
        .subcommand_matches("clean")
        .unwrap()
        .get_flag("yes"));
}

#[test]
fn doctor_takes_all_deps_and_fix_as_switches() {
    let none = parse(&["doctor"]);
    let none = none.subcommand_matches("doctor").unwrap();
    assert!(!none.get_flag("all") && !none.get_flag("deps") && !none.get_flag("fix"));
    let m = parse(&["doctor", "--all", "--deps", "--fix"]);
    let m = m.subcommand_matches("doctor").unwrap();
    assert!(m.get_flag("all") && m.get_flag("deps") && m.get_flag("fix"));
    let m = parse(&["doctor", "--fix"]);
    let m = m.subcommand_matches("doctor").unwrap();
    assert!(m.get_flag("fix") && !m.get_flag("all"));
}

#[test]
fn init_takes_repeatable_remotes_a_forge_and_a_prompt() {
    let m = parse(&[
        "init", "--remote", "a/b", "--remote", "c/d", "--forge", "git",
    ]);
    let init = m.subcommand_matches("init").unwrap();
    let remotes: Vec<&String> = init.get_many::<String>("remote").unwrap().collect();
    assert_eq!(remotes, ["a/b", "c/d"]);
    assert_eq!(
        init.get_one::<String>("forge").map(String::as_str),
        Some("git")
    );
    let m = parse(&["init", "prompt"]);
    assert!(m
        .subcommand_matches("init")
        .unwrap()
        .subcommand_matches("prompt")
        .is_some());
}

// auth.feature: "There is no command that stores a credential": the only verb
// of `auth` reports, and nothing else parses.
#[test]
fn auth_has_status_and_nothing_that_takes_a_credential() {
    let auth = nouns()
        .into_iter()
        .find(|n| n.get_name() == "auth")
        .unwrap();
    let verbs: Vec<&str> = auth.get_subcommands().map(Command::get_name).collect();
    assert_eq!(verbs, ["status"]);
    assert!(auth.get_arguments().all(|a| a.get_id() == "help"));
    for line in [
        vec!["auth", "set", "--engine", "claude-code", "--mode", "token"],
        vec!["auth", "login"],
        vec!["auth", "status", "--token", "x"],
    ] {
        let mut full = vec!["ctxloom"];
        full.extend(line.clone());
        let parsed = nouns()
            .into_iter()
            .fold(Command::new("ctxloom"), Command::subcommand)
            .try_get_matches_from(full);
        assert!(parsed.is_err(), "{line:?} parsed");
    }
}

#[test]
fn auth_status_and_a_bare_auth_parse() {
    assert!(parse(&["auth", "status"])
        .subcommand_matches("auth")
        .is_some());
    assert!(parse(&["auth"]).subcommand_matches("auth").is_some());
}
