//! The session's inbox (the former mailbox, D5 Q4). RED (S5): `inbox.rs`
//! holds stubs. Each test is PORTED-FROM-GO or NEW-DESIGN.

use crate::aggregate::tests::Session;
use crate::{
    ClaimMessage, DeliverMessage, FailMessage, InboxStatus, MessageArrived, RecordMessageArrived,
    SessionEvent, WithdrawSteer,
};

fn arrive(id: &str) -> RecordMessageArrived {
    RecordMessageArrived {
        message_id: id.to_string(),
        from: "peer".to_string(),
        kind: "note".to_string(),
        body_ref: format!("body:{id}"),
    }
}

fn claim(id: &str, claimant: &str) -> ClaimMessage {
    ClaimMessage {
        message_id: id.to_string(),
        claimant: claimant.to_string(),
    }
}

fn deliver(id: &str, turn: u64) -> DeliverMessage {
    DeliverMessage {
        message_id: id.to_string(),
        at_turn: turn,
    }
}

// NEW-DESIGN (plan 3.1: MessageArrived is a fact recorded on the recipient's
// session by the policy that routes a send).
#[test]
fn mail_arriving_is_queued_in_the_inbox() {
    let mut session = Session::minted();
    let events = session.fact(arrive("m1"), "arrive:s:m1");
    assert_eq!(
        events,
        vec![SessionEvent::MessageArrived(MessageArrived {
            message_id: "m1".to_string(),
            from: "peer".to_string(),
            kind: "note".to_string(),
            body_ref: "body:m1".to_string(),
        })]
    );
    let inbox = session.state().inbox;
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].status, InboxStatus::Queued);
}

// NEW-DESIGN: claim, then deliver, moves the entry along; the claimant is kept.
#[test]
fn a_message_is_claimed_then_delivered_at_a_turn() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session.fact(claim("m1", "runner"), "claim:s:m1:runner");
    assert_eq!(session.state().inbox[0].status, InboxStatus::Claimed);
    assert_eq!(session.state().inbox[0].claimant, "runner");
    session.fact(deliver("m1", 3), "deliver:s:m1");
    let entry = &session.state().inbox[0];
    assert_eq!(
        (entry.status, entry.at_turn),
        (InboxStatus::Delivered, Some(3))
    );
}

// PORTED-FROM-GO: internal/core/coord/creds_test.go
// TestMailbox_AtLeastOnceRedelivery: a message the runner delivers twice (its
// first report lost) is delivered once; the second delivery records nothing.
#[test]
fn a_message_delivered_twice_is_delivered_once() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    assert_eq!(session.fact(deliver("m1", 1), "deliver:s:m1").len(), 1);
    // A different external_id (a retry from another turn's cause) is still a
    // no-op by level.
    assert!(session
        .fact(deliver("m1", 2), "deliver:s:m1:retry")
        .is_empty());
    assert_eq!(session.state().inbox[0].at_turn, Some(1));
}

// NEW-DESIGN: a claim by a second claimant records nothing (the first wins).
#[test]
fn a_second_claimant_does_not_take_a_claimed_message() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session.fact(claim("m1", "runner"), "claim:s:m1:runner");
    assert!(session
        .fact(claim("m1", "hook"), "claim:s:m1:hook")
        .is_empty());
    assert_eq!(session.state().inbox[0].claimant, "runner");
}

// NEW-DESIGN (plan 3.1: MessageFailed, Go's `failed/`).
#[test]
fn a_failed_message_is_marked_failed() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session.fact(
        FailMessage {
            message_id: "m1".to_string(),
            reason: "engine".to_string(),
        },
        "fail:s:m1",
    );
    assert_eq!(session.state().inbox[0].status, InboxStatus::Failed);
}

// NEW-DESIGN (plan 3.1: WithdrawSteer, refused `sessions-already-delivered`).
#[test]
fn a_queued_steer_can_be_withdrawn_but_a_delivered_one_cannot() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session.fact(arrive("m2"), "arrive:s:m2");
    session
        .run(WithdrawSteer {
            message_id: "m1".to_string(),
        })
        .unwrap();
    assert_eq!(session.state().inbox[0].status, InboxStatus::Withdrawn);
    session.fact(deliver("m2", 1), "deliver:s:m2");
    let (code, args) = session
        .run(WithdrawSteer {
            message_id: "m2".to_string(),
        })
        .unwrap_err();
    assert_eq!(code, "sessions-already-delivered");
    assert_eq!(args["message_id"], "m2");
}

// NEW-DESIGN: withdrawing a message the inbox never held is refused.
#[test]
fn withdrawing_an_unknown_message_is_refused() {
    let mut session = Session::minted();
    let (code, _) = session
        .run(WithdrawSteer {
            message_id: "nope".to_string(),
        })
        .unwrap_err();
    assert_eq!(code, "sessions-unknown-message");
}

// NEW-DESIGN: mail that arrives twice under one message id queues once (the
// recipient's session is the dedupe point; arrival is at-least-once).
#[test]
fn mail_arriving_twice_under_one_id_queues_once() {
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    assert!(session.fact(arrive("m1"), "arrive:s:m1:again").is_empty());
    assert_eq!(session.state().inbox.len(), 1);
}

// NEW-DESIGN: mail already settled is settled: a failure of delivered mail, or
// a delivery of failed mail, records nothing, and neither does mail unknown.
#[test]
fn settled_mail_is_not_failed_or_delivered_again() {
    let fail = |id: &str| FailMessage {
        message_id: id.to_string(),
        reason: "x".to_string(),
    };
    let mut session = Session::minted();
    session.fact(arrive("m1"), "arrive:s:m1");
    session.fact(arrive("m2"), "arrive:s:m2");
    session.fact(deliver("m1", 1), "deliver:s:m1");
    assert!(session.fact(fail("m1"), "fail:s:m1").is_empty());
    session.fact(fail("m2"), "fail:s:m2");
    assert!(session.fact(deliver("m2", 2), "deliver:s:m2").is_empty());
    assert!(session.fact(fail("nope"), "fail:s:nope").is_empty());
    assert!(session
        .fact(deliver("nope", 1), "deliver:s:nope")
        .is_empty());
    assert!(session
        .fact(claim("nope", "runner"), "claim:s:nope:runner")
        .is_empty());
}
