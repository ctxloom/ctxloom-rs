//! A run on angzarr-embedded, driven through its kernel as the binary drives
//! it (`Harness`, over an in-memory store). RED (S5, design-by-test): the
//! aggregate is a stub. Each test says PORTED-FROM-GO (naming the Go test it
//! ports) or NEW-DESIGN (needs human review; the plan section it follows).

use std::collections::BTreeMap;

use angzarr_diagnostic::{Arg, Diagnostic};
use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::testing::{Harness, Samples};
use angzarr_embedded::v1::{command_page, CommandBook, CommandPage, Cover, Fact, FactOrigin};
use angzarr_embedded::{Any, Failure, Handled, Payload, Registry};

use crate::{
    run_root, EndCause, EnqueueRun, LaunchFailed, QueueTurn, RequestStop, RunAggregate, RunCommand,
    RunEnded, RunEvent, RunExited, RunMode, RunState, RunnerAttached, TurnCompleted, TurnFailed,
    TurnFailureClass, TurnStarted, RUN_DOMAIN,
};

const SESSION: &str = "5f0c7a52-1111-5111-8111-000000000001";
const HASH: [u8; 32] = [0xab; 32];

type Said = (String, BTreeMap<String, String>);

fn said(diagnostic: &dyn Diagnostic) -> Said {
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

fn cover() -> Cover {
    Cover {
        domain: RUN_DOMAIN.to_string(),
        root: Some(run_root(SESSION, 1)),
        edition: None,
    }
}

struct Run {
    harness: Harness<RunAggregate>,
}

impl Run {
    fn new() -> Run {
        let mut registry = Registry::new();
        registry
            .aggregate::<RunAggregate>(RunAggregate::default)
            .unwrap();
        Run {
            harness: Harness::new(registry, cover()).with_samples(Samples::starting_at(100)),
        }
    }

    fn enqueued() -> Run {
        let mut run = Run::new();
        run.command(enqueue()).unwrap();
        run
    }

    fn command(&mut self, command: impl Into<RunCommand>) -> Result<Vec<RunEvent>, Said> {
        match self.harness.when(command).result() {
            Ok(events) => Ok(events),
            Err(Failure::Rejected(rejected)) => Err(said(&rejected)),
            Err(other) => panic!("not a rejection: {other:?}"),
        }
    }

    /// A fact, delivered as the listener delivers one: a Fact body with its
    /// derived `external_id`.
    fn fact(&mut self, fact: impl DomainMessage, external_id: &str) -> Vec<RunEvent> {
        self.harness
            .when_book(fact_book(fact, external_id))
            .events()
    }

    fn state(&self) -> RunState {
        self.harness.state().state().clone()
    }

    fn event_count(&self) -> usize {
        self.harness.pages().len()
    }
}

/// A fact as the listener delivers one: a Fact body with its derived
/// `external_id`.
fn fact_book(fact: impl DomainMessage, external_id: &str) -> CommandBook {
    let any = Any::from_msg(&fact.to_wire()).unwrap();
    CommandBook {
        cover: Some(cover()),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(Fact { payload: Some(any) })),
            fact_origin: Some(FactOrigin {
                external_id: external_id.to_string(),
                description: String::new(),
            }),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

fn enqueue() -> EnqueueRun {
    EnqueueRun {
        session: SESSION.to_string(),
        incarnation: 1,
        mode: RunMode::Interactive,
        credential_hash: HASH.to_vec(),
    }
}

fn queue_turn() -> QueueTurn {
    QueueTurn {
        prompt_ref: "prompt:1".to_string(),
        resume: false,
    }
}

fn exited(code: i32) -> RunExited {
    RunExited { code, signal: 0 }
}

// NEW-DESIGN (plan 3.3, D29): the event holds the hash and nothing else of
// the credential; the fold keeps it for `identify`.
#[test]
fn enqueue_records_the_session_incarnation_mode_and_the_hash_of_the_token() {
    let mut run = Run::new();
    let events = run.command(enqueue()).unwrap();
    let [RunEvent::RunEnqueued(enqueued)] = events.as_slice() else {
        panic!("one RunEnqueued, got {events:?}");
    };
    assert_eq!(enqueued.session, SESSION);
    assert_eq!(enqueued.incarnation, 1);
    assert_eq!(enqueued.credential_hash, HASH.to_vec());
    let state = run.state();
    assert_eq!(state.credential_hash, HASH.to_vec());
    assert!(!state.credential_revoked);
}

// NEW-DESIGN (D29: "only its SHA-256 goes in the event").
#[test]
fn enqueue_refuses_a_credential_hash_that_is_not_a_sha256_digest() {
    let mut run = Run::new();
    let short = EnqueueRun {
        credential_hash: vec![1, 2, 3],
        ..enqueue()
    };
    let (code, args) = run.command(short).unwrap_err();
    assert_eq!(code, "runs-credential-hash-invalid");
    assert_eq!(args["length"], "3");
    assert_eq!(run.event_count(), 0);
}

// NEW-DESIGN (plan 3.3: a run is enqueued once).
#[test]
fn enqueue_on_a_stream_with_history_is_refused() {
    let mut run = Run::enqueued();
    let (code, _) = run.command(enqueue()).unwrap_err();
    assert_eq!(code, "runs-already-enqueued");
    assert_eq!(run.event_count(), 1);
}

// NEW-DESIGN (plan 3.3: root = UUIDv5(session, incarnation)): an enqueue
// whose session and incarnation derive another root is refused, so a stream
// can never hold a run that is not its own.
#[test]
fn enqueue_on_a_stream_that_is_not_the_runs_own_root_is_refused() {
    let mut run = Run::new();
    let wrong = EnqueueRun {
        incarnation: 2,
        ..enqueue()
    };
    let (code, _) = run.command(wrong).unwrap_err();
    assert_eq!(code, "runs-not-its-stream");
}

// NEW-DESIGN (plan 3.3 / 4.4: "a run with a turn in flight refuses
// QueueTurn"). Go's turn gate; the Go test is not traced, so it needs review.
#[test]
fn a_turn_is_refused_while_another_is_in_flight() {
    let mut run = Run::enqueued();
    run.command(queue_turn()).unwrap();
    let (code, args) = run.command(queue_turn()).unwrap_err();
    assert_eq!(code, "runs-turn-in-flight");
    // The turn is the sequence of its TurnQueued event (plan 4.3).
    assert_eq!(args["turn"], "1");
}

// NEW-DESIGN (plan 4.3: `<turn>` is the sequence of the TurnQueued event in
// the run's stream: a per-stream position, the cause).
#[test]
fn the_turn_in_flight_is_the_sequence_of_its_turn_queued_event() {
    let mut run = Run::enqueued();
    run.command(queue_turn()).unwrap();
    assert_eq!(run.state().turn_in_flight, Some(1));
}

// NEW-DESIGN (D5 Q7, settled by the owner: one run per incarnation, not per
// turn; plan 3.3 over Go's `sessions.MaxRunEnds` comment "a run per turn"):
// two turns of one runner are one RunEnqueued and one RunEnded.
#[test]
fn two_turns_of_one_runner_are_one_run_enqueued_and_one_run_ended() {
    let mut run = Run::enqueued();
    run.command(queue_turn()).unwrap();
    run.fact(TurnStarted { turn: 1 }, "turn:r:1:started");
    run.fact(
        TurnCompleted {
            turn: 1,
            native_key: "k".to_string(),
            answer_ref: String::new(),
            exit_code: 0,
        },
        "turn:r:1:completed",
    );
    run.command(queue_turn()).unwrap();
    run.fact(TurnStarted { turn: 3 }, "turn:r:3:started");
    run.fact(
        TurnCompleted {
            turn: 3,
            native_key: "k".to_string(),
            answer_ref: String::new(),
            exit_code: 0,
        },
        "turn:r:3:completed",
    );
    run.fact(exited(0), "exit:r");
    let all: Vec<RunEvent> = run
        .harness
        .pages()
        .iter()
        .map(|page| RunEvent::from_any(page.payload.as_ref().unwrap()).unwrap())
        .collect();
    let enqueued = all
        .iter()
        .filter(|e| matches!(e, RunEvent::RunEnqueued(_)))
        .count();
    let ended = all
        .iter()
        .filter(|e| matches!(e, RunEvent::RunEnded(_)))
        .count();
    assert_eq!((enqueued, ended), (1, 1));
}

// NEW-DESIGN (plan 3.3: TurnCompleted, then RunIdle): a completed turn frees
// the gate, so the next one queues.
#[test]
fn a_completed_turn_frees_the_gate() {
    let mut run = Run::enqueued();
    run.command(queue_turn()).unwrap();
    run.fact(
        TurnCompleted {
            turn: 1,
            native_key: String::new(),
            answer_ref: String::new(),
            exit_code: 0,
        },
        "turn:r:1:completed",
    );
    assert_eq!(run.state().turn_in_flight, None);
    run.command(queue_turn()).unwrap();
}

// NEW-DESIGN (plan 3.3: TurnFailed records the class; the policy run->hold
// reads it).
#[test]
fn a_failed_turn_records_its_class_and_frees_the_gate() {
    let mut run = Run::enqueued();
    run.command(queue_turn()).unwrap();
    let events = run.fact(
        TurnFailed {
            turn: 1,
            class: TurnFailureClass::RateLimited,
            detail: "429".to_string(),
        },
        "turn:r:1:failed",
    );
    assert_eq!(events.len(), 1);
    assert_eq!(run.state().turn_in_flight, None);
}

// PORTED-FROM-GO: internal/core/coord/endfence_test.go
// TestEndFence_MailForAnEndedRunWaitsForTheResumedRun. After the run's end
// there is no courier, and the run refuses a turn: the end fence (plan 3.4).
#[test]
fn a_turn_queued_after_the_runs_end_is_refused() {
    let mut run = Run::enqueued();
    run.fact(exited(0), "exit:r");
    let (code, _) = run.command(queue_turn()).unwrap_err();
    assert_eq!(code, "runs-ended");
}

// NEW-DESIGN (plan 3.3: "RunEnded, exactly once: a second terminal input
// records nothing").
#[test]
fn the_first_terminal_input_ends_the_run_and_a_second_records_nothing() {
    let mut run = Run::enqueued();
    let first = run.fact(exited(0), "exit:r");
    assert!(matches!(
        first.as_slice(),
        [RunEvent::RunEnded(RunEnded {
            cause: EndCause::RunnerExit,
            ..
        })]
    ));
    let before = run.event_count();
    let second = run.fact(
        LaunchFailed {
            deterministic: true,
            detail: String::new(),
        },
        "launch:r:1",
    );
    assert!(second.is_empty());
    assert_eq!(run.event_count(), before);
}

// NEW-DESIGN (plan 4.3: a re-sent fact with the same external_id is
// DUPLICATE; the kernel dedupes by `fact_command_id(external_id)`).
#[test]
fn the_same_fact_again_is_a_duplicate() {
    let mut run = Run::enqueued();
    run.fact(exited(0), "exit:r");
    let book = fact_book(exited(0), "exit:r");
    let handled = run.harness.when_book(book).handled().unwrap();
    assert!(matches!(handled, Handled::Duplicate(..)));
}

// PORTED-FROM-GO: internal/core/coord/creds_test.go
// TestCredentialRevocation_RevokesTheChildsCredential, extended per D29: the
// end revokes in the fold, whatever the cause.
#[test]
fn every_end_revokes_the_credential_whatever_the_cause() {
    let exit = |run: &mut Run| {
        run.fact(exited(1), "exit:r");
    };
    let launch_failed = |run: &mut Run| {
        run.fact(
            LaunchFailed {
                deterministic: false,
                detail: "boom".to_string(),
            },
            "launch:r:1",
        );
    };
    for end in [&exit as &dyn Fn(&mut Run), &launch_failed] {
        let mut run = Run::enqueued();
        assert!(!run.state().credential_revoked);
        end(&mut run);
        assert!(run.state().credential_revoked);
    }
}

// PORTED-FROM-GO: internal/core/coord/session_owner_revoke_test.go
// TestRevokeSessionOwner_RevocationSurvivesAdoption: the revocation is a
// fact in the stream, so a fresh process replaying it still sees the token
// revoked (`Harness::state` replays from the store, as the next process does).
#[test]
fn a_revocation_survives_a_restart() {
    let mut run = Run::enqueued();
    run.fact(exited(0), "exit:r");
    let replayed = run.harness.state();
    assert!(replayed.state().credential_revoked);
}

// NEW-DESIGN (plan 3.3: RequestStop from the policy session->run; the run
// ends later by RunExited or StopGraceElapsed, not by the request).
#[test]
fn a_stop_request_is_recorded_once_and_does_not_end_the_run() {
    let mut run = Run::enqueued();
    let stop = RequestStop {
        reason: "user".to_string(),
    };
    assert_eq!(run.command(stop.clone()).unwrap().len(), 1);
    assert!(run.command(stop).unwrap().is_empty());
    let state = run.state();
    assert!(state.stop_requested);
    assert_eq!(state.ended, None);
}

// NEW-DESIGN: a runner attaches once per id; the attach is a fact.
#[test]
fn a_runner_attaching_is_recorded_on_the_run() {
    let mut run = Run::enqueued();
    let events = run.fact(
        RunnerAttached {
            runner_id: "r1".to_string(),
            protocol: "v1".to_string(),
        },
        "attach:r:r1",
    );
    assert_eq!(events.len(), 1);
    assert!(run.state().runner_attached);
}
