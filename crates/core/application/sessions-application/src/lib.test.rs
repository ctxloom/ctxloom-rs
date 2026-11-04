//! The sessions use cases, over fakes of the ports and angzarr-embedded's
//! kernel on an in-memory store. Each test names the Go scenario or test it
//! ports (session.feature, j001200, j001300, `operations`/`cli` tests,
//! release/0.7), or says it is design.

use angzarr_diagnostic::Diagnostic;
use session_ports::LockState;

use crate::fakes::{
    bound, claim, ended, holder, id_text, minted, project, ready, FakeClock, FakeDelay, FakeFiles,
    FakeIdentity, FakeLiveness, FakeStores,
};
use crate::{
    AppError, BindRequest, Bound, Class, KeepRequest, ListQuery, Refusal, RemoveRequest,
    RenameRequest, SearchQuery, Service, SessionService, ShowQuery, State, TranscriptQuery,
};

type Svc = Service<FakeIdentity, FakeStores, FakeFiles, FakeLiveness, FakeClock, FakeDelay>;

/// The fake clock reads from second 1_700_000_001: this is "now" (roughly) to
/// a test of retention.
const NOW: i64 = 1_700_000_000;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;
const HERE: &str = "jolly-mute-magma";
const ELSEWHERE: &str = "brisk-tall-oak";

fn service() -> Svc {
    Service::new(
        FakeIdentity {
            bound: Some(project(HERE)),
            every: vec![project(ELSEWHERE), project(HERE)],
            fails: None,
        },
        FakeStores::default(),
        FakeFiles::default(),
        FakeLiveness::default(),
        FakeClock::default(),
        FakeDelay::default(),
    )
}

/// The canonical transcript Go's fixture records: one user prompt.
fn transcript_of(harp: &str) -> String {
    format!(
        r#"{{"v":1,"harp":"{harp}","engine":"claude-code","seq":0,"ts":"2026-01-01T00:00:00Z","kind":"entry","entry":{{"type":"user","content":"Seeded prompt for {harp}."}}}}
"#
    )
}

impl Svc {
    /// A session of the project `project`, minted at `started`, with its
    /// transcript captured and its lock `lock` (Go's fixtures: "a finished
    /// session ... with a transcript").
    fn seed(&mut self, project: &str, id: u8, harp: &str, started: i64, lock: Option<LockState>) {
        let store = self.stores.stores.entry(project.to_string()).or_default();
        minted(store, id, harp, "/work/proj", started);
        self.files
            .transcripts
            .insert(harp.to_string(), transcript_of(harp));
        if let Some(lock) = lock {
            self.liveness.0.insert(harp.to_string(), lock);
        }
    }

    fn store(&mut self, project: &str) -> &angzarr_embedded::angzarr_ports::memory::MemoryStore {
        self.stores.stores.get(project).expect("a store")
    }

    fn harps(&mut self, query: ListQuery) -> Vec<String> {
        ready(self.list(query))
            .unwrap()
            .into_iter()
            .map(|v| v.record.harp)
            .collect()
    }
}

fn all() -> ListQuery {
    ListQuery {
        all: true,
        ..ListQuery::default()
    }
}

fn state(state: State) -> ListQuery {
    ListQuery {
        all: true,
        state: Some(state),
        ..ListQuery::default()
    }
}

// ------------------------------------------------------------------ reading

// session.feature: "The bare noun lists what has been recorded".
#[test]
fn a_listing_holds_the_sessions_of_the_current_project() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 2 * DAY, None);
    svc.seed(HERE, 2, "brisk-copper-moth", NOW - DAY, None);
    svc.seed(ELSEWHERE, 3, "calm-dusty-wren", NOW - DAY, None);
    assert_eq!(
        svc.harps(ListQuery::default()),
        ["brisk-copper-moth", "amber-swift-owl"],
        "this project's, most recently started first"
    );
}

// session.feature: "--all reaches past the current project".
#[test]
fn all_reaches_past_the_current_project() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 2 * DAY, None);
    svc.seed(ELSEWHERE, 3, "calm-dusty-wren", NOW - DAY, None);
    assert_eq!(svc.harps(all()), ["calm-dusty-wren", "amber-swift-owl"]);
}

// session.feature: "A fresh project has recorded nothing": an empty list, not
// an error and not nothing.
#[test]
fn a_fresh_project_has_no_sessions_and_a_read_creates_no_store() {
    let mut svc = service();
    assert_eq!(svc.harps(ListQuery::default()), Vec::<String>::new());
    assert_eq!(svc.harps(all()), Vec::<String>::new());
    assert!(
        svc.stores.opened.is_empty(),
        "reading the record touches nothing"
    );
}

// Design: a directory in no project has no sessions to list.
#[test]
fn a_directory_in_no_project_lists_nothing() {
    let mut svc = service();
    svc.identity.bound = None;
    assert_eq!(svc.harps(ListQuery::default()), Vec::<String>::new());
}

// session.feature: "The listing takes each session's state from its lock, not
// its end stamp": held is live, free is ended, no lock file is unknown, and a
// live session's end is blank rather than a stamp from its previous run.
#[test]
fn the_state_is_the_locks_and_a_live_session_has_no_end() {
    let mut svc = service();
    svc.seed(
        HERE,
        1,
        "amber-swift-owl",
        NOW - 3 * DAY,
        Some(LockState::Held),
    );
    svc.seed(
        HERE,
        2,
        "brisk-copper-moth",
        NOW - 2 * DAY,
        Some(LockState::Free),
    );
    svc.seed(HERE, 3, "calm-dusty-wren", NOW - DAY, None);
    for (id, harp) in [
        (1, "amber-swift-owl"),
        (2, "brisk-copper-moth"),
        (3, "calm-dusty-wren"),
    ] {
        let _ = harp;
        ended(svc.store(HERE), id, NOW - HOUR, NOW - 2 * HOUR);
    }
    let views = ready(svc.list(all())).unwrap();
    let by = |harp: &str| {
        views
            .iter()
            .find(|v| v.record.harp == harp)
            .unwrap()
            .clone()
    };
    assert_eq!(by("amber-swift-owl").state, State::Live);
    assert_eq!(
        by("amber-swift-owl").end(),
        None,
        "the record's stamp is the LAST run's"
    );
    assert_eq!(by("brisk-copper-moth").state, State::Ended);
    assert!(by("brisk-copper-moth").end().is_some());
    assert_eq!(by("calm-dusty-wren").state, State::Unknown);
}

// session.feature: "--state keeps only the sessions in that state" (list).
#[test]
fn state_keeps_only_the_sessions_in_that_state() {
    let mut svc = service();
    svc.seed(
        HERE,
        1,
        "amber-swift-owl",
        NOW - 3 * DAY,
        Some(LockState::Held),
    );
    svc.seed(
        HERE,
        2,
        "brisk-copper-moth",
        NOW - 2 * DAY,
        Some(LockState::Free),
    );
    svc.seed(HERE, 3, "calm-dusty-wren", NOW - DAY, None);
    assert_eq!(svc.harps(state(State::Live)), ["amber-swift-owl"]);
    assert_eq!(svc.harps(state(State::Ended)), ["brisk-copper-moth"]);
    assert_eq!(svc.harps(state(State::Unknown)), ["calm-dusty-wren"]);
}

// session.feature: "Showing a session prints its record, titled by its first
// prompt": the listing carries the title.
#[test]
fn a_session_is_titled_by_the_first_prompt_of_its_transcript() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    let views = ready(svc.list(all())).unwrap();
    assert_eq!(
        views[0].title.as_deref(),
        Some("Seeded prompt for amber-swift-owl.")
    );
    assert_eq!(views[0].summary(), "Seeded prompt for amber-swift-owl.");
    let captured = views[0].captured.as_ref().expect("a transcript");
    assert!(captured.path.ends_with("transcripts/transcript.jsonl"));
}

// j001200: a session with no captured transcript has no title and no
// transcript, and is still listed.
#[test]
fn a_session_with_no_transcript_is_listed_with_no_title() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    svc.files.transcripts.clear();
    let views = ready(svc.list(all())).unwrap();
    assert_eq!(views[0].title, None);
    assert_eq!(views[0].captured, None);
    assert_eq!(views[0].summary(), "(no summary)");
}

// j001200: "A phrase from the question she asked finds the session that
// answered it", beside an unrelated session the search could have returned.
#[test]
fn a_search_finds_the_session_whose_title_has_the_words() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-quiet-heron", NOW - 3 * DAY, None);
    svc.seed(HERE, 2, "brisk-copper-moth", NOW - 2 * DAY, None);
    svc.files.transcripts.insert(
        "amber-quiet-heron".to_string(),
        transcript_of("amber-quiet-heron").replace(
            "Seeded prompt for amber-quiet-heron.",
            "J001200-TITLE-WORKTREE-QUESTION why this layout",
        ),
    );
    let found = ready(svc.search(SearchQuery {
        list: all(),
        words: vec!["J001200-TITLE-WORKTREE-QUESTION".to_string()],
    }))
    .unwrap();
    let harps: Vec<&str> = found.iter().map(|v| v.record.harp.as_str()).collect();
    assert_eq!(harps, ["amber-quiet-heron"]);
}

// j001200: "A query that matches nothing says so": an empty result, which the
// edge renders; the use case does not fail.
#[test]
fn a_search_that_matches_nothing_is_an_empty_result() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-quiet-heron", NOW - 3 * DAY, None);
    let found = ready(svc.search(SearchQuery {
        list: all(),
        words: vec!["J001200-MATCHES-ABSOLUTELY-NOTHING".to_string()],
    }))
    .unwrap();
    assert!(found.is_empty());
}

// session.feature: "--state keeps only the sessions in that state" (search).
#[test]
fn a_search_is_narrowed_by_state_as_a_listing_is() {
    let mut svc = service();
    svc.seed(
        HERE,
        1,
        "amber-swift-owl",
        NOW - 3 * DAY,
        Some(LockState::Held),
    );
    svc.seed(
        HERE,
        2,
        "brisk-copper-moth",
        NOW - 2 * DAY,
        Some(LockState::Free),
    );
    let found = ready(svc.search(SearchQuery {
        list: state(State::Ended),
        words: vec!["seeded".to_string()],
    }))
    .unwrap();
    let harps: Vec<&str> = found.iter().map(|v| v.record.harp.as_str()).collect();
    assert_eq!(harps, ["brisk-copper-moth"]);
}

#[test]
fn show_answers_one_session_with_its_record() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, Some(LockState::Free));
    bound(
        svc.store(HERE),
        1,
        NOW - DAY + 10,
        "native-1",
        "/vendor/t.jsonl",
    );
    let view = ready(svc.show(ShowQuery {
        harp: "amber-swift-owl".to_string(),
        ..ShowQuery::default()
    }))
    .unwrap();
    assert_eq!(view.record.engine, "claude-code");
    assert_eq!(view.record.native_id, "native-1");
    assert_eq!(view.state, State::Ended);
    assert_eq!(
        view.title.as_deref(),
        Some("Seeded prompt for amber-swift-owl.")
    );
}

// Go: GetSession is global: a harp in another project is still found.
#[test]
fn show_finds_a_session_of_another_project() {
    let mut svc = service();
    svc.seed(ELSEWHERE, 3, "calm-dusty-wren", NOW - DAY, None);
    let view = ready(svc.show(ShowQuery {
        harp: "calm-dusty-wren".to_string(),
        ..ShowQuery::default()
    }))
    .unwrap();
    assert_eq!(view.record.harp, "calm-dusty-wren");
}

// session.feature: "Editing a session nobody recorded fails" / "The harp is a
// positional, and an unknown one fails": an unknown harp is an error naming it.
#[test]
fn an_unknown_harp_is_an_error_naming_it() {
    let mut svc = service();
    let error = ready(svc.show(ShowQuery {
        harp: "no-such-harp".to_string(),
        ..ShowQuery::default()
    }))
    .unwrap_err();
    assert_eq!(
        error,
        AppError::NoSession {
            harp: "no-such-harp".to_string()
        }
    );
    assert_eq!(error.args()[0].0, "harp");
}

// ---------------------------------------------------------------- transcripts

// session.feature: "The bare sub-noun lists which sessions have a transcript"
// and "A session with no captured transcript is still named": absence is a row.
#[test]
fn the_transcript_listing_names_every_session_captured_or_not() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 2 * DAY, None);
    svc.seed(HERE, 2, "brisk-copper-moth", NOW - DAY, None);
    svc.files.transcripts.remove("brisk-copper-moth");
    let rows = ready(svc.transcripts(TranscriptQuery {
        all: true,
        ..TranscriptQuery::default()
    }))
    .unwrap();
    let got: Vec<(&str, bool, u64)> = rows
        .iter()
        .map(|r| (r.harp.as_str(), r.captured, r.bytes))
        .collect();
    assert_eq!(
        got,
        [
            ("brisk-copper-moth", false, 0),
            (
                "amber-swift-owl",
                true,
                transcript_of("amber-swift-owl").len() as u64
            )
        ]
    );
    assert_eq!(rows[0].path, None);
    assert!(rows[1]
        .path
        .as_deref()
        .unwrap()
        .ends_with("transcript.jsonl"));
}

#[test]
fn the_transcript_listing_of_one_session_is_that_session() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 2 * DAY, None);
    svc.seed(HERE, 2, "brisk-copper-moth", NOW - DAY, None);
    let rows = ready(svc.transcripts(TranscriptQuery {
        harp: Some("amber-swift-owl".to_string()),
        ..TranscriptQuery::default()
    }))
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].harp, "amber-swift-owl");
}

// session.feature: "The harp is a positional, and an unknown one fails": an
// empty listing would read as "this session has nothing".
#[test]
fn the_transcript_listing_of_an_unknown_session_fails() {
    let mut svc = service();
    let error = ready(svc.transcripts(TranscriptQuery {
        harp: Some("no-such-harp".to_string()),
        ..TranscriptQuery::default()
    }))
    .unwrap_err();
    assert!(matches!(error, AppError::NoSession { .. }));
}

// session.feature: "Show prints the whole transcript and exits".
#[test]
fn a_transcript_is_shown_whole() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    assert_eq!(
        ready(svc.transcript("amber-swift-owl")).unwrap(),
        transcript_of("amber-swift-owl")
    );
}

// Go: errNoCapturedTranscript.
#[test]
fn a_transcript_never_captured_is_an_error_not_an_empty_text() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    svc.files.transcripts.clear();
    assert_eq!(
        ready(svc.transcript("amber-swift-owl")).unwrap_err(),
        AppError::NoCapturedTranscript {
            harp: "amber-swift-owl".to_string()
        }
    );
    assert!(matches!(
        ready(svc.transcript("no-such-harp")).unwrap_err(),
        AppError::NoSession { .. }
    ));
}

// ----------------------------------------------------------------------- keep

fn keep(harp: &str, keep: bool) -> KeepRequest {
    KeepRequest {
        harp: harp.to_string(),
        keep,
        ..KeepRequest::default()
    }
}

// session.feature: "A kept transcript is refused at any age, and the listing
// says it is kept": keep marks it, the listing shows it, --undo releases it.
#[test]
fn keep_marks_the_session_and_undo_releases_it() {
    let mut svc = service();
    svc.seed(
        HERE,
        1,
        "amber-swift-owl",
        NOW - 40 * DAY,
        Some(LockState::Free),
    );
    let kept = ready(svc.keep(keep("amber-swift-owl", true))).unwrap();
    assert!((kept.kept, kept.changed) == (true, true));
    let views = ready(svc.list(all())).unwrap();
    assert!(views[0].record.kept);
    assert!(
        views[0].summary().contains("kept"),
        "the badge rides the row"
    );
    let undone = ready(svc.keep(keep("amber-swift-owl", false))).unwrap();
    assert!((undone.kept, undone.changed) == (false, true));
    assert!(!ready(svc.list(all())).unwrap()[0].record.kept);
}

// Go: SetKeep is idempotent: an unchanged mark records nothing.
#[test]
fn keeping_a_kept_session_changes_nothing() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 40 * DAY, None);
    ready(svc.keep(keep("amber-swift-owl", true))).unwrap();
    let again = ready(svc.keep(keep("amber-swift-owl", true))).unwrap();
    assert!(!again.changed);
    assert!(again.kept);
}

// Go: SetKeep on a harp with no sidecar is ErrNotFound.
#[test]
fn keeping_an_unknown_session_fails_naming_it() {
    let mut svc = service();
    assert_eq!(
        ready(svc.keep(keep("no-such-harp", true))).unwrap_err(),
        AppError::NoSession {
            harp: "no-such-harp".to_string()
        }
    );
}

// ---------------------------------------------------------------------- remove

fn remove(harp: &str, yes: bool) -> RemoveRequest {
    RemoveRequest {
        harp: harp.to_string(),
        yes,
        ..RemoveRequest::default()
    }
}

/// A finished session whose transcript retention has passed (j001300): its
/// lock free, its end 60 days ago, nothing worked on since.
fn finished(svc: &mut Svc, id: u8, harp: &str) {
    svc.seed(HERE, id, harp, NOW - 70 * DAY, Some(LockState::Free));
    ended(svc.store(HERE), id, NOW - 70 * DAY + 10, NOW - 60 * DAY);
    svc.files.last_active.insert(
        harp.to_string(),
        crate::Timestamp {
            seconds: NOW - 60 * DAY,
            nanos: 0,
        },
    );
}

// session.feature: "Delete reports before it removes": the plan, applied
// false, and everything still there.
#[test]
fn a_removal_reports_before_it_removes() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    let removal = ready(svc.remove(remove("amber-swift-owl", false))).unwrap();
    assert!(!removal.applied && !removal.index_entry_removed);
    assert_eq!(removal.refusal, None);
    let destroy: Vec<&str> = removal
        .files
        .destroy
        .iter()
        .map(|i| i.rel.as_str())
        .collect();
    assert_eq!(destroy, ["transcripts/transcript.jsonl"]);
    assert_eq!(removal.files.bytes_freed, 0, "nothing freed on a report");
    assert!(svc.files.destroyed.is_empty());
    assert!(svc.files.transcripts.contains_key("amber-swift-owl"));
    assert_eq!(svc.harps(all()), ["amber-swift-owl"], "still listed");
}

// session.feature: "--yes removes the index entry and the transcript", and
// leaves what is authored, named.
#[test]
fn a_removal_destroys_the_transcript_and_the_index_entry_and_names_what_it_keeps() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    svc.files.others.insert(
        "amber-swift-owl".to_string(),
        vec![("design-notes.md".to_string(), 12)],
    );
    let removal = ready(svc.remove(remove("amber-swift-owl", true))).unwrap();
    assert!(removal.applied && removal.index_entry_removed);
    assert_eq!(removal.refusal, None);
    assert_eq!(
        removal.files.bytes_freed,
        transcript_of("amber-swift-owl").len() as u64
    );
    assert!(!svc.files.transcripts.contains_key("amber-swift-owl"));
    assert_eq!(svc.harps(all()), Vec::<String>::new(), "no longer listed");
    let kept = &removal.files.keep;
    assert_eq!(
        kept.len(),
        1,
        "an authored file is named, never silently kept"
    );
    assert_eq!(kept[0].rel, "design-notes.md");
    assert_eq!(kept[0].class, Class::Authored);
    assert!(kept[0].reason.is_some());
    assert_eq!(
        svc.files.others["amber-swift-owl"].len(),
        1,
        "and left on disk"
    );
}

// session.feature: "The sweep empties a session but does not unlist it"
// (session purge): the conversation goes, the index entry stays.
#[test]
fn a_purge_empties_the_session_and_leaves_it_listed() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    let request = RemoveRequest {
        scope: crate::Scope::Purge,
        ..remove("amber-swift-owl", true)
    };
    let purged = ready(svc.remove(request)).unwrap();
    assert!(purged.applied && !purged.index_entry_removed);
    assert!(!svc.files.transcripts.contains_key("amber-swift-owl"));
    assert_eq!(svc.harps(all()), ["amber-swift-owl"], "still listed");
}

// session.feature: "--yes destroys the transcript and leaves the output dir"
// (session transcript purge): the transcripts alone, never native history.
#[test]
fn a_transcript_purge_takes_the_transcripts_and_names_the_native_history_it_leaves() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    svc.files.others.insert(
        "amber-swift-owl".to_string(),
        vec![("native/history.jsonl".to_string(), 12)],
    );
    let request = RemoveRequest {
        scope: crate::Scope::Transcript,
        ..remove("amber-swift-owl", false)
    };
    let plan = ready(svc.remove(request)).unwrap();
    let destroy: Vec<&str> = plan.files.destroy.iter().map(|i| i.rel.as_str()).collect();
    assert_eq!(destroy, ["transcripts/transcript.jsonl"]);
    assert_eq!(plan.files.keep.len(), 1);
    assert_eq!(plan.files.keep[0].rel, "native/history.jsonl");
    assert!(plan.files.keep[0]
        .reason
        .as_deref()
        .unwrap()
        .contains("not a transcript"));
}

// session.feature: "A session running again is refused by the transcript's own
// destroyer": the refusal names the command that was run.
#[test]
fn the_refusal_names_the_destroyer_that_was_run() {
    let mut svc = service();
    svc.seed(
        HERE,
        1,
        "amber-swift-owl",
        NOW - 70 * DAY,
        Some(LockState::Held),
    );
    for (scope, verb) in [
        (crate::Scope::Remove, "session remove"),
        (crate::Scope::Purge, "session purge"),
        (crate::Scope::Transcript, "session transcript purge"),
    ] {
        let request = RemoveRequest {
            scope,
            ..remove("amber-swift-owl", true)
        };
        match ready(svc.remove(request)).unwrap().refusal {
            Some(Refusal::NotProvenDead {
                verb: said, reason, ..
            }) => {
                assert_eq!(said, verb);
                assert!(reason.contains("owner is alive"), "{reason}");
            }
            other => panic!("{other:?}"),
        }
    }
}

// session.feature: "A transcript within its retention is refused by every
// destroyer" (session remove): refused, nothing destroyed, the plan shown.
#[test]
fn a_removal_within_the_retention_is_refused() {
    let mut svc = service();
    svc.seed(
        HERE,
        1,
        "brisk-copper-moth",
        NOW - 2 * HOUR,
        Some(LockState::Free),
    );
    ended(svc.store(HERE), 1, NOW - HOUR, NOW - HOUR);
    svc.files.last_active.insert(
        "brisk-copper-moth".to_string(),
        crate::Timestamp {
            seconds: NOW - HOUR,
            nanos: 0,
        },
    );
    let removal = ready(svc.remove(remove("brisk-copper-moth", true))).unwrap();
    assert!(matches!(removal.refusal, Some(Refusal::Retained { .. })));
    assert!(!removal.applied && !removal.index_entry_removed);
    assert!(svc.files.destroyed.is_empty());
    assert_eq!(svc.harps(all()), ["brisk-copper-moth"]);
    assert_eq!(removal.files.destroy.len(), 1, "the plan is still shown");
}

// session.feature: "A kept transcript is refused at any age".
#[test]
fn a_removal_of_a_kept_session_is_refused_at_any_age() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    ready(svc.keep(keep("amber-swift-owl", true))).unwrap();
    let removal = ready(svc.remove(remove("amber-swift-owl", true))).unwrap();
    assert_eq!(
        removal.refusal,
        Some(Refusal::Kept {
            harp: "amber-swift-owl".to_string()
        })
    );
    assert!(svc.files.destroyed.is_empty());
    ready(svc.keep(keep("amber-swift-owl", false))).unwrap();
    let removal = ready(svc.remove(remove("amber-swift-owl", true))).unwrap();
    assert!(removal.applied, "released, it goes");
}

// session.feature: "A session the lock cannot prove finished is refused": a
// free lock proves the owner dead; a held lock, or no lock at all, refuses.
#[test]
fn a_removal_of_a_session_the_lock_cannot_prove_dead_is_refused() {
    for (lock, name) in [(Some(LockState::Held), "held"), (None, "no lock")] {
        let mut svc = service();
        svc.seed(HERE, 1, "amber-swift-owl", NOW - 70 * DAY, lock);
        ended(svc.store(HERE), 1, NOW - 70 * DAY + 10, NOW - 60 * DAY);
        let removal = ready(svc.remove(remove("amber-swift-owl", true))).unwrap();
        assert!(
            matches!(removal.refusal, Some(Refusal::NotProvenDead { .. })),
            "{name}: {:?}",
            removal.refusal
        );
        assert!(svc.files.destroyed.is_empty(), "{name}");
    }
}

// session.feature: "--even-if-live sweeps it anyway".
#[test]
fn even_if_live_permits_a_session_whose_lock_proves_nothing() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 70 * DAY, None);
    ended(svc.store(HERE), 1, NOW - 70 * DAY + 10, NOW - 60 * DAY);
    let removal = ready(svc.remove(RemoveRequest {
        yes: true,
        even_if_live: true,
        ..remove("amber-swift-owl", true)
    }))
    .unwrap();
    assert!(removal.applied);
    assert_eq!(svc.harps(all()), Vec::<String>::new());
}

// Go: PurgeSession checks the lock first, then the retention.
#[test]
fn the_lock_is_asked_before_the_retention() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - 2 * HOUR, None);
    ended(svc.store(HERE), 1, NOW - HOUR, NOW - HOUR);
    let removal = ready(svc.remove(remove("amber-swift-owl", true))).unwrap();
    assert!(matches!(
        removal.refusal,
        Some(Refusal::NotProvenDead { .. })
    ));
}

// Go: TestRetention_OneShotIsReclaimableAtAnyAgeUnlessKept.
#[test]
fn an_internal_one_shot_is_reclaimable_at_any_age() {
    let mut svc = service();
    let store = svc.stores.stores.entry(HERE.to_string()).or_default();
    crate::fakes::run(
        store,
        1,
        NOW - 2 * HOUR,
        sessions::MintSession {
            harp: "gray-oneshot-wisp".to_string(),
            project_id: HERE.to_string(),
            project_dir: "/work/proj".to_string(),
            output_dir: String::new(),
            engine: "claude-code".to_string(),
            origin: sessions::Origin::Oneshot,
            started_at: None,
            depth: 0,
        },
    );
    ended(store, 1, NOW - HOUR, NOW - HOUR);
    svc.files.transcripts.insert(
        "gray-oneshot-wisp".to_string(),
        transcript_of("gray-oneshot-wisp"),
    );
    svc.liveness
        .0
        .insert("gray-oneshot-wisp".to_string(), LockState::Free);
    let removal = ready(svc.remove(remove("gray-oneshot-wisp", true))).unwrap();
    assert!(removal.applied, "{:?}", removal.refusal);
}

// Go: PurgeSession marks the entry purged BEFORE any file is unlinked: "if the
// process dies between the two, the index already reads 'purged'".
#[test]
fn the_purge_is_recorded_before_the_files_go() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    svc.files.fail_destroy = true;
    let error = ready(svc.remove(remove("amber-swift-owl", true))).unwrap_err();
    assert!(matches!(error, AppError::Files(_)), "{error:?}");
    let views = ready(svc.list(all())).unwrap();
    assert_eq!(views.len(), 1, "the entry is still there");
    assert!(
        views[0].record.purged_at.is_some(),
        "and says its transcript was purged"
    );
    assert!(views[0].summary().contains("purged"));
}

#[test]
fn a_removal_of_an_unknown_session_fails_naming_it() {
    let mut svc = service();
    assert_eq!(
        ready(svc.remove(remove("no-such-harp", true))).unwrap_err(),
        AppError::NoSession {
            harp: "no-such-harp".to_string()
        }
    );
}

// ------------------------------------------------------------------------ bind

fn bind(harp: &str, native: &str, transcript: &str) -> BindRequest {
    BindRequest {
        harp: harp.to_string(),
        native_id: native.to_string(),
        transcript_path: transcript.to_string(),
        ..BindRequest::default()
    }
}

// session_hooks.feature: "Binding a session records the harp both in the
// transcript and in the index": the index binds the harp to the session.
#[test]
fn a_bind_records_the_engines_session_on_the_harp() {
    let mut svc = service();
    svc.seed(HERE, 1, "brisk-copper-moth", NOW - DAY, None);
    let bound = ready(svc.bind(bind(
        "brisk-copper-moth",
        "vendor-session-42",
        "/tmp/vendor/transcript.jsonl",
    )))
    .unwrap();
    assert!(matches!(bound, Bound::Recorded { .. }));
    let view = ready(svc.show(ShowQuery {
        harp: "brisk-copper-moth".to_string(),
        ..ShowQuery::default()
    }))
    .unwrap();
    assert_eq!(view.record.native_id, "vendor-session-42");
    assert_eq!(view.record.transcript_path, "/tmp/vendor/transcript.jsonl");
}

// Go: TestBindSession_Idempotent, _RotationRepointsTheBinding.
#[test]
fn a_repeat_bind_changes_nothing_and_a_new_session_rotates() {
    let mut svc = service();
    svc.seed(HERE, 1, "brisk-copper-moth", NOW - DAY, None);
    ready(svc.bind(bind("brisk-copper-moth", "first", "/t1"))).unwrap();
    assert_eq!(
        ready(svc.bind(bind("brisk-copper-moth", "first", "/t1"))).unwrap(),
        Bound::Unchanged
    );
    assert!(matches!(
        ready(svc.bind(bind("brisk-copper-moth", "second", "/t2"))).unwrap(),
        Bound::Recorded { .. }
    ));
    let view = ready(svc.show(ShowQuery {
        harp: "brisk-copper-moth".to_string(),
        ..ShowQuery::default()
    }))
    .unwrap();
    assert_eq!(view.record.native_id, "second");
    assert_eq!(view.record.rotations.len(), 1);
    assert_eq!(view.record.rotations[0].native_id, "first");
}

// Go: operations.BindSession "no-ops a harp that is absent from the index": a
// hook never fails an engine's startup over it.
#[test]
fn a_bind_of_a_harp_nobody_recorded_is_not_a_failure() {
    let mut svc = service();
    assert_eq!(
        ready(svc.bind(bind("no-such-harp", "id", "/t"))).unwrap(),
        Bound::NoSuchSession
    );
    assert!(svc.stores.opened.is_empty(), "and creates nothing");
}

// ---------------------------------------------------------------------- rename

// plan 3.1 / D5 Q1: a rename claims the new name, records it, and frees the
// old one; the output folder moves beside itself, to the new name; the stable
// id never changes.
#[test]
fn a_rename_moves_the_claim_frees_the_old_name_and_moves_the_output_folder() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    let old_out = "/home/Documents/ctxloom/proj/amber-swift-owl";
    let new_out = "/home/Documents/ctxloom/proj/bright-keen-hawk";
    svc.files.outputs.insert(old_out.to_string());
    claim(svc.store(HERE), HERE, "amber-swift-owl", &id_text(1)).unwrap();
    let renamed = ready(svc.rename(rename("amber-swift-owl", "bright-keen-hawk"))).unwrap();
    assert_eq!(
        (
            renamed.from.as_str(),
            renamed.to.as_str(),
            renamed.output_dir.as_str()
        ),
        ("amber-swift-owl", "bright-keen-hawk", new_out)
    );
    assert_eq!(
        svc.files.moves,
        [(old_out.to_string(), new_out.to_string())]
    );
    let views = ready(svc.list(all())).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].record.harp, "bright-keen-hawk");
    assert_eq!(views[0].record.output_dir, new_out);
    assert_eq!(
        views[0].record.id,
        id_text(1),
        "the stable id never changes"
    );
    let store = svc.store(HERE);
    assert_eq!(holder(store, HERE, "bright-keen-hawk"), id_text(1));
    assert_eq!(
        holder(store, HERE, "amber-swift-owl"),
        "",
        "the old name is free"
    );
    assert!(
        claim(store, HERE, "amber-swift-owl", &id_text(2)).is_ok(),
        "and another may take it"
    );
}

fn rename(from: &str, to: &str) -> RenameRequest {
    RenameRequest {
        harp: from.to_string(),
        to: to.to_string(),
        ..RenameRequest::default()
    }
}

// D5 Q1: a name another session holds is refused, and nothing moves.
#[test]
fn a_rename_onto_a_name_another_session_holds_is_refused_and_changes_nothing() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    svc.seed(HERE, 2, "brisk-copper-moth", NOW - DAY, None);
    claim(svc.store(HERE), HERE, "amber-swift-owl", &id_text(1)).unwrap();
    claim(svc.store(HERE), HERE, "brisk-copper-moth", &id_text(2)).unwrap();
    let error = ready(svc.rename(rename("amber-swift-owl", "brisk-copper-moth"))).unwrap_err();
    assert_eq!(error.code(), "sessions-names-taken");
    assert_eq!(error.exit_code(), 1);
    assert!(svc.files.moves.is_empty());
    assert_eq!(holder(svc.store(HERE), HERE, "amber-swift-owl"), id_text(1));
    assert_eq!(
        holder(svc.store(HERE), HERE, "brisk-copper-moth"),
        id_text(2)
    );
    let mut harps = svc.harps(all());
    harps.sort();
    assert_eq!(harps, ["amber-swift-owl", "brisk-copper-moth"]);
}

// Go: Rename onto its own name collides with itself ("name already in use").
#[test]
fn a_rename_to_the_name_it_has_is_refused() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    claim(svc.store(HERE), HERE, "amber-swift-owl", &id_text(1)).unwrap();
    let error = ready(svc.rename(rename("amber-swift-owl", "amber-swift-owl"))).unwrap_err();
    assert_eq!(error.code(), "sessions-names-taken");
    assert_eq!(
        holder(svc.store(HERE), HERE, "amber-swift-owl"),
        id_text(1),
        "still held"
    );
}

// Go: TestRename_RefusesUnsafeNewName / ErrNameTooLong, before anything is
// claimed: a refused rename leaves no claim behind.
#[test]
fn a_rename_to_a_name_no_session_may_have_claims_nothing() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    for bad in ["../../etc", "a/b", " lead", &"x".repeat(25)] {
        let error = ready(svc.rename(rename("amber-swift-owl", bad))).unwrap_err();
        assert_eq!(error.exit_code(), 2, "{bad:?}");
        assert_eq!(holder(svc.store(HERE), HERE, bad), "", "{bad:?}");
    }
    assert!(svc.files.moves.is_empty());
    assert_eq!(svc.harps(all()), ["amber-swift-owl"]);
}

// Go: "A refusal leaves everything where it was: each step is undone if a
// later one fails": a folder that cannot move gives the new name back.
#[test]
fn a_folder_that_cannot_move_refuses_the_rename_whole() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    claim(svc.store(HERE), HERE, "amber-swift-owl", &id_text(1)).unwrap();
    svc.files.fail_move = true;
    let error = ready(svc.rename(rename("amber-swift-owl", "bright-keen-hawk"))).unwrap_err();
    assert!(matches!(error, AppError::Files(_)), "{error:?}");
    assert_eq!(
        holder(svc.store(HERE), HERE, "bright-keen-hawk"),
        "",
        "the new name is given back"
    );
    assert_eq!(holder(svc.store(HERE), HERE, "amber-swift-owl"), id_text(1));
    assert_eq!(svc.harps(all()), ["amber-swift-owl"]);
}

// A session minted before claims existed holds no claim on its name: a rename
// still works, and frees nothing it never held.
#[test]
fn a_session_with_no_claim_on_its_name_can_still_be_renamed() {
    let mut svc = service();
    svc.seed(HERE, 1, "amber-swift-owl", NOW - DAY, None);
    ready(svc.rename(rename("amber-swift-owl", "bright-keen-hawk"))).unwrap();
    assert_eq!(svc.harps(all()), ["bright-keen-hawk"]);
    assert_eq!(
        holder(svc.store(HERE), HERE, "bright-keen-hawk"),
        id_text(1)
    );
}

// plan 3.1: "a forgotten session's name stays claimed", so the harp is never
// reused.
#[test]
fn a_removed_sessions_name_stays_claimed() {
    let mut svc = service();
    finished(&mut svc, 1, "amber-swift-owl");
    claim(svc.store(HERE), HERE, "amber-swift-owl", &id_text(1)).unwrap();
    assert!(
        ready(svc.remove(remove("amber-swift-owl", true)))
            .unwrap()
            .applied
    );
    assert_eq!(svc.harps(all()), Vec::<String>::new());
    assert_eq!(holder(svc.store(HERE), HERE, "amber-swift-owl"), id_text(1));
    assert!(claim(svc.store(HERE), HERE, "amber-swift-owl", &id_text(2)).is_err());
}

// session.feature: "Editing a session nobody recorded fails": the harp is named,
// whatever else is unavailable.
#[test]
fn a_rename_of_a_session_nobody_recorded_fails_naming_it() {
    let mut svc = service();
    assert_eq!(
        ready(svc.rename(RenameRequest {
            harp: "no-such-harp".to_string(),
            to: "bright-keen-hawk".to_string(),
            ..RenameRequest::default()
        }))
        .unwrap_err(),
        AppError::NoSession {
            harp: "no-such-harp".to_string()
        }
    );
}

// ---------------------------------------------------------------------- errors

#[test]
fn a_project_that_cannot_be_resolved_is_an_identity_failure() {
    let mut svc = service();
    svc.identity.fails = Some(angzarr_diagnostic::Report::new("x-no-home"));
    let error = ready(svc.list(ListQuery::default())).unwrap_err();
    assert!(matches!(error, AppError::Identity(_)), "{error:?}");
    assert_eq!(error.code(), "x-no-home");
}

// The refusal for a lock says which: held means the owner is running.
#[test]
fn a_held_lock_and_a_missing_one_are_refused_for_different_reasons() {
    let reason = |lock: Option<LockState>| {
        let mut svc = service();
        svc.seed(HERE, 1, "amber-swift-owl", NOW - 70 * DAY, lock);
        match ready(svc.remove(remove("amber-swift-owl", true)))
            .unwrap()
            .refusal
        {
            Some(Refusal::NotProvenDead { reason, .. }) => reason,
            other => panic!("{other:?}"),
        }
    };
    let held = reason(Some(LockState::Held));
    let none = reason(None);
    assert!(held.contains("running"), "{held}");
    assert!(none.contains("no lock"), "{none}");
}

// A session that records no output folder has none to move.
#[test]
fn a_session_with_no_output_folder_renames_without_moving_one() {
    let mut svc = service();
    let store = svc.stores.stores.entry(HERE.to_string()).or_default();
    crate::fakes::run(
        store,
        1,
        NOW - DAY,
        sessions::MintSession {
            harp: "amber-swift-owl".to_string(),
            project_id: HERE.to_string(),
            project_dir: "/work/proj".to_string(),
            output_dir: String::new(),
            engine: "claude-code".to_string(),
            origin: sessions::Origin::Session,
            started_at: None,
            depth: 0,
        },
    );
    let renamed = ready(svc.rename(rename("amber-swift-owl", "bright-keen-hawk"))).unwrap();
    assert_eq!(renamed.output_dir, "");
    assert!(svc.files.moves.is_empty());
}

// The sample dead letter names its component and reason (a driving adapter's
// tests say it).
#[test]
fn the_sample_dead_letter_names_its_component_and_reason() {
    let line = crate::dead_letter_line(&crate::dead_letter_sample("sessions").letter);
    assert!(
        line.contains("sessions") && line.contains("ROUTER_REASON_UNHANDLED_HANDLER_ERROR"),
        "{line}"
    );
}
