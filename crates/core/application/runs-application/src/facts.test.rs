//! The inbound fact port (plan 4.2, 4.3). RED (S5): stubs.

use std::sync::Arc;

use run_ports::Edge;

use angzarr_embedded::v1::{DeliveryOutcome, Uuid};

use crate::fakes::{fact_book, uuid, World, SESSION_ME};
use crate::{AppError, Facts, Identity, ModelFact, Principal, Recorded, RunnerFact};

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
        principal: Principal::Runner,
    }
}

const RUN: &str = "a1a1a1a1-0000-5000-8000-000000000001";

fn turn_started(turn: u64) -> RunnerFact {
    RunnerFact::TurnStarted(runs::TurnStarted { turn })
}

fn exited() -> RunnerFact {
    RunnerFact::RunExited(runs::RunExited { code: 0, signal: 0 })
}

fn delivered(id: &str) -> RunnerFact {
    RunnerFact::MessageDelivered(sessions::DeliverMessage {
        message_id: id.to_string(),
        at_turn: 1,
    })
}

// NEW-DESIGN (plan 4.3 table): the external_id is derived from the fact's
// cause, never from a counter, so a retry after any crash rebuilds the same id.
#[test]
fn turn_facts_are_identified_by_run_and_turn() {
    let who = identity();
    let completed = RunnerFact::TurnCompleted(runs::TurnCompleted {
        turn: 4,
        native_key: String::new(),
        answer_ref: String::new(),
        exit_code: 0,
    });
    let failed = RunnerFact::TurnFailed(runs::TurnFailed {
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
    let attached = RunnerFact::RunnerAttached(runs::RunnerAttached {
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
    let bound = RunnerFact::EngineSessionBound(sessions::BindEngineSession {
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

// NEW-DESIGN, owner ruling 2026-11-04 (item 16): a claim's external id is
// derived by the RUNNER side from the run and turn (and, for a progress report,
// the ordinal it stamps), never from a counter the model supplies, and it
// addresses the caller's own run.
#[test]
fn claims_are_identified_by_run_and_turn() {
    let who = Identity {
        principal: Principal::Model,
        ..identity()
    };
    let summary = ModelFact::SummaryReported(runs::SummaryReported {
        turn: 3,
        ordinal: 2,
        scope: "progress".to_string(),
        text_ref: "t".to_string(),
    });
    let done = ModelFact::FinalReported(runs::FinalReported {
        turn: 3,
        text_ref: "t".to_string(),
    });
    assert_eq!(summary.external_id(&who), format!("report:{RUN}:3:2"));
    assert_eq!(done.external_id(&who), format!("final:{RUN}:3"));
    assert_eq!(summary.cover(&who), who.run_cover());
    assert_eq!(done.cover(&who), who.run_cover());
}

// NEW-DESIGN (plan 4.3: the stable session id, not the harp, D5 Q1).
#[test]
fn inbox_facts_are_identified_by_session_and_message() {
    let who = identity();
    let claimed = RunnerFact::MessageClaimed(sessions::ClaimMessage {
        message_id: "m7".to_string(),
        claimant: "runner".to_string(),
    });
    let failed = RunnerFact::MessageFailed(sessions::FailMessage {
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
    let covers = RunnerFact::samples()
        .iter()
        .map(|f| (f.type_url(), f.cover(&who)))
        .chain(
            ModelFact::samples()
                .iter()
                .map(|f| (f.type_url(), f.cover(&who))),
        )
        .collect::<Vec<_>>();
    for (url, cover) in covers {
        assert!(
            cover == who.run_cover() || cover == who.session_cover(),
            "{url} addresses {cover:?}"
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
    let books = RunnerFact::samples()
        .iter()
        .map(|f| (f.type_url(), f.book(&who), f.external_id(&who)))
        .chain(
            ModelFact::samples()
                .iter()
                .map(|f| (f.type_url(), f.book(&who), f.external_id(&who))),
        )
        .collect::<Vec<_>>();
    for (url, book, external_id) in books {
        let page = book.page.expect("a page");
        assert!(matches!(page.body, Some(Body::Fact(_))), "{url}");
        assert_eq!(
            page.fact_origin.expect("an origin").external_id,
            external_id
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

// NEW-DESIGN, owner ruling 2026-11-04 (item 16, fail closed): a fact is
// recorded only by its author. Every fact, offered by the OTHER principal, is
// hidden, nothing is sent to the kernel and nothing is written.
#[tokio::test]
async fn a_fact_of_the_other_principal_is_denied_and_nothing_is_sent() {
    let world = World::new().await;
    let facts = Facts::new(Arc::clone(&world.edge));
    // Defence in depth: the types keep a model call site from building a runner
    // fact, yet a runner fact offered with the model's identity (and a claim
    // with the runner's) is still hidden, and nothing is sent.
    let runner_facts = RunnerFact::samples();
    let claims = ModelFact::samples();
    for fact in runner_facts {
        let other = &world.me.model_identity;
        denied_and_unwritten(&world, &facts, other, fact).await;
    }
    for fact in claims {
        let other = &world.me.identity;
        denied_and_unwritten(&world, &facts, other, fact).await;
    }
}

/// `fact` offered by `other`, who is not its author: hidden, nothing sent and
/// nothing written to the run or the session.
async fn denied_and_unwritten<F: crate::Fact>(
    world: &World,
    facts: &Facts<crate::fakes::KernelEdge>,
    other: &Identity,
    fact: F,
) {
    let served = world.edge.served();
    let run_before = world.edge.pages(&other.run_cover()).await.len();
    let session_before = world.edge.pages(&other.session_cover()).await.len();
    let book = fact.book(other);
    let denied = facts.record(other, fact).await.unwrap_err();
    assert!(
        matches!(denied, AppError::Denied(run_ports::Unserved::Hidden { .. })),
        "{book:?}"
    );
    assert_eq!(world.edge.served(), served, "{book:?}");
    assert_eq!(world.edge.pages(&other.run_cover()).await.len(), run_before);
    assert_eq!(
        world.edge.pages(&other.session_cover()).await.len(),
        session_before
    );
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16): claims are facts the MODEL
// records, applied once and then DUPLICATE, on the run's stream. They are
// advisory: the run does not end and the runner's own token still identifies.
#[tokio::test]
async fn a_claim_is_recorded_by_the_model_and_ends_nothing() {
    use crate::{Gate, RunsGate};
    let world = World::new().await;
    let facts = Facts::new(Arc::clone(&world.edge));
    let who = &world.me.model_identity;
    let claims = [
        ModelFact::SummaryReported(runs::SummaryReported {
            turn: 1,
            ordinal: 0,
            scope: "progress".to_string(),
            text_ref: "t:1".to_string(),
        }),
        ModelFact::FinalReported(runs::FinalReported {
            turn: 1,
            text_ref: "t:final".to_string(),
        }),
    ];
    for claim in claims {
        assert_eq!(
            facts.record(who, claim.clone()).await,
            Ok(Recorded::Applied)
        );
        assert_eq!(facts.record(who, claim).await, Ok(Recorded::Duplicate));
    }
    let gate = RunsGate::new(Arc::clone(&world.edge));
    assert!(gate.identify(&world.me.credential).await.is_ok());
    assert!(gate.identify(&world.me.model_credential).await.is_ok());
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16, totality): every AgentFact
// applies in every state of the run and its session, and is never refused,
// errored or parked: live, a turn in flight, a stop ordered, ended, and with
// mail pending in the inbox. A fact that has nothing to do records nothing.
#[tokio::test]
async fn every_agent_fact_applies_in_every_state() {
    use crate::fakes::command_book;
    let states = [
        "live",
        "turn in flight",
        "stop ordered",
        "ended",
        "mail pending",
    ];
    for state in states {
        let world = World::new().await;
        let facts = Facts::new(Arc::clone(&world.edge));
        let me = &world.me;
        match state {
            "turn in flight" => {
                let queue = runs::QueueTurn {
                    prompt_ref: "p".to_string(),
                    resume: false,
                };
                world
                    .edge
                    .handle(command_book(me.identity.run_cover(), queue))
                    .await
                    .unwrap();
            }
            "stop ordered" => {
                let stop = sessions::StopSession {
                    requester_depth: 0,
                    reason: "x".to_string(),
                };
                world
                    .edge
                    .handle(command_book(me.identity.session_cover(), stop))
                    .await
                    .unwrap();
            }
            "ended" => world.end(me).await,
            "mail pending" => {
                world
                    .edge
                    .handle(fact_book(
                        me.identity.session_cover(),
                        sessions::RecordMessageArrived {
                            message_id: "m1".to_string(),
                            from: "peer".to_string(),
                            kind: "note".to_string(),
                            body_ref: "b".to_string(),
                        },
                        "arrive:fixture:m1",
                    ))
                    .await
                    .unwrap();
            }
            _ => {}
        }
        let mut outcomes = Vec::new();
        for fact in RunnerFact::samples() {
            let url = fact.type_url();
            outcomes.push((url, facts.record(&me.identity, fact).await));
        }
        for fact in ModelFact::samples() {
            let url = fact.type_url();
            outcomes.push((url, facts.record(&me.model_identity, fact).await));
        }
        for (url, outcome) in outcomes {
            assert!(
                matches!(outcome, Ok(Recorded::Applied | Recorded::Duplicate)),
                "{url} in state {state}: {outcome:?}"
            );
        }
    }
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
