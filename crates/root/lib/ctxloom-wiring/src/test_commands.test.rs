//! The hidden `ctxloom test ...` commands, run as the extension's noun: the
//! exit ladder, the sample list in both formats, the envelopes, the id mints,
//! the store open and the seed.

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{Catalog, Format};
use clap::Command;
use ctxloom_cli::{Call, Extension};
use sessions_application::{ListQuery, SessionService};

use super::*;
use crate::commands::tests::{run_line, try_line, World};

fn block<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn failure(world: &World, line: &[&str]) -> (u8, String) {
    let failed = try_line(&mut world.commands(), Format::Json, line).unwrap_err();
    (failed.exit, failed.cause.code().to_string())
}

#[test]
fn the_noun_is_hidden_and_has_a_verb_for_each_test_command() {
    let world = World::new("test-noun");
    let nouns = world.commands().nouns();
    let noun = nouns.iter().find(|n| n.get_name() == NOUN).unwrap();
    assert!(noun.is_hide_set());
    assert!(noun.is_disable_help_subcommand_set());
    let verbs: Vec<&str> = noun.get_subcommands().map(Command::get_name).collect();
    assert_eq!(
        verbs,
        [
            "refuse",
            "findings",
            "sample",
            "diagnostics",
            "session-id",
            "run-id",
            "open-store",
            "seed-session"
        ]
    );
}

#[test]
fn refuse_is_a_refusal_with_a_remedy_and_findings_are_fatal() {
    let world = World::new("test-ladder");
    assert_eq!(
        failure(&world, &["test", "refuse"]),
        (2, "ctxloom-wiring-test-refused".to_string())
    );
    assert_eq!(
        failure(&world, &["test", "findings"]),
        (3, "ctxloom-wiring-test-findings".to_string())
    );
}

#[test]
fn the_sample_list_is_two_rows_or_none_in_json_and_in_text() {
    let world = World::new("test-sample");
    let json = |line: &[&str]| run_line(&mut world.commands(), line);
    // Whatever the layout, the document is these rows.
    let compact = |text: String| text.split_whitespace().collect::<String>();
    let (code, out, err) = json(&["test", "sample"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        compact(out),
        "[{\"name\":\"alpha\",\"count\":1},{\"name\":\"beta\",\"count\":2}]"
    );
    assert_eq!(compact(json(&["test", "sample", "--empty"]).1), "[]");
    let text = |line: &[&str]| {
        try_line(&mut world.commands(), Format::Text, line)
            .unwrap()
            .1
    };
    assert_eq!(text(&["test", "sample"]), "alpha  1\nbeta  2\n");
    assert_eq!(text(&["test", "sample", "--empty"]), "(none)\n");
}

#[test]
fn an_answer_that_cannot_be_written_is_a_failure_with_exit_1() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let program = Command::new("ctxloom").subcommand(noun());
    let matches = program
        .try_get_matches_from(["ctxloom", "test", "session-id"])
        .unwrap();
    let call = Call::with_globals(
        &matches,
        Some(Format::Json),
        Format::Json,
        library_application::Flags::default(),
    );
    let catalog = Catalog::en_us(&[crate::EN_US, ctxloom_cli::EN_US]).unwrap();
    let failed = run(
        || unreachable!("a session id needs no home"),
        Vec::new,
        &call,
        &catalog,
        &mut Broken,
    )
    .unwrap_err();
    assert_eq!(
        (failed.exit, failed.cause.code()),
        (1, "ctxloom-cli-output")
    );
}

fn extra() -> Vec<Box<dyn Diagnostic>> {
    vec![Box::new(LibError::NoHome {
        variable: "EXTRA".to_string(),
    })]
}

#[test]
fn diagnostics_are_the_envelopes_of_the_lines_wiring_and_the_roots_samples() {
    let world = World::new("test-diagnostics");
    let mut commands = world.commands().with_diagnostic_samples(extra);
    let (code, out, _) = run_line(&mut commands, &["test", "diagnostics"]);
    assert_eq!(code, 0);
    assert!(out.starts_with('[') && out.ends_with("]\n"), "{out}");
    for code in [
        "ctxloom-cli-usage",
        "ctxloom-wiring-seed",
        "ctxloom-wiring-test-findings",
    ] {
        assert!(
            out.contains(&format!("\"code\": \"{code}\"")),
            "{code}: {out}"
        );
    }
    assert!(out.contains("EXTRA"), "the root's sample: {out}");
    assert!(
        out.contains("\"message\": \"there is no home directory"),
        "rendered from the catalog: {out}"
    );
}

#[test]
fn a_session_id_is_a_minted_harp_on_a_line() {
    let world = World::new("test-session-id");
    let (code, out, _) = run_line(&mut world.commands(), &["test", "session-id"]);
    assert_eq!(code, 0);
    let line = out.strip_suffix('\n').unwrap();
    assert_eq!(line.split('-').count(), 3, "{line}");
}

#[test]
fn a_run_id_is_the_runs_uuid_and_a_bad_session_is_refused() {
    let world = World::new("test-run-id");
    let session = "0b0b0b0b-0b0b-4b0b-8b0b-0b0b0b0b0b0b";
    let (_, out, _) = run_line(&mut world.commands(), &["test", "run-id", session, "3"]);
    assert_eq!(out, format!("{}\n", crate::run_id(session, 3).unwrap()));
    assert_ne!(out, format!("{}\n", crate::run_id(session, 4).unwrap()));
    assert_eq!(
        failure(&world, &["test", "run-id", "not-a-uuid", "1"]),
        (1, "ctxloom-wiring-bad-session".to_string())
    );
}

#[test]
fn open_store_prints_the_path_of_the_projects_database() {
    let world = World::new("test-open-store");
    let (_, out, _) = run_line(&mut world.commands(), &["test", "open-store", "proj-1"]);
    let path = world.wiring().open_store("proj-1").unwrap();
    assert_eq!(out, format!("{}\n", path.display()));
    assert!(path.starts_with(world.home()), "{path:?}");
    assert_eq!(
        failure(&world, &["test", "open-store", "../escape"]),
        (1, "ctxloom-wiring-store".to_string())
    );
}

#[test]
fn seed_session_plants_a_session_and_prints_its_id() {
    let world = World::new("test-seed-session");
    let (_, out, _) = run_line(
        &mut world.commands(),
        &["test", "seed-session", "jolly-mute-magma", "--transcript"],
    );
    let id = out.strip_suffix('\n').unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok(), "{id}");
    let mut sessions = world.wiring().sessions();
    let views = block(sessions.list(ListQuery::default())).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].record.id, id);
    assert_eq!(views[0].record.harp, "jolly-mute-magma");
}

#[test]
fn a_seed_that_cannot_plant_ends_in_a_failure() {
    let world = World::new("test-seed-fails");
    // The same name twice: the second claim is refused.
    let line = ["test", "seed-session", "jolly-mute-magma"];
    run_line(&mut world.commands(), &line);
    assert_eq!(
        failure(&world, &line),
        (1, "ctxloom-wiring-seed".to_string())
    );
}

fn seed_of(line: &[&str]) -> Seed {
    let program = Command::new("ctxloom").subcommand(noun());
    let argv = ["ctxloom", "test", "seed-session"];
    let matches = program
        .try_get_matches_from(argv.iter().chain(line.iter()))
        .unwrap();
    let matches = matches.subcommand_matches(NOUN).unwrap();
    match TestVerb::from_arg_matches(matches).unwrap() {
        TestVerb::SeedSession(args) => args.into(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_seed_flag_lands_in_the_seed() {
    let seed = seed_of(&[
        "harp-one",
        "--project",
        "other",
        "--started-ago",
        "5",
        "--ended-ago",
        "3",
        "--lock",
        "free",
        "--transcript",
        "--keep",
        "--oneshot",
        "--native-id",
        "n1",
        "--native-transcript",
        "/t/n1.jsonl",
        "--other-file",
        "a.md",
        "--other-file",
        "b.md",
    ]);
    assert_eq!(
        format!("{seed:?}"),
        format!(
            "{:?}",
            Seed {
                harp: "harp-one".to_string(),
                project: Some("other".to_string()),
                started_ago: 5,
                ended_ago: Some(3),
                lock_free: true,
                transcript: true,
                keep: true,
                oneshot: true,
                native: Some(("n1".to_string(), "/t/n1.jsonl".to_string())),
                other_files: vec!["a.md".to_string(), "b.md".to_string()],
            }
        )
    );
}

#[test]
fn a_seed_with_no_flags_is_a_day_old_unfinished_unlocked_session() {
    let seed = seed_of(&["harp-one"]);
    assert_eq!(
        format!("{seed:?}"),
        format!(
            "{:?}",
            Seed {
                harp: "harp-one".to_string(),
                started_ago: 86_400,
                ..Seed::default()
            }
        )
    );
}

#[test]
fn a_half_named_native_binding_keeps_the_half_it_has() {
    assert_eq!(
        seed_of(&["h", "--native-id", "n1"]).native,
        Some(("n1".to_string(), String::new()))
    );
    assert_eq!(
        seed_of(&["h", "--native-transcript", "/t"]).native,
        Some((String::new(), "/t".to_string()))
    );
    assert!(!seed_of(&["h", "--lock", "none"]).lock_free);
}
