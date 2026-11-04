//! The gate's duties against the local shapes: `identify`, the allowlist,
//! `owns` (listener plan 4.1, 4.5; `stamp` left with the Command envelope, Q3 ruling). RED (S5): stubs; and, since the
//! world is built through the real aggregates, red until those land too.

use std::sync::Arc;

use angzarr_embedded::v1::{Cover, Uuid};
use run_ports::Token;

use crate::fakes::World;
use crate::{owns, AgentFact, AgentRequest, Allowlist, AppError, Credential, Gate, RunsGate, Why};

/// The type URL of a domain message's wire form.
fn url<M: angzarr_embedded::markup::DomainMessage>() -> String {
    <M::Wire as prost::Name>::type_url()
}

fn why(error: AppError) -> Why {
    let AppError::Unauthenticated(why) = error else {
        panic!("not unauthenticated: {error:?}");
    };
    why
}

// PORTED-FROM-GO: internal/core/coord/session_owner_revoke_test.go
// TestRevokeSessionOwner_TokenStopsIdentifying (its precondition: "a freshly
// minted owner token must identify", at depth 0).
#[tokio::test]
async fn a_live_runs_token_identifies_its_run_session_harp_and_depth() {
    let world = World::new().await;
    let gate = RunsGate::new(Arc::clone(&world.edge));
    let identity = gate.identify(&world.me.credential).await.unwrap();
    assert_eq!(identity, world.me.identity);
    assert_eq!(identity.harp, "amber-swift-owl");
    assert_eq!(identity.depth, 0);
}

// PORTED-FROM-GO: internal/core/coord/creds_test.go
// TestVerifyToken_ConstantTimeMatch (a wrong token does not identify), with
// plan 5.3 ("the hash must match").
#[tokio::test]
async fn a_wrong_token_for_a_real_run_does_not_identify() {
    let world = World::new().await;
    let gate = RunsGate::new(Arc::clone(&world.edge));
    let forged = Credential {
        token: Token::from_bytes([0xee; 32]),
        run: world.me.identity.run.clone(),
    };
    assert_eq!(
        why(gate.identify(&forged).await.unwrap_err()),
        Why::Mismatch
    );
}

// NEW-DESIGN (plan 5.3: "the bearer plus the run id find the run's stream"):
// one run's token presented for another run's id does not identify.
#[tokio::test]
async fn another_runs_token_does_not_identify_for_this_runs_id() {
    let world = World::new().await;
    let gate = RunsGate::new(Arc::clone(&world.edge));
    let crossed = Credential {
        token: world.other.token.clone(),
        run: world.me.identity.run.clone(),
    };
    assert_eq!(
        why(gate.identify(&crossed).await.unwrap_err()),
        Why::Mismatch
    );
}

// NEW-DESIGN: a run id no stream holds is unauthenticated, not an error of
// another kind (existence cannot be probed).
#[tokio::test]
async fn an_unknown_run_does_not_identify() {
    let world = World::new().await;
    let gate = RunsGate::new(Arc::clone(&world.edge));
    let nobody = Credential {
        token: world.me.token.clone(),
        run: Uuid::from_bytes([0x42; 16]),
    };
    assert_eq!(
        why(gate.identify(&nobody).await.unwrap_err()),
        Why::UnknownRun
    );
}

// PORTED-FROM-GO: internal/core/coord/session_owner_revoke_test.go
// TestRevokeSessionOwner_TokenStopsIdentifying and
// internal/core/coord/creds_test.go TestCredentialRevocation_RevokesTheChildsCredential,
// per D29: EVERY token, the owner's included, stops identifying when its run's
// end is recorded.
#[tokio::test]
async fn a_token_stops_identifying_when_its_runs_end_is_recorded_the_owners_too() {
    let world = World::new().await;
    let child = world.child().await;
    let gate = RunsGate::new(Arc::clone(&world.edge));
    for party in [&world.me, &child] {
        assert!(
            gate.identify(&party.credential).await.is_ok(),
            "precondition"
        );
        world.end(party).await;
        assert_eq!(
            why(gate.identify(&party.credential).await.unwrap_err()),
            Why::Revoked,
            "depth {}",
            party.identity.depth
        );
    }
}

// PORTED-FROM-GO: internal/core/coord/session_owner_revoke_test.go
// TestRevokeSessionOwner_TokenStopsIdentifying ("revoking the OLD one must
// not disturb the NEW one"): ending one run revokes only its token.
#[tokio::test]
async fn ending_one_run_revokes_only_its_own_token() {
    let world = World::new().await;
    let gate = RunsGate::new(Arc::clone(&world.edge));
    world.end(&world.me).await;
    assert!(gate.identify(&world.other.credential).await.is_ok());
}

// NEW-DESIGN (listener plan 4.5): `owns` is a pure predicate over the cover:
// the caller's own run, its own session, and nothing else.
#[tokio::test]
async fn an_identity_owns_its_own_run_and_session_and_nothing_else() {
    let world = World::new().await;
    let me = &world.me.identity;
    let other = &world.other.identity;
    assert!(owns(me, &me.run_cover()));
    assert!(owns(me, &me.session_cover()));
    assert!(!owns(me, &other.run_cover()));
    assert!(!owns(me, &other.session_cover()));
    // Its own root in another domain is not its own stream.
    let trees = Cover {
        domain: "trees".to_string(),
        root: Some(me.run.clone()),
        edition: None,
    };
    assert!(!owns(me, &trees));
    // A cover with no root, or in another edition, is not owned.
    assert!(!owns(
        me,
        &Cover {
            root: None,
            ..me.run_cover()
        }
    ));
    assert!(!owns(
        me,
        &Cover {
            edition: Some(Default::default()),
            ..me.run_cover()
        }
    ));
}

// NEW-DESIGN (listener plan section 11, risk "Allowlist drift": "ctxloom's own
// test should assert every variant is allowlisted (S5)"): every AgentFact
// variant is on the fact allowlist, and the match below stops compiling when a
// variant is added without being listed here.
#[test]
fn every_agent_fact_variant_is_on_the_allowlist() {
    let allowlist = Allowlist::agent();
    let samples = AgentFact::samples();
    for fact in &samples {
        match fact {
            AgentFact::RunnerAttached(_)
            | AgentFact::TurnStarted(_)
            | AgentFact::TurnCompleted(_)
            | AgentFact::TurnFailed(_)
            | AgentFact::RunExited(_)
            | AgentFact::MessageClaimed(_)
            | AgentFact::MessageDelivered(_)
            | AgentFact::MessageFailed(_)
            | AgentFact::EngineSessionBound(_) => {}
        }
        assert!(
            allowlist.permits_fact(&fact.type_url()),
            "{} is not allowlisted",
            fact.type_url()
        );
    }
    assert_eq!(samples.len(), 9, "one sample per variant");
    assert_eq!(
        allowlist.facts().len(),
        samples.len(),
        "nothing else is on the fact list"
    );
}

// NEW-DESIGN (D6: "facts must not open a path for arbitrary commands"): no
// command of the domains is permitted as a fact (the fact list is closed to the
// AgentFact types), and no AgentRequest has a wire type on it either.
#[test]
fn the_fact_allowlist_holds_no_command() {
    let allowlist = Allowlist::agent();
    for command in [
        url::<runs::EnqueueRun>(),
        url::<runs::QueueTurn>(),
        url::<runs::RequestStop>(),
        url::<sessions::MintSession>(),
        url::<sessions::RenameSession>(),
        url::<sessions::ForgetSession>(),
        url::<sessions::StopSession>(),
    ] {
        assert!(!allowlist.permits_fact(&command), "{command} as a fact");
    }
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q3): the request kinds are a
// closed type of their own, enumerable like AgentFact; the match stops
// compiling when a kind is added without being listed here.
#[test]
fn every_agent_request_kind_is_enumerated() {
    let samples = AgentRequest::samples();
    for request in &samples {
        match request {
            AgentRequest::Stop { .. } => {}
        }
    }
    assert_eq!(samples.len(), 1, "one sample per kind");
}
