//! A session on angzarr-embedded, driven through its kernel exactly as the
//! binary drives it: `Kernel::handle` over angzarr-ports' in-memory store
//! (angzarr-embedded's test support, `Harness`). The rules are Go's, from
//! `internal/core/sessions` (release/0.7): each test names the Go test or
//! condition it ports, or says it is design.

use std::collections::BTreeMap;

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::testing::{Harness, Samples};
use angzarr_embedded::v1::{
    command_page, CommandBook, CommandPage, Cover, EventPage, Fact, FactOrigin, Uuid,
};
use angzarr_embedded::{Any, Failure, Payload, Registry, Timestamp};

use crate::{
    AdoptRotations, BindEngineSession, EndRecorded, EngineSessionBound, ForgetSession, KeptChanged,
    MintSession, Origin, RecordEnd, RecordPurge, RenameSession, RenamedTo, Rotation,
    SessionAggregate, SessionCommand, SessionEvent, SessionForgotten, SessionPurged,
    SessionRejection, SessionState, SetKept, SESSION_DOMAIN,
};

pub(crate) const HARP: &str = "amber-swift-owl";

/// What a diagnostic says: its key and its args, each as text, by name
/// (decision 14.37: a key and a map, never English).
pub(crate) type Said = (String, BTreeMap<String, String>);

pub(crate) fn said(diagnostic: &dyn Diagnostic) -> Said {
    let args = diagnostic
        .args()
        .into_iter()
        .map(|(name, arg)| {
            let text = match arg {
                Arg::Int(n) => n.to_string(),
                Arg::Uint(n) => n.to_string(),
                Arg::Str(s) => s,
                Arg::Nested(cause) => cause.code().to_string(),
            };
            (name.to_string(), text)
        })
        .collect();
    (diagnostic.code().to_string(), args)
}

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn cover() -> Cover {
    Cover {
        domain: SESSION_DOMAIN.to_string(),
        root: Some(Uuid::from_bytes([7; 16])),
        edition: None,
    }
}

/// One project's store holding one session's stream, through
/// angzarr-embedded's test support: the kernel over an in-memory store, each
/// command one second after the last (from second 101).
pub(crate) struct Session {
    harness: Harness<SessionAggregate>,
}

impl Session {
    pub(crate) fn new() -> Session {
        let mut registry = Registry::new();
        registry
            .aggregate::<SessionAggregate>(SessionAggregate::default)
            .unwrap();
        Session {
            harness: Harness::new(registry, cover()).with_samples(Samples::starting_at(100)),
        }
    }

    /// A session minted at second 101.
    pub(crate) fn minted() -> Session {
        let mut session = Session::new();
        session.run(mint(HARP)).unwrap();
        session
    }

    /// Run `command` one second after the last: the events it committed, or
    /// its refusal as said.
    pub(crate) fn run(
        &mut self,
        command: impl Into<SessionCommand>,
    ) -> Result<Vec<SessionEvent>, Said> {
        match self.harness.when(command).result() {
            Ok(events) => Ok(events),
            Err(Failure::Rejected(rejected)) => Err(said(&rejected)),
            Err(other) => panic!("not a rejection: {other:?}"),
        }
    }

    /// Record `fact` as the listener delivers one: a Fact body with its
    /// derived `external_id` (S5). The events it recorded, or none for a
    /// fact that records nothing.
    pub(crate) fn fact(
        &mut self,
        fact: impl DomainMessage,
        external_id: &str,
    ) -> Vec<SessionEvent> {
        let book = CommandBook {
            cover: Some(cover()),
            page: Some(CommandPage {
                body: Some(command_page::Body::Fact(Fact {
                    payload: Some(Any::from_msg(&fact.to_wire()).unwrap()),
                })),
                fact_origin: Some(FactOrigin {
                    external_id: external_id.to_string(),
                    description: String::new(),
                }),
                ..CommandPage::default()
            }),
            handling: None,
        };
        self.harness.when_book(book).events()
    }

    /// The pages stored, oldest first.
    pub(crate) fn pages(&self) -> Vec<EventPage> {
        self.harness.pages()
    }

    /// The session, replayed from the store as the next process would.
    pub(crate) fn state(&self) -> SessionState {
        self.harness.state().state().clone()
    }
}

pub(crate) fn mint(harp: &str) -> MintSession {
    MintSession {
        harp: harp.to_string(),
        project_id: "calm-otter".to_string(),
        project_dir: "/work/proj".to_string(),
        output_dir: format!("/home/Documents/ctxloom/proj/{harp}"),
        engine: "claude-code".to_string(),
        origin: Origin::Session,
        started_at: None,
    }
}

fn bind(native_id: &str, transcript_path: &str) -> BindEngineSession {
    BindEngineSession {
        native_id: native_id.to_string(),
        transcript_path: transcript_path.to_string(),
        engine_version: None,
    }
}

fn rotation(native_id: &str, transcript_path: &str, rotated_at: Timestamp) -> Rotation {
    Rotation {
        native_id: native_id.to_string(),
        transcript_path: transcript_path.to_string(),
        rotated_at: Some(rotated_at),
    }
}

fn rename(harp: &str) -> RenameSession {
    RenameSession {
        harp: harp.to_string(),
        output_dir: format!("/home/Documents/ctxloom/proj/{harp}"),
    }
}

// ------------------------------------------------------------------- mint

// Go: `AssignHarp` records the project dir and the backend; the output dir is
// recorded at the mint, absolute; the start is `time.Now()`, here the
// context's `recorded_at`, the one instant the application sampled.
#[test]
fn a_mint_records_the_session_at_the_sampled_instant() {
    let mut session = Session::new();
    let events = session.run(mint(HARP)).unwrap();
    let [SessionEvent::SessionMinted(minted)] = events.as_slice() else {
        panic!("one SessionMinted, not {events:?}");
    };
    assert_eq!(minted.harp, HARP);
    assert_eq!(
        minted.started_at, None,
        "the event copies no instant: its page's recorded_at is the start"
    );
    let state = session.state();
    assert_eq!(state.harp, HARP);
    assert_eq!(state.project_id, "calm-otter");
    assert_eq!(state.project_dir, "/work/proj");
    assert_eq!(
        state.output_dir,
        "/home/Documents/ctxloom/proj/amber-swift-owl"
    );
    assert_eq!(state.engine, "claude-code");
    assert_eq!(state.origin, Some(Origin::Session));
    assert_eq!(state.started_at, at(101));
    assert!(state.native_id.is_empty(), "pending until the engine binds");
    assert!(!state.kept && !state.forgotten);
    assert_eq!(state.ended_at, None);
}

// The importer mints a session Go started long ago: Go's `started_at`, read in
// preference to the instant of the import (design: plan 9).
#[test]
fn a_mint_that_carries_a_start_is_started_then() {
    let mut session = Session::new();
    let started = at(1_700_000_000);
    session
        .run(MintSession {
            started_at: Some(started),
            ..mint(HARP)
        })
        .unwrap();
    assert_eq!(session.state().started_at, started);
}

// Go: `harp.Validate` is the rule for every name that becomes a path component.
#[test]
fn a_mint_with_a_harp_that_cannot_be_a_name_is_refused_and_records_nothing() {
    let mut session = Session::new();
    let refusal = session.run(mint("../etc")).unwrap_err();
    assert_eq!(
        refusal,
        said(&SessionRejection::HarpInvalid {
            harp: "../etc".to_string(),
            reason: "separator".to_string(),
        })
    );
    assert!(session.pages().is_empty(), "a refusal records nothing");
}

// ------------------------------------------------------------------- bind

// Go: TestBindSession_FillsPendingEntry.
#[test]
fn a_bind_fills_the_pending_session() {
    let mut session = Session::minted();
    let events = session
        .run(bind("session-uuid-123", "/tmp/transcript.jsonl"))
        .unwrap();
    let [SessionEvent::EngineSessionBound(bound)] = events.as_slice() else {
        panic!("one EngineSessionBound, not {events:?}");
    };
    assert_eq!(bound.displaced, None, "the first bind displaces nothing");
    let state = session.state();
    assert_eq!(state.native_id, "session-uuid-123");
    assert_eq!(state.transcript_path, "/tmp/transcript.jsonl");
    assert!(state.rotations.is_empty());
}

// Go: TestBindSession_Idempotent. A repeat of the bound id records nothing:
// the hook fires again on every resume.
#[test]
fn a_bind_of_the_bound_id_records_nothing() {
    let mut session = Session::minted();
    session.run(bind("uuid-1", "/t1")).unwrap();
    let before = session.pages().len();
    let events = session.run(bind("uuid-1", "/t1")).unwrap();
    assert!(events.is_empty());
    assert_eq!(session.pages().len(), before);
    assert!(
        session.state().rotations.is_empty(),
        "an idempotent rebind never appends a rotation (Go: TestBindSession_IdempotentRebindNeverAppendsRotation)"
    );
}

// Go: TestBindSession_RotationRepointsTheBinding. A new id that arrives with a
// transcript is the engine rotating its transcript under a live process
// (claude-code's /clear): the binding follows it and the displaced one is kept
// in the lineage, stamped when it was displaced.
#[test]
fn a_bind_of_a_new_id_with_a_transcript_rotates_and_keeps_the_displaced_binding() {
    let mut session = Session::minted();
    session.run(bind("first-id", "/orig")).unwrap();
    let events = session.run(bind("second-id", "/new")).unwrap();
    let [SessionEvent::EngineSessionBound(bound)] = events.as_slice() else {
        panic!("one EngineSessionBound, not {events:?}");
    };
    let displaced = bound.displaced.as_ref().expect("the displaced binding");
    assert_eq!(
        (
            displaced.native_id.as_str(),
            displaced.transcript_path.as_str()
        ),
        ("first-id", "/orig")
    );
    let state = session.state();
    assert_eq!(state.native_id, "second-id", "rotation re-points the id");
    assert_eq!(state.transcript_path, "/new", "and the transcript");
    assert_eq!(state.rotations.len(), 1);
    assert_eq!(state.rotations[0].native_id, "first-id");
    assert_eq!(state.rotations[0].transcript_path, "/orig");
    assert_eq!(
        state.rotations[0].rotated_at,
        Some(at(103)),
        "stamped with the instant it was displaced"
    );
}

// Go: TestBindSession_MultipleRotationsAppendOldestFirst.
#[test]
fn rotations_accumulate_oldest_first() {
    let mut session = Session::minted();
    for (id, path) in [("id-1", "/t1"), ("id-2", "/t2"), ("id-3", "/t3")] {
        session.run(bind(id, path)).unwrap();
    }
    let state = session.state();
    assert_eq!(state.native_id, "id-3");
    let lineage: Vec<(&str, &str)> = state
        .rotations
        .iter()
        .map(|r| (r.native_id.as_str(), r.transcript_path.as_str()))
        .collect();
    assert_eq!(lineage, [("id-1", "/t1"), ("id-2", "/t2")]);
}

// Go: TestBindSession_IDOnlyBindNeverDisplacesALiveBinding. A binder that
// knows an id but names no transcript (the compactor's backstop) must never
// displace what the SessionStart hook established.
#[test]
fn a_bind_of_a_new_id_with_no_transcript_never_displaces_the_binding() {
    let mut session = Session::minted();
    session.run(bind("first-id", "/orig")).unwrap();
    let before = session.pages().len();
    let events = session.run(bind("stale-id", "")).unwrap();
    assert!(events.is_empty());
    assert_eq!(session.pages().len(), before);
    let state = session.state();
    assert_eq!(state.native_id, "first-id");
    assert_eq!(state.transcript_path, "/orig");
    assert!(state.rotations.is_empty());
}

// Go: BindSession short-circuits when both are empty, "the ordinary shape of
// a hook payload that carried no session identifier at all".
#[test]
fn a_bind_naming_neither_an_id_nor_a_transcript_records_nothing() {
    let mut session = Session::minted();
    let before = session.pages().len();
    assert!(session.run(bind("", "")).unwrap().is_empty());
    assert_eq!(session.pages().len(), before);
}

// Go: BindSession on an unbound entry sets the transcript path when the id is
// empty ("if transcriptPath != ''").
#[test]
fn a_bind_of_a_transcript_alone_binds_the_transcript() {
    let mut session = Session::minted();
    session.run(bind("", "/t0")).unwrap();
    let state = session.state();
    assert_eq!(state.native_id, "");
    assert_eq!(state.transcript_path, "/t0");
}

// Go: BindSession records the engine's version only with `RecordEngineVersion`;
// here it rides the bind that learned it, and an empty one never blanks it.
#[test]
fn a_bind_records_the_engine_version_and_a_later_one_without_it_keeps_it() {
    let mut session = Session::minted();
    session
        .run(BindEngineSession {
            engine_version: Some("2.1.229 (Claude Code)".to_string()),
            ..bind("id-1", "/t1")
        })
        .unwrap();
    assert_eq!(session.state().engine_version, "2.1.229 (Claude Code)");
    session.run(bind("id-2", "/t2")).unwrap();
    assert_eq!(session.state().engine_version, "2.1.229 (Claude Code)");
}

// Go: TestBindSession_UnknownHarpErrors.
#[test]
fn a_bind_on_a_stream_holding_no_session_is_refused() {
    let mut session = Session::new();
    assert_eq!(
        session.run(bind("uuid", "/t")).unwrap_err(),
        said(&SessionRejection::NoSession)
    );
}

// An engine version given empty never blanks the one held.
#[test]
fn a_bind_naming_an_empty_engine_version_keeps_the_one_held() {
    let mut session = Session::minted();
    session
        .run(BindEngineSession {
            engine_version: Some("2.1".to_string()),
            ..bind("id-1", "/t1")
        })
        .unwrap();
    session
        .run(BindEngineSession {
            engine_version: Some(String::new()),
            ..bind("id-2", "/t2")
        })
        .unwrap();
    assert_eq!(session.state().engine_version, "2.1");
}

// A repeat of a transcript-only bind (an unbound session) changes nothing.
#[test]
fn a_repeat_of_a_transcript_only_bind_records_nothing() {
    let mut session = Session::minted();
    session.run(bind("", "/t0")).unwrap();
    let before = session.pages().len();
    assert!(session.run(bind("", "/t0")).unwrap().is_empty());
    assert!(
        session.run(bind("id-1", "")).unwrap().len() == 1,
        "an id alone binds an unbound session"
    );
    assert_eq!(session.pages().len(), before + 1);
    assert_eq!(
        session.state().transcript_path,
        "/t0",
        "and keeps the transcript"
    );
}

// ------------------------------------------------------------------ adopt

// One adopt naming a session twice records it once.
#[test]
fn an_adopt_naming_a_session_twice_records_it_once() {
    let mut session = Session::minted();
    let events = adopted(
        &mut session,
        vec![rotation("id-1", "/a", at(5)), rotation("id-1", "/b", at(6))],
    );
    let [SessionEvent::SessionAdopted(event)] = events.as_slice() else {
        panic!("one SessionAdopted, not {events:?}");
    };
    assert_eq!(event.rotations.len(), 1);
    assert_eq!(session.state().rotations.len(), 1);
}

fn adopted(session: &mut Session, rotations: Vec<Rotation>) -> Vec<SessionEvent> {
    session.run(AdoptRotations { rotations }).unwrap()
}

// Go: TestAppendRotations_InsertsBeforeExistingByRotatedAt. The adopted
// rotation is chronologically earliest, so it sorts first; the live binding
// is untouched.
#[test]
fn an_adopted_rotation_is_ordered_by_when_it_rotated() {
    let mut session = Session::minted();
    session.run(bind("id-2", "/t2")).unwrap();
    session.run(bind("id-3", "/t3")).unwrap();
    adopted(&mut session, vec![rotation("id-1", "/t1", at(5))]);
    let state = session.state();
    let ids: Vec<&str> = state
        .rotations
        .iter()
        .map(|r| r.native_id.as_str())
        .collect();
    assert_eq!(ids, ["id-1", "id-2"]);
    assert_eq!(state.native_id, "id-3", "the live binding is untouched");
}

// Go: TestAppendRotations_SkipsSessionIDAlreadyInLineage.
#[test]
fn an_adopt_skips_what_the_lineage_already_holds() {
    let mut session = Session::minted();
    session.run(bind("id-1", "/t1")).unwrap();
    session.run(bind("id-2", "/t2")).unwrap();
    let events = adopted(
        &mut session,
        vec![
            rotation("id-1", "/t1", at(500)),
            rotation("id-2", "/t2", at(500)),
            rotation("id-3", "/t3", at(500)),
        ],
    );
    let [SessionEvent::SessionAdopted(event)] = events.as_slice() else {
        panic!("one SessionAdopted, not {events:?}");
    };
    let recorded: Vec<&str> = event
        .rotations
        .iter()
        .map(|r| r.native_id.as_str())
        .collect();
    assert_eq!(recorded, ["id-3"], "only the genuinely new rotation");
    let state = session.state();
    let mut ids: Vec<&str> = state
        .rotations
        .iter()
        .map(|r| r.native_id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, ["id-1", "id-3"]);
}

// Go: TestAppendRotations_EmptyIsANoOp, and the repeat adopt "a safe no-op
// rather than a duplicate-lineage hazard".
#[test]
fn an_adopt_of_nothing_new_records_nothing() {
    let mut session = Session::minted();
    let before = session.pages().len();
    assert!(adopted(&mut session, Vec::new()).is_empty());
    adopted(&mut session, vec![rotation("id-1", "/t1", at(5))]);
    let after_first = session.pages().len();
    assert!(adopted(&mut session, vec![rotation("id-1", "/t1", at(5))]).is_empty());
    assert_eq!(session.pages().len(), after_first);
    assert_eq!(after_first, before + 1);
}

// Go: TestAppendRotations_UnknownHarpErrors.
#[test]
fn an_adopt_on_a_stream_holding_no_session_is_refused() {
    let mut session = Session::new();
    assert_eq!(
        session
            .run(AdoptRotations {
                rotations: vec![rotation("x", "", at(1))]
            })
            .unwrap_err(),
        said(&SessionRejection::NoSession)
    );
}

// ------------------------------------------------------------------- keep

// Go: SetKeep: "Idempotent: an unchanged mark is not rewritten."
#[test]
fn keep_marks_and_unmarks_and_records_only_a_change() {
    let mut session = Session::minted();
    let events = session.run(SetKept { kept: true }).unwrap();
    assert_eq!(
        events,
        [SessionEvent::KeptChanged(KeptChanged { kept: true })]
    );
    assert!(session.state().kept);
    let before = session.pages().len();
    assert!(session.run(SetKept { kept: true }).unwrap().is_empty());
    assert_eq!(
        session.pages().len(),
        before,
        "an unchanged mark records nothing"
    );
    session.run(SetKept { kept: false }).unwrap();
    assert!(!session.state().kept);
    assert!(session.run(SetKept { kept: false }).unwrap().is_empty());
}

// ----------------------------------------------------------------- rename

// Go: TestRename; and `session edit --name`'s record "names where the folder
// went" (session.feature, "--name renames the harp and moves its output
// folder"). The name's claim is the application's (design, plan 3.1).
#[test]
fn a_rename_records_the_new_name_and_where_the_output_went() {
    let mut session = Session::minted();
    let events = session.run(rename("bright-keen-hawk")).unwrap();
    assert_eq!(
        events,
        [SessionEvent::RenamedTo(RenamedTo {
            harp: "bright-keen-hawk".to_string(),
            output_dir: "/home/Documents/ctxloom/proj/bright-keen-hawk".to_string(),
        })]
    );
    let state = session.state();
    assert_eq!(state.harp, "bright-keen-hawk");
    assert_eq!(
        state.output_dir,
        "/home/Documents/ctxloom/proj/bright-keen-hawk"
    );
    assert_eq!(state.project_dir, "/work/proj", "the rest is untouched");
}

// Go: TestRename_RefusesUnsafeNewName: the new name becomes a path component.
#[test]
fn a_rename_to_an_unsafe_name_is_refused_and_the_name_is_kept() {
    for (bad, reason) in [
        ("..", "directory"),
        ("../..", "separator"),
        ("../../etc/passwd", "separator"),
        ("a/b", "separator"),
        ("a\\b", "separator"),
        (" lead", "whitespace"),
        ("nul\0name", "control"),
        ("", "empty"),
    ] {
        let mut session = Session::minted();
        let before = session.pages().len();
        let refusal = session.run(rename(bad)).unwrap_err();
        assert_eq!(
            refusal,
            said(&SessionRejection::HarpInvalid {
                harp: bad.to_string(),
                reason: reason.to_string(),
            }),
            "{bad:?}"
        );
        assert_eq!(session.pages().len(), before, "{bad:?} records nothing");
        assert_eq!(session.state().harp, HARP);
    }
}

// Go: harp.ValidateRename and ErrNameTooLong (MaxNameLen 24).
#[test]
fn a_rename_past_the_limit_is_refused() {
    let mut session = Session::minted();
    let long = "a".repeat(25);
    assert_eq!(
        session.run(rename(&long)).unwrap_err(),
        said(&SessionRejection::HarpTooLong {
            harp: long,
            limit: 24,
            length: 25,
        })
    );
    session.run(rename(&"b".repeat(24))).unwrap();
}

// Go: Rename onto the same name collides with itself; the claim stream is
// where uniqueness lives here, so the domain records nothing for it.
#[test]
fn a_rename_to_the_name_held_records_nothing() {
    let mut session = Session::minted();
    let before = session.pages().len();
    assert!(session.run(rename(HARP)).unwrap().is_empty());
    assert_eq!(session.pages().len(), before);
}

// ----------------------------------------------------------------- forget

// Go: TestForget: "second forget should error since entry is gone"; a
// forgotten session is not found by anything else either. The stream stays
// (plan 3.1: the name is never reused).
#[test]
fn a_forgotten_session_is_gone_to_every_command() {
    let mut session = Session::minted();
    let events = session.run(ForgetSession {}).unwrap();
    assert_eq!(
        events,
        [SessionEvent::SessionForgotten(SessionForgotten {})]
    );
    assert!(session.state().forgotten);
    let gone = said(&SessionRejection::Forgotten {
        harp: HARP.to_string(),
    });
    assert_eq!(session.run(ForgetSession {}).unwrap_err(), gone);
    assert_eq!(session.run(bind("a", "/b")).unwrap_err(), gone);
    assert_eq!(session.run(SetKept { kept: true }).unwrap_err(), gone);
    assert_eq!(session.run(rename("other-name")).unwrap_err(), gone);
}

// ------------------------------------------------------------------ purge

// Go: MarkPurged stamps PurgedAt, "so a purged session stays visible,
// marked". The instant is the page's.
#[test]
fn a_purge_fact_marks_the_session_purged_at_the_instant() {
    let mut session = Session::minted();
    session
        .run(RecordPurge {
            files: 3,
            bytes: 1024,
            at: None,
        })
        .unwrap();
    assert_eq!(session.state().purged_at, Some(at(102)));
}

#[test]
fn a_purge_fact_that_carries_its_instant_is_read_in_preference() {
    let mut session = Session::minted();
    let events = session
        .run(RecordPurge {
            files: 1,
            bytes: 1,
            at: Some(at(42)),
        })
        .unwrap();
    assert_eq!(
        events,
        [SessionEvent::SessionPurged(SessionPurged {
            files: 1,
            bytes: 1,
            at: Some(at(42)),
        })]
    );
    assert_eq!(session.state().purged_at, Some(at(42)));
}

// ------------------------------------------------------------------- end

// Go: MarkEnded sets EndedAt. PROVISIONAL: the importer's record of Go's
// ended_at (plan 9, open).
#[test]
fn an_end_fact_records_when_the_last_run_ended() {
    let mut session = Session::minted();
    let events = session.run(RecordEnd { at: at(900) }).unwrap();
    assert_eq!(
        events,
        [SessionEvent::EndRecorded(EndRecorded { at: at(900) })]
    );
    assert_eq!(session.state().ended_at, Some(at(900)));
}

// ------------------------------------------------------------------ wire

#[test]
fn the_state_round_trips_through_its_wire_form() {
    let mut session = Session::minted();
    session.run(bind("id-1", "/t1")).unwrap();
    session.run(bind("id-2", "/t2")).unwrap();
    session.run(SetKept { kept: true }).unwrap();
    let state = session.state();
    let wire = sessions_proto::v1::SessionState::from(state.clone());
    assert_eq!(SessionState::try_from(wire).unwrap(), state);
}

// A stored event is read back by its type URL (the kernel's replay).
#[test]
fn an_event_is_read_back_by_its_type_url() {
    let event = SessionEvent::from(EngineSessionBound {
        native_id: "n".to_string(),
        transcript_path: "/t".to_string(),
        engine_version: String::new(),
        displaced: None,
    });
    let any = event.to_any();
    assert_eq!(SessionEvent::from_any(&any).unwrap(), event);
}

/// What the sibling test modules share.
pub(crate) mod tests_support {
    pub(crate) use super::{mint, said, Session, HARP};
}

#[path = "stable_id.test.rs"]
mod stable_id;
