//! The mounted `session` and `hook` nouns.

use std::sync::Arc;

use ::process_env::EnvSnapshot;
use angzarr_presentation::{Catalog, Format};
use clap::Command;
use ctxloom_cli::{Call, Extension};

use crate::commands::tests::World;
use crate::Commands;

fn commands() -> Commands {
    Commands::new(Arc::new(EnvSnapshot::from_process()))
}

/// Run `line` as the command line would: the nouns parsed, then handed over.
fn run(
    commands: &mut Commands,
    line: &[&str],
) -> Result<(u8, String, String), ctxloom_cli::Failure> {
    run_with(commands, "", line)
}

/// As [`run`], with `input` as the hook's payload (stdin, in the binary).
fn run_with(
    commands: &mut Commands,
    input: &str,
    line: &[&str],
) -> Result<(u8, String, String), ctxloom_cli::Failure> {
    let program = Command::new("ctxloom")
        .subcommand_required(true)
        .subcommands(commands.nouns());
    let matches = program
        .try_get_matches_from(std::iter::once("ctxloom").chain(line.iter().copied()))
        .unwrap();
    let noun = matches.subcommand_name().unwrap().to_string();
    let call = Call::with_globals(
        &matches,
        Some(Format::Text),
        Format::Text,
        library_application::Flags::default(),
    );
    let catalog = Catalog::en_us(&[crate::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = if noun == "hook" {
        commands.hook(input, &call, &catalog, &mut out, &mut err)?
    } else {
        commands.run(&noun, &call, &catalog, &mut out, &mut err)?
    };
    Ok((
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    ))
}

#[test]
fn test_the_nouns_include_session_and_a_hidden_hook() {
    let nouns = commands().nouns();
    let find = |name: &str| nouns.iter().find(|n| n.get_name() == name).expect(name);
    assert!(!find("session").is_hide_set());
    assert!(find("hook").is_hide_set());
    assert_eq!(
        nouns.iter().filter(|n| n.get_name() == "session").count(),
        1
    );
}

#[test]
fn test_a_bare_session_is_its_default_verb_not_its_usage() {
    let noun = commands()
        .nouns()
        .into_iter()
        .find(|n| n.get_name() == "session")
        .unwrap();
    assert!(!noun.is_arg_required_else_help_set());
    assert!(noun.is_disable_help_subcommand_set());
    let hook = commands()
        .nouns()
        .into_iter()
        .find(|n| n.get_name() == "hook")
        .unwrap();
    assert!(hook.is_arg_required_else_help_set());
}

#[test]
fn test_an_unknown_session_verb_is_refused_by_the_parser_with_a_suggestion() {
    let program = Command::new("ctxloom").subcommands(commands().nouns());
    let refused = program
        .try_get_matches_from(["ctxloom", "session", "lisst"])
        .unwrap_err();
    assert_eq!(refused.kind(), clap::error::ErrorKind::InvalidSubcommand);
    let hook = Command::new("ctxloom")
        .subcommands(commands().nouns())
        .try_get_matches_from(["ctxloom", "hook", "nosuch"])
        .unwrap_err();
    assert_eq!(hook.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn test_the_hud_hook_writes_its_statusline_to_stdout_only() {
    // The payload is read from stdin, which is not a document here: the
    // statusline hook answers with the bare name.
    let (code, out, err) = run(&mut commands(), &["hook", "hud"]).ok().unwrap();
    assert_eq!(code, 0);
    assert!(out.contains("ctxloom"), "{out}");
    assert!(err.is_empty());
}

#[test]
fn test_each_session_hook_answers_through_the_engines_codec() {
    for verb in ["session-start", "skill-mates"] {
        let line = ["hook", verb, "--engine", "claude-code"];
        let (code, out, _) = run_with(&mut commands(), r#"{"skill":"x"}"#, &line)
            .ok()
            .unwrap();
        assert_eq!((code, out.as_str()), (0, "{}\n"), "{verb}");
    }
}

#[test]
fn test_a_hook_for_an_engine_nobody_knows_fails_with_exit_1() {
    let failure = run(
        &mut commands(),
        &["hook", "session-start", "--engine", "nosuch"],
    )
    .err()
    .unwrap();
    assert_eq!(failure.exit, 1);
}

// `session list` runs on the sessions use cases over the world's home: no
// session is recorded there, so it answers an empty list and exits 0.
#[test]
fn test_a_session_command_is_run_on_the_sessions_use_cases() {
    let world = World::new("session-list");
    let (code, out, err) = run(&mut world.commands(), &["session", "list"])
        .ok()
        .unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(err.is_empty(), "{err}");
    assert_eq!(out, "(no sessions)\n");
}

// The hook that binds a session answers the engine with the harp the process
// names (`CTXLOOM_SESSION_HARP`) on stdout, and nothing on stderr.
#[test]
fn test_the_bind_hook_answers_with_the_harp_the_environment_names() {
    let mut world = World::new("session-bind");
    world.set("CTXLOOM_SESSION_HARP", "jolly-mute-magma");
    let payload = r#"{"session_id":"s1","transcript_path":"/tmp/t.jsonl"}"#;
    let (code, out, err) = run_with(
        &mut world.commands(),
        payload,
        &["hook", "session-bind", "--engine", "claude-code"],
    )
    .ok()
    .unwrap();
    assert_eq!(code, 0);
    assert!(out.contains("jolly-mute-magma"), "{out}");
    assert!(err.is_empty(), "{err}");
}

// The statusline hook shows the harp the environment names.
#[test]
fn test_the_hud_hook_shows_the_harp_the_environment_names() {
    let mut world = World::new("hud-harp");
    world.set("CTXLOOM_SESSION_HARP", "jolly-mute-magma");
    let (_, out, _) = run_with(
        &mut world.commands(),
        r#"{"model":{"display_name":"M"}}"#,
        &["hook", "hud"],
    )
    .ok()
    .unwrap();
    assert!(out.contains("jolly-mute-magma"), "{out}");
}
