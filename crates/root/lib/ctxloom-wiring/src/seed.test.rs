//! The seed the acceptance tests plant sessions with, and the sessions use
//! cases it plants them for: what it writes is what the commands then read.

use std::future::Future;
use std::time::{Duration, SystemTime};

use sessions_application::{
    BindRequest, ImportRequest, ImportService, ListQuery, SearchQuery, SessionService,
    TranscriptQuery,
};

use crate::commands::tests::World;
use crate::fixtures::fs;
use crate::{Seed, TestError};

const HARP: &str = "jolly-mute-magma";

fn block<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

fn seed() -> Seed {
    Seed {
        harp: HARP.to_string(),
        started_ago: 7200,
        ended_ago: Some(3600),
        lock_free: true,
        transcript: true,
        native: Some(("native-1".to_string(), "/t/native-1.jsonl".to_string())),
        other_files: vec!["notes/plan.md".to_string()],
        ..Seed::default()
    }
}

fn ago(secs: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(secs)
}

fn near(at: SystemTime, expected: SystemTime) {
    let off = at.duration_since(expected).unwrap_or_else(|e| e.duration());
    assert!(
        off < Duration::from_secs(120),
        "{at:?} is not near {expected:?}"
    );
}

// The id the seed answers is the stable id of the session it minted, which
// the listing then holds, started when the seed said.
#[test]
fn a_seeded_session_is_listed_under_the_id_the_seed_answered() {
    let world = World::new("seed-list");
    let wiring = world.wiring();
    let id = wiring.seed_session(&seed()).unwrap();
    assert!(uuid::Uuid::parse_str(&id).is_ok(), "{id}");
    let mut sessions = wiring.sessions();
    let views = block(sessions.list(ListQuery::default())).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].record.id, id);
    assert_eq!(views[0].record.harp, HARP);
    let started = SystemTime::UNIX_EPOCH
        + Duration::from_secs(u64::try_from(views[0].record.started_at.seconds).unwrap());
    near(started, ago(7200));
}

#[test]
fn a_session_is_searched_by_the_words_of_its_listing() {
    let world = World::new("seed-search");
    let wiring = world.wiring();
    wiring.seed_session(&seed()).unwrap();
    let mut sessions = wiring.sessions();
    let search = |words: &[&str]| SearchQuery {
        list: ListQuery::default(),
        words: words.iter().map(ToString::to_string).collect(),
    };
    let found = block(sessions.search(search(&["jolly"]))).unwrap();
    assert_eq!(found.len(), 1);
    assert!(block(sessions.search(search(&["nosuchword"])))
        .unwrap()
        .is_empty());
}

#[test]
fn the_transcripts_listed_and_read_are_the_one_the_seed_captured() {
    let world = World::new("seed-transcripts");
    let wiring = world.wiring();
    wiring.seed_session(&seed()).unwrap();
    let mut sessions = wiring.sessions();
    let rows = block(sessions.transcripts(TranscriptQuery::default())).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].harp, HARP);
    assert!(rows[0].captured && rows[0].bytes > 0, "{rows:?}");
    let text = block(sessions.transcript(HARP)).unwrap();
    assert!(
        text.contains("Seeded prompt for jolly-mute-magma."),
        "{text}"
    );
}

// The seed writes beside the session what Go's fixture writes: the
// transcript, the authored files and the lock, each backdated to the end.
#[test]
fn the_seed_plants_the_sessions_files_backdated_to_its_end() {
    let world = World::new("seed-files");
    world.wiring().seed_session(&seed()).unwrap();
    let sessions = world.home().join(".ctxloom/sessions");
    let dir = sessions.join(HARP);
    let transcript = dir.join("transcripts/transcript.jsonl");
    let text = fs().read_to_string(&transcript).unwrap();
    assert!(text.contains(r#""harp":"jolly-mute-magma""#), "{text}");
    assert_eq!(
        fs().read_to_string(&dir.join("notes/plan.md")).unwrap(),
        "authored\n"
    );
    let lock = sessions.join(format!("{HARP}.lock"));
    for path in [transcript, dir.join("notes/plan.md"), lock] {
        near(fs().metadata(&path).unwrap().modified, ago(3600));
    }
}

#[test]
fn a_session_that_never_ended_keeps_its_files_as_they_were_written() {
    let world = World::new("seed-live");
    let live = Seed {
        ended_ago: None,
        ..seed()
    };
    world.wiring().seed_session(&live).unwrap();
    let transcript = world
        .home()
        .join(".ctxloom/sessions")
        .join(HARP)
        .join("transcripts/transcript.jsonl");
    near(
        fs().metadata(&transcript).unwrap().modified,
        SystemTime::now(),
    );
}

#[test]
fn a_name_already_claimed_is_refused_as_a_seed_error() {
    let world = World::new("seed-claimed");
    let wiring = world.wiring();
    wiring.seed_session(&seed()).unwrap();
    match wiring.seed_session(&seed()) {
        Err(TestError::Seed { detail }) => assert!(!detail.is_empty()),
        other => panic!("{other:?}"),
    }
}

// The bind hook records the engine's session and transcript against the
// harp: the session then shows them.
#[test]
fn the_bind_hook_binds_the_session_it_names() {
    use ctxloom_cli::Extension;
    let mut world = World::new("seed-bind");
    let wiring = world.wiring();
    let unbound = Seed {
        native: None,
        ..seed()
    };
    wiring.seed_session(&unbound).unwrap();
    world.set("CTXLOOM_SESSION_HARP", HARP);
    let payload = r#"{"session_id":"native-9","transcript_path":"/t/native-9.jsonl"}"#;
    let mut commands = world.commands();
    let program = clap::Command::new("ctxloom").subcommands(commands.nouns());
    let matches = program
        .try_get_matches_from(["ctxloom", "hook", "session-bind", "--engine", "claude-code"])
        .unwrap();
    let call = ctxloom_cli::Call::with_globals(
        &matches,
        Some(angzarr_presentation::Format::Text),
        angzarr_presentation::Format::Text,
        library_application::Flags::default(),
    );
    let catalog = angzarr_presentation::Catalog::en_us(&[crate::EN_US]).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    commands
        .hook(payload, &call, &catalog, &mut out, &mut err)
        .ok()
        .unwrap();
    let mut sessions = world.wiring().sessions();
    let views = block(sessions.list(ListQuery::default())).unwrap();
    assert_eq!(views[0].record.native_id, "native-9");
    assert_eq!(views[0].record.transcript_path, "/t/native-9.jsonl");
    // And the service itself binds: a repeat of the same binding records nothing.
    let again = block(sessions.bind(BindRequest {
        project: None,
        harp: HARP.to_string(),
        native_id: "native-9".to_string(),
        transcript_path: "/t/native-9.jsonl".to_string(),
    }))
    .unwrap();
    assert!(
        matches!(again, sessions_application::Bound::Unchanged),
        "{again:?}"
    );
}

// A Go session of the project's work root is imported; the sessions the
// import found are the ones its report names.
#[test]
fn a_go_session_of_the_project_is_imported() {
    let world = World::new("seed-import");
    let wiring = world.wiring();
    // Seeding registers the project of the working directory.
    wiring.seed_session(&seed()).unwrap();
    let go = world.home().join(".ctxloom/sessions/amber-swift-owl");
    crate::fixtures::mkdirs(&go);
    let work = crate::fixtures::canonical(world.work());
    crate::fixtures::put_mode(
        go.join("session.yaml"),
        &format!(
            "schema_version: 3\nsession_id: 9a1c0d1e-3333-4333-8333-000000000003\nbackend: claude-code\nproject_dir: {}\noutput_dir: /out\nstarted_at: 2026-06-01T12:00:00Z\nended_at: 2026-06-01T13:00:00Z\ntranscript_path: /t.jsonl\n",
            work.display()
        ),
        0o644,
    );
    let mut sessions = wiring.sessions();
    let report = block(sessions.import(ImportRequest {})).unwrap();
    let harps: Vec<&str> = report.imported.iter().map(|i| i.harp.as_str()).collect();
    assert_eq!(harps, ["amber-swift-owl"], "{report:?}");
}
