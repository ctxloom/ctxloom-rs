//! The inbound fact port (plan 4.2, 4.3). RED (S5): stubs.

use std::sync::Arc;

use run_ports::Edge;

use angzarr_embedded::v1::{DeliveryOutcome, Uuid};

use crate::fakes::{fact_book, uuid, World, SESSION_ME};
use crate::{AgentFact, Facts, Identity, Recorded};

fn identity() -> Identity {
    Identity {
        run: Uuid::from_bytes(
            *uuid::Uuid::parse_str("a1a1a1a1-0000-5000-8000-000000000001")
                .unwrap()
                .as_bytes(),
        ),
        session: uuid(SESSION_ME),
        incarnation: 1,
        harp: "amber-swift-owl".to_string(),
        depth: 0,
    }
}

const RUN: &str = "a1a1a1a1-0000-5000-8000-000000000001";

fn turn_started(turn: u64) -> AgentFact {
    AgentFact::TurnStarted(runs::TurnStarted { turn })
}

fn exited() -> AgentFact {
    AgentFact::RunExited(runs::RunExited { code: 0, signal: 0 })
}

fn delivered(id: &str) -> AgentFact {
    AgentFact::MessageDelivered(sessions::DeliverMessage {
        message_id: id.to_string(),
        at_turn: 1,
    })
}

// NEW-DESIGN (plan 4.3 table): the external_id is derived from the fact's
// cause, never from a counter, so a retry after any crash rebuilds the same id.
#[test]
fn turn_facts_are_identified_by_run_and_turn() {
    let who = identity();
    let completed = AgentFact::TurnCompleted(runs::TurnCompleted {
        turn: 4,
        native_key: String::new(),
        answer_ref: String::new(),
        exit_code: 0,
    });
    let failed = AgentFact::TurnFailed(runs::TurnFailed {
        turn: 4,
        class: runs::TurnFailureClass::EngineError,
        detail: String::new(),
    });
    assert_eq!(
        turn_started(4).external_id(&who),
        format!("turn:{RUN}:4:started")
    );
    assert_eq!(
        completed.external_id(&who),
        format!("turn:{RUN}:4:completed")
    );
    assert_eq!(failed.external_id(&who), format!("turn:{RUN}:4:failed"));
}

// NEW-DESIGN (plan 4.3: `exit:<run>`, one terminal; `attach` is this plan's
// addition, review question 9).
#[test]
fn the_exit_and_the_attach_are_identified_by_the_run() {
    let who = identity();
    assert_eq!(exited().external_id(&who), format!("exit:{RUN}"));
    let attached = AgentFact::RunnerAttached(runs::RunnerAttached {
        runner_id: "r1".to_string(),
        protocol: "v1".to_string(),
    });
    assert_eq!(attached.external_id(&who), format!("attach:{RUN}:r1"));
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review item 8; plan 4.3 table
// `bind:<session>:<native_id>`): the engine session bind is an AgentFact on the
// caller's own session. The hook-cli `session-bind` hook reaches it through
// the sessions use case, whose book is `sessions::bind_book` (hook-cli's own
// code needs no change; its tests stay as they are).
#[test]
fn the_engine_session_bind_is_identified_by_session_and_native_id() {
    let who = identity();
    let bound = AgentFact::EngineSessionBound(sessions::BindEngineSession {
        native_id: "native-1".to_string(),
        transcript_path: "/t".to_string(),
        engine_version: None,
    });
    assert_eq!(
        bound.external_id(&who),
        format!("bind:{SESSION_ME}:native-1")
    );
    assert_eq!(bound.cover(&who), who.session_cover());
}

// NEW-DESIGN (plan 4.3: the stable session id, not the harp, D5 Q1).
#[test]
fn inbox_facts_are_identified_by_session_and_message() {
    let who = identity();
    let claimed = AgentFact::MessageClaimed(sessions::ClaimMessage {
        message_id: "m7".to_string(),
        claimant: "runner".to_string(),
    });
    let failed = AgentFact::MessageFailed(sessions::FailMessage {
        message_id: "m7".to_string(),
        reason: "engine".to_string(),
    });
    assert_eq!(
        delivered("m7").external_id(&who),
        format!("deliver:{SESSION_ME}:m7")
    );
    assert_eq!(
        claimed.external_id(&who),
        format!("claim:{SESSION_ME}:m7:runner")
    );
    assert_eq!(failed.external_id(&who), format!("fail:{SESSION_ME}:m7"));
}

// NEW-DESIGN (plan 4.2 narrowness 3): a fact may address only the caller's own
// run, or its own session (inbox included): the stream is chosen by the
// variant and the identity, never by the payload.
#[test]
fn a_fact_addresses_only_the_callers_own_run_or_session() {
    let who = identity();
    for fact in AgentFact::samples() {
        let cover = fact.cover(&who);
        assert!(
            cover == who.run_cover() || cover == who.session_cover(),
            "{} addresses {cover:?}",
            fact.type_url()
        );
    }
    assert_eq!(turn_started(1).cover(&who), who.run_cover());
    assert_eq!(delivered("m").cover(&who), who.session_cover());
}

// NEW-DESIGN (plan 4.2 narrowness 1 and 4): the book is a Fact body, with its
// origin, and never a Command body.
#[test]
fn a_fact_is_sent_as_a_fact_body_with_its_external_id() {
    use angzarr_embedded::v1::command_page::Body;
    let who = identity();
    for fact in AgentFact::samples() {
        let book = fact.book(&who);
        let page = book.page.expect("a page");
        assert!(
            matches!(page.body, Some(Body::Fact(_))),
            "{}",
            fact.type_url()
        );
        assert_eq!(
            page.fact_origin.expect("an origin").external_id,
            fact.external_id(&who)
        );
    }
}

// PORTED-FROM-GO: S5 plan red test "a fact on the caller's own run is APPLIED,
// and the same fact again is DUPLICATE" (Go: facts are journaled once by id,
// internal/core/coord/facts.go).
#[tokio::test]
async fn a_fact_on_the_callers_own_run_is_applied_and_again_is_duplicate() {
    let world = World::new().await;
    let facts = Facts::new(Arc::clone(&world.edge));
    let who = &world.me.identity;
    assert_eq!(facts.record(who, exited()).await, Ok(Recorded::Applied));
    let before = world.edge.pages(&who.run_cover()).await.len();
    assert_eq!(facts.record(who, exited()).await, Ok(Recorded::Duplicate));
    assert_eq!(world.edge.pages(&who.run_cover()).await.len(), before);
}

// NEW-DESIGN: an inbox fact lands on the session's stream, not the run's. The
// mail arrived first (a fact of the routing policy, as the fixture sends it).
#[tokio::test]
async fn an_inbox_fact_lands_on_the_callers_session_and_not_its_run() {
    let world = World::new().await;
    let facts = Facts::new(Arc::clone(&world.edge));
    let who = &world.me.identity;
    world
        .edge
        .handle(fact_book(
            who.session_cover(),
            sessions::RecordMessageArrived {
                message_id: "m1".to_string(),
                from: "peer".to_string(),
                kind: "note".to_string(),
                body_ref: "body:m1".to_string(),
            },
            "arrive:fixture:m1",
        ))
        .await
        .unwrap();
    let session_before = world.edge.pages(&who.session_cover()).await.len();
    let run_before = world.edge.pages(&who.run_cover()).await.len();
    assert_eq!(
        facts.record(who, delivered("m1")).await,
        Ok(Recorded::Applied)
    );
    assert_eq!(
        world.edge.pages(&who.session_cover()).await.len(),
        session_before + 1
    );
    assert_eq!(world.edge.pages(&who.run_cover()).await.len(), run_before);
}

// PORTED-FROM-GO: S5 plan red test "a fact at the race cap is PARKED, and the
// client stops retrying it" (spec DeliveryOutcome PARKED; plan 4.3 "At the
// cap (D13)"): the fact loses every race, the answer is Parked, nothing is
// persisted, and the port asked the kernel exactly once.
#[tokio::test]
async fn a_fact_at_the_race_cap_is_parked_and_asked_once() {
    let world = World::new().await;
    let facts = Facts::new(Arc::clone(&world.edge));
    let who = &world.me.identity;
    let before = world.edge.pages(&who.run_cover()).await.len();
    world.edge.lose_every_race();
    let served = world.edge.served();
    assert_eq!(
        facts.record(who, turn_started(1)).await,
        Ok(Recorded::Parked)
    );
    assert_eq!(world.edge.served(), served + 1, "no retry of a parked fact");
    assert_eq!(world.edge.pages(&who.run_cover()).await.len(), before);
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q5, keep + re-pin to
// angzarr-embedded ffc58aa, which contains 7873207: "a fact answered with a
// rejection is dead-lettered"). The kernel, not this crate, dead-letters a fact
// it cannot take: the answer is `Parked`, nothing is persisted on the stream,
// and the kernel's own letter about the run's stream is readable from the
// store (`DeadLetterStore`). A markup `#[handles_fact]` cannot return a
// rejection, so the case is provoked the way it can happen, a write that loses
// every race; the kernel treats a bare handler's rejection the same way.
// Replaces the earlier test that expected `Err(Refused)` and a `DeadLetters` port.
#[tokio::test]
async fn a_fact_the_kernel_dead_letters_is_parked_and_its_letter_is_in_the_store() {
    let world = World::new().await;
    let facts = Facts::new(Arc::clone(&world.edge));
    let who = &world.me.identity;
    assert!(world.edge.dead_letters(&who.run_cover()).await.is_empty());
    world.edge.lose_every_race();
    assert_eq!(
        facts.record(who, turn_started(1)).await,
        Ok(Recorded::Parked)
    );
    let letters = world.edge.dead_letters(&who.run_cover()).await;
    assert_eq!(letters.len(), 1, "the kernel's own dead letter");
    assert_eq!(letters[0].stream, who.run_cover());
}

// NEW-DESIGN: the answer maps the spec's three outcomes and no others.
#[test]
fn recorded_is_the_specs_delivery_outcome() {
    let named = |outcome: DeliveryOutcome| match outcome {
        DeliveryOutcome::Applied => Recorded::Applied,
        DeliveryOutcome::Duplicate => Recorded::Duplicate,
        DeliveryOutcome::Parked => Recorded::Parked,
        DeliveryOutcome::Unspecified => panic!("unspecified is not an answer"),
    };
    assert_eq!(named(DeliveryOutcome::Parked), Recorded::Parked);
}
