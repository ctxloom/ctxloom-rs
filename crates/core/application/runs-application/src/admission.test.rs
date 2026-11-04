//! Admission: the listener's order of evaluation, in process (listener plan
//! 4.2). Every refusal is checked over BOTH arrivals, loopback and the
//! container bridge (S5 plan: "the same admission refusals hold over the TCP
//! bridge listener"). RED (S5): stubs.

use std::sync::Arc;

use angzarr_embedded::v1::{Cover, Uuid};
use angzarr_embedded::{Failure, KernelError, Malformed};
use run_ports::{Arrival, Unserved};

use crate::fakes::{command_book, fact_book, World};
use crate::{Admission, AgentFact, AgentRequest, AppError, Credential, Principal, Requested, Why};

const ARRIVALS: [Arrival; 2] = [Arrival::Loopback, Arrival::Ip];

fn exited(run: &Cover) -> angzarr_embedded::v1::CommandBook {
    fact_book(
        run.clone(),
        runs::RunExited { code: 0, signal: 0 },
        "exit:x",
    )
}

fn unknown_domain(domain: &str) -> Failure<angzarr_embedded::Rejected> {
    Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
        domain: domain.to_string(),
    }))
}

// PORTED-FROM-GO: S5 plan red test "A fact on the caller's own run is APPLIED"
// (over the admission path, both arrivals).
#[tokio::test]
async fn a_fact_on_the_callers_own_run_is_admitted_and_applied() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        let answered = admission
            .admit(
                arrival,
                &world.me.credential,
                exited(&world.me.identity.run_cover()),
            )
            .await
            .unwrap_or_else(|e| panic!("{arrival:?}: {e:?}"));
        assert_eq!(
            answered.outcome.map(|o| o.delivery),
            Some(angzarr_embedded::v1::DeliveryOutcome::Applied as i32),
            "{arrival:?}"
        );
    }
}

// NEW-DESIGN (S5 plan red test: "A fact on another run's stream is answered as
// an unknown domain"; listener plan 4.3: hidden, existence cannot be probed):
// the caller is answered exactly as for a domain that does not exist, and the
// other run's stream is untouched.
#[tokio::test]
async fn a_fact_on_another_runs_stream_is_answered_as_an_unknown_domain() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        let theirs = world.other.identity.run_cover();
        let before = world.edge.pages(&theirs).await.len();
        let denied = admission
            .admit(arrival, &world.me.credential, exited(&theirs))
            .await
            .unwrap_err();
        let AppError::Denied(unserved) = denied else {
            panic!("{arrival:?}: not denied: {denied:?}");
        };
        // Not a distinguishable answer: the same a missing domain gets.
        assert_eq!(unserved.answer(), unknown_domain("runs"), "{arrival:?}");
        assert_eq!(world.edge.pages(&theirs).await.len(), before, "{arrival:?}");
    }
}

// NEW-DESIGN (listener plan 4.3): a stream in a domain that exists but was
// never exposed (trees) is answered the same as a domain that does not exist.
#[tokio::test]
async fn an_unexposed_domain_is_hidden_the_same_as_a_missing_one() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        for domain in ["trees", "nowhere"] {
            let cover = Cover {
                domain: domain.to_string(),
                root: Some(world.me.identity.run.clone()),
                edition: None,
            };
            let denied = admission
                .admit(arrival, &world.me.credential, exited(&cover))
                .await
                .unwrap_err();
            let AppError::Denied(unserved) = denied else {
                panic!("{domain}: {denied:?}");
            };
            assert_eq!(
                unserved.answer(),
                unknown_domain(domain),
                "{domain} {arrival:?}"
            );
        }
    }
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q3 (a)): the plan's red test
// "A Command body on the agent listener is refused" holds as written. A
// Command envelope of ANY type is hidden, on the caller's own streams too: a
// non-request command, and the wire type of an AgentRequest itself.
#[tokio::test]
async fn a_command_envelope_of_any_type_is_refused_on_the_agent_listener() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        let run = world.me.identity.run_cover();
        let session = world.me.identity.session_cover();
        let books = [
            command_book(
                run.clone(),
                runs::QueueTurn {
                    prompt_ref: "x".to_string(),
                    resume: false,
                },
            ),
            command_book(
                session.clone(),
                sessions::StopSession {
                    requester_depth: 0,
                    reason: "as a generic command".to_string(),
                },
            ),
        ];
        for book in books {
            let denied = admission
                .admit(arrival, &world.me.credential, book)
                .await
                .unwrap_err();
            assert!(
                matches!(denied, AppError::Denied(Unserved::Hidden { .. })),
                "{arrival:?}"
            );
        }
        assert_eq!(world.edge.pages(&run).await.len(), 1, "{arrival:?}");
        assert!(!world.edge.pages(&session).await.is_empty());
        assert_eq!(
            world.edge.pages(&session).await.len(),
            2,
            "mint and owed only"
        );
    }
}

// NEW-DESIGN, owner ruling 2026-11-04 (S5 review Q3 (b); item 16: requests are
// the MODEL principal's): AgentRequest kinds pass through their closed type,
// over both arrivals. The owner's stop is applied; a child's is refused by the session with its code, and the
// requester is the identity, never a field of the request (there is none to
// forge, so the old forged-depth test is gone with the Command envelope).
#[tokio::test]
async fn agent_requests_pass_through_the_closed_request_type() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let child = world.child().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        for request in AgentRequest::samples() {
            assert_eq!(
                admission
                    .request(arrival, &world.me.model_credential, request.clone())
                    .await,
                Ok(Requested::Applied),
                "{arrival:?}"
            );
            let refused = admission
                .request(arrival, &child.model_credential, request)
                .await
                .unwrap_err();
            let AppError::Refused { code, .. } = refused else {
                panic!("{arrival:?}: {refused:?}");
            };
            assert_eq!(code, "sessions-child-cannot-stop");
        }
    }
}

// NEW-DESIGN (same ruling): a request needs a valid credential like any other
// arrival; a revoked token is refused before anything is sent.
#[tokio::test]
async fn an_agent_request_with_a_revoked_token_is_refused() {
    let world = World::new().await;
    let admission = Admission::new(Arc::clone(&world.edge));
    world.end(&world.me).await;
    for request in AgentRequest::samples() {
        let refused = admission
            .request(Arrival::Ip, &world.me.model_credential, request)
            .await
            .unwrap_err();
        assert_eq!(refused, AppError::Unauthenticated(Why::Revoked));
    }
}

// PORTED-FROM-GO: internal/core/coord/session_owner_revoke_test.go
// TestRevokeSessionOwner_TokenStopsIdentifying + S5 plan red test "A stale or
// revoked token is refused": after the run's end, the token is refused and
// nothing is written.
#[tokio::test]
async fn a_revoked_token_is_refused_and_writes_nothing() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        world.end(&world.me).await;
        let run = world.me.identity.run_cover();
        let before = world.edge.pages(&run).await.len();
        let refused = admission
            .admit(arrival, &world.me.credential, exited(&run))
            .await
            .unwrap_err();
        assert_eq!(
            refused,
            AppError::Unauthenticated(Why::Revoked),
            "{arrival:?}"
        );
        assert_eq!(world.edge.pages(&run).await.len(), before, "{arrival:?}");
    }
}

// NEW-DESIGN (S5 plan: "a stale ... token"): a token for a run the core never
// held (stale after a store reset) is refused as unknown.
#[tokio::test]
async fn a_stale_credential_for_a_run_that_is_gone_is_refused() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        let stale = Credential {
            token: world.me.token.clone(),
            run: Uuid::from_bytes([0x13; 16]),
        };
        let refused = admission
            .admit(arrival, &stale, exited(&world.me.identity.run_cover()))
            .await
            .unwrap_err();
        assert!(
            matches!(refused, AppError::Unauthenticated(_)),
            "{arrival:?}"
        );
    }
}

// NEW-DESIGN (listener plan 4.2 step 1: authentication first): a credential
// that does not identify is refused BEFORE the allowlist, so it learns nothing
// about which types or domains exist.
#[tokio::test]
async fn authentication_is_checked_before_anything_else() {
    let world = World::new().await;
    let admission = Admission::new(Arc::clone(&world.edge));
    let nobody = Credential {
        token: world.me.token.clone(),
        run: Uuid::from_bytes([0x77; 16]),
    };
    let nonsense = command_book(
        Cover {
            domain: "nowhere".to_string(),
            root: None,
            edition: None,
        },
        runs::QueueTurn {
            prompt_ref: String::new(),
            resume: false,
        },
    );
    let refused = admission
        .admit(Arrival::Ip, &nobody, nonsense)
        .await
        .unwrap_err();
    assert!(matches!(refused, AppError::Unauthenticated(_)));
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16, fail closed): the model's
// credential cannot forge what the runner observed. Every runner fact, sent
// with the MODEL's token, over both arrivals, is hidden as an unknown domain
// and nothing is written to the run or the session.
#[tokio::test]
async fn the_models_credential_cannot_forge_a_runner_fact() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        let who = &world.me.identity;
        let (run, session) = (who.run_cover(), who.session_cover());
        let (run_before, session_before) = (
            world.edge.pages(&run).await.len(),
            world.edge.pages(&session).await.len(),
        );
        for fact in AgentFact::samples()
            .into_iter()
            .filter(|f| f.author() == Principal::Runner)
        {
            let denied = admission
                .admit(arrival, &world.me.model_credential, fact.book(who))
                .await
                .unwrap_err();
            assert!(
                matches!(denied, AppError::Denied(Unserved::Hidden { .. })),
                "{arrival:?} {}: {denied:?}",
                fact.type_url()
            );
        }
        assert_eq!(world.edge.pages(&run).await.len(), run_before);
        assert_eq!(world.edge.pages(&session).await.len(), session_before);
        // The run is still live: the forged exit did not revoke anything.
        assert!(admission
            .admit(arrival, &world.me.credential, exited(&run))
            .await
            .is_ok());
    }
}

// NEW-DESIGN (same ruling): the model's credential records its claims, over
// both arrivals; and the runner's credential cannot record them (disjoint
// lists), nor send a request.
#[tokio::test]
async fn the_model_records_claims_and_the_runner_cannot() {
    for arrival in ARRIVALS {
        let world = World::new().await;
        let admission = Admission::new(Arc::clone(&world.edge));
        let who = &world.me.identity;
        for claim in AgentFact::samples()
            .into_iter()
            .filter(|f| f.author() == Principal::Model)
        {
            let answered = admission
                .admit(arrival, &world.me.model_credential, claim.book(who))
                .await
                .unwrap_or_else(|e| panic!("{arrival:?} {}: {e:?}", claim.type_url()));
            assert_eq!(
                answered.outcome.map(|o| o.delivery),
                Some(angzarr_embedded::v1::DeliveryOutcome::Applied as i32)
            );
            let denied = admission
                .admit(arrival, &world.me.credential, claim.book(who))
                .await
                .unwrap_err();
            assert!(matches!(denied, AppError::Denied(Unserved::Hidden { .. })));
        }
        for request in AgentRequest::samples() {
            let denied = admission
                .request(arrival, &world.me.credential, request)
                .await
                .unwrap_err();
            assert!(matches!(denied, AppError::Denied(Unserved::Hidden { .. })));
        }
    }
}
