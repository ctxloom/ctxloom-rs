//! The session's run lifeline (plan 3.1): which incarnation it owes, whether
//! one is open, the relaunch budget, the stop flag. RED (S5): `lifeline.rs`
//! holds stubs. Each test is PORTED-FROM-GO (naming the Go test) or
//! NEW-DESIGN.

use crate::aggregate::tests::{mint, Session};
use crate::{ClaimMessage, DeliverMessage, MintSession, RecordMessageArrived};
use crate::{
    IncarnationEnded, IncarnationOwed, RecordIncarnationEnded, SessionEvent, StopOrdered,
    StopSession, RELAUNCH_BUDGET,
};

fn ended(incarnation: u32, cause: &str) -> RecordIncarnationEnded {
    RecordIncarnationEnded {
        incarnation,
        cause: cause.to_string(),
    }
}

fn arrive(id: &str) -> RecordMessageArrived {
    RecordMessageArrived {
        message_id: id.to_string(),
        from: "peer".to_string(),
        kind: "note".to_string(),
        body_ref: format!("body:{id}"),
    }
}

/// A session (its first incarnation owed by the mint) whose run ended by `cause`.
fn first_run_ended(cause: &str) -> Session {
    let mut session = Session::minted();
    session.fact(ended(1, cause), "end:s:1");
    session
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q1 (b)): the mint emits
// [SessionMinted, IncarnationOwed{1}] (plan 3.1: "a new session then owes its
// first incarnation"); this supersedes the separate OweIncarnation command.
#[test]
fn a_mint_records_the_session_and_owes_its_first_incarnation() {
    let mut session = Session::new();
    let events = session.run(mint(crate::aggregate::tests::HARP)).unwrap();
    assert!(matches!(events[0], SessionEvent::SessionMinted(_)));
    assert_eq!(
        events[1],
        SessionEvent::IncarnationOwed(IncarnationOwed {
            incarnation: 1,
            reason: "first".to_string()
        })
    );
    assert_eq!(events.len(), 2);
    let state = session.state();
    assert_eq!((state.incarnation, state.open_incarnation), (1, Some(1)));
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q2 (b)): minimal lineage, the
// session's depth, is recorded at the mint. Parent and may_delegate are S10's.
#[test]
fn a_mint_records_the_sessions_depth() {
    let mut session = Session::new();
    session
        .run(MintSession {
            depth: 2,
            ..mint(crate::aggregate::tests::HARP)
        })
        .unwrap();
    assert_eq!(session.state().depth, 2);
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q1): "incarnation owed on
// mail for an ended, unstopped recipient", by CONTINUATION: the session's own
// handling of MessageArrived records IncarnationOwed, like a relaunch.
// Go: TestEndFence_MailForAnEndedRunWaitsForTheResumedRun (the mail waits for
// the resumed run).
#[test]
fn mail_for_an_ended_unstopped_session_owes_the_next_incarnation() {
    let mut session = first_run_ended("runner-exit");
    let events = session.fact(arrive("m1"), "arrive:s:m1");
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], SessionEvent::MessageArrived(_)));
    assert_eq!(
        events[1],
        SessionEvent::IncarnationOwed(IncarnationOwed {
            incarnation: 2,
            reason: "mail".to_string()
        })
    );
}

// NEW-DESIGN (same ruling): while an incarnation is open the mail just queues,
// and after a stop it is queued without owing a run.
#[test]
fn mail_owes_nothing_while_a_run_is_open_or_after_a_stop() {
    let mut open = Session::minted();
    assert_eq!(open.fact(arrive("m1"), "arrive:s:m1").len(), 1);

    let mut stopped = first_run_ended("runner-exit");
    stopped
        .run(StopSession {
            requester_depth: 0,
            reason: "user".to_string(),
        })
        .unwrap();
    assert_eq!(stopped.fact(arrive("m1"), "arrive:s:m1").len(), 1);
}

// NEW-DESIGN (plan 3.1): the session decides by level. Mail pending, budget
// left, not stopped, cause not `stopped`: it relaunches. Go's gate:
// TestTerminateRun_LeftoverMailRelaunchesAndDeliversIt
// (coordinator_drain_bound_test.go) is the nearest Go scenario; the fold is new.
#[test]
fn an_end_with_mail_pending_owes_the_next_incarnation() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    let events = session.fact(ended(1, "runner-exit"), "end:s:1");
    assert_eq!(
        events,
        vec![
            SessionEvent::IncarnationEnded(IncarnationEnded {
                incarnation: 1,
                cause: "runner-exit".to_string(),
                notice_parent: false,
            }),
            SessionEvent::IncarnationOwed(IncarnationOwed {
                incarnation: 2,
                reason: "relaunch".to_string(),
            }),
        ]
    );
    assert_eq!(session.state().open_incarnation, Some(2));
}

// NEW-DESIGN (plan 3.1): no mail pending: give up and notice the parent.
#[test]
fn an_end_with_no_mail_pending_gives_up_and_notices_the_parent() {
    let mut session = Session::minted();
    let events = session.fact(ended(1, "runner-exit"), "end:s:1");
    assert_eq!(
        events,
        vec![SessionEvent::IncarnationEnded(IncarnationEnded {
            incarnation: 1,
            cause: "runner-exit".to_string(),
            notice_parent: true,
        })]
    );
    assert_eq!(session.state().open_incarnation, None);
}

// PORTED-FROM-GO: internal/core/coord/coordinator_drain_bound_test.go
// TestBeginDrain_ChildDyingDuringDrainIsNotRelaunched: a run that ended
// `stopped` is never relaunched, mail or no mail.
#[test]
fn a_stopped_end_is_never_relaunched_even_with_mail_pending() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    let events = session.fact(ended(1, "stopped"), "end:s:1");
    assert_eq!(events.len(), 1);
    assert_eq!(session.state().open_incarnation, None);
}

// PORTED-FROM-GO: internal/core/coord/launchgate_giveup_test.go
// TestAgentStop_OnAnEndedRunStillStopsTheRelaunch: a stop order sets the flag
// even after the run ended, and the relaunch it would have owed is not.
#[test]
fn a_stop_ordered_after_the_end_still_stops_the_relaunch() {
    let mut session = first_run_ended("runner-exit");
    let events = session
        .run(StopSession {
            requester_depth: 0,
            reason: "user".to_string(),
        })
        .unwrap();
    assert_eq!(
        events,
        vec![SessionEvent::StopOrdered(StopOrdered {
            reason: "user".to_string()
        })]
    );
    assert!(session.state().stopped);
}

// NEW-DESIGN (plan 3.1): a stopped session does not relaunch on an end.
#[test]
fn a_stopped_session_does_not_relaunch_on_an_end() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session
        .run(StopSession {
            requester_depth: 0,
            reason: "user".to_string(),
        })
        .unwrap();
    let events = session.fact(ended(1, "runner-exit"), "end:s:1");
    assert_eq!(events.len(), 1);
}

// NEW-DESIGN (plan 3.1, Go's I2: "refused from a child"). The depth is
// STAMPED from the caller's identity by admission, never read from the
// payload; the aggregate refuses depth > 0.
#[test]
fn a_stop_ordered_by_a_child_is_refused() {
    let mut session = Session::minted();
    let (code, args) = session
        .run(StopSession {
            requester_depth: 2,
            reason: "x".to_string(),
        })
        .unwrap_err();
    assert_eq!(code, "sessions-child-cannot-stop");
    assert_eq!(args["depth"], "2");
    assert!(!session.state().stopped);
}

// NEW-DESIGN: a second stop order records nothing.
#[test]
fn a_second_stop_order_records_nothing() {
    let mut session = Session::minted();
    let stop = StopSession {
        requester_depth: 0,
        reason: "u".to_string(),
    };
    assert_eq!(session.run(stop.clone()).unwrap().len(), 1);
    assert!(session.run(stop).unwrap().is_empty());
}

// PORTED-FROM-GO: internal/core/coord/relaunch_budget_test.go
// TestRelaunchBudget_BoundsAttachThenDie: a run that comes up and dies with
// mail still pending exhausts the budget (an attach is NOT progress), so the
// relaunch loop is bounded, then the session gives up and notices the parent.
#[test]
fn attach_then_die_with_mail_pending_exhausts_the_relaunch_budget() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    let mut relaunches = 0;
    for incarnation in 1..=200 {
        let events = session.fact(
            ended(incarnation, "runner-exit"),
            &format!("end:s:{incarnation}"),
        );
        if events
            .iter()
            .any(|e| matches!(e, SessionEvent::IncarnationOwed(_)))
        {
            relaunches += 1;
        } else {
            break;
        }
    }
    assert!(relaunches > 0, "the budget allows some relaunches");
    assert!(relaunches <= RELAUNCH_BUDGET, "{relaunches} relaunches");
    assert!(session.state().open_incarnation.is_none());
}

// PORTED-FROM-GO: internal/core/coord/relaunch_budget_test.go
// TestRelaunchBudget_ResetByMailConsumption: a delivery is progress and earns
// a fresh budget.
#[test]
fn consuming_mail_resets_the_relaunch_budget() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session.fact(ended(1, "runner-exit"), "end:s:1");
    assert_eq!(session.state().relaunches, 1);
    session.fact(
        ClaimMessage {
            message_id: "m1".to_string(),
            claimant: "runner".to_string(),
        },
        "claim:s:m1:runner",
    );
    session.fact(
        DeliverMessage {
            message_id: "m1".to_string(),
            at_turn: 1,
        },
        "deliver:s:m1",
    );
    assert_eq!(session.state().relaunches, 0);
}

// NEW-DESIGN: an end told twice (a retried policy step) records nothing the
// second time; the kernel answers it DUPLICATE by its external_id, and a
// different external_id for an already-ended incarnation is a no-op by level.
#[test]
fn an_end_for_an_incarnation_already_ended_records_nothing() {
    let mut session = first_run_ended("runner-exit");
    let events = session.fact(ended(1, "runner-exit"), "end:s:1:again");
    assert!(events.is_empty());
}
