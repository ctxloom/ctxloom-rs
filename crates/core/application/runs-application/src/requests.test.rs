//! The agent's refusable asks (D7). RED (S5): stubs.

use std::sync::Arc;

use crate::fakes::World;
use run_ports::Unserved;

use crate::{AgentRequest, AgentRequests, AppError, Requested};

fn stop() -> AgentRequest {
    AgentRequest::Stop {
        reason: "user".to_string(),
    }
}

// NEW-DESIGN (S5 plan red test: "A child identity cannot send a
// coordinator-only command"; Go's I2): refused with the session's code, and
// nothing recorded.
#[tokio::test]
async fn a_child_cannot_send_a_coordinator_only_command() {
    let world = World::new().await;
    let child = world.child().await;
    let requests = AgentRequests::new(Arc::clone(&world.edge));
    let before = world
        .edge
        .pages(&child.identity.session_cover())
        .await
        .len();
    let result = requests.request(&child.model_identity, stop()).await;
    let Err(AppError::Refused { code, args }) = result else {
        panic!("not refused: {result:?}");
    };
    assert_eq!(code, "sessions-child-cannot-stop");
    assert_eq!(args["depth"], "2");
    assert_eq!(
        world
            .edge
            .pages(&child.identity.session_cover())
            .await
            .len(),
        before
    );
}

// NEW-DESIGN: the owner's stop is applied, the order recorded on its session;
// a second records nothing.
#[tokio::test]
async fn an_owners_stop_is_applied_and_a_second_changes_nothing() {
    let world = World::new().await;
    let requests = AgentRequests::new(Arc::clone(&world.edge));
    assert_eq!(
        requests.request(&world.me.model_identity, stop()).await,
        Ok(Requested::Applied)
    );
    assert_eq!(
        requests.request(&world.me.model_identity, stop()).await,
        Ok(Requested::Unchanged)
    );
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16, fail closed): requests are the
// MODEL principal's. Every kind, sent as the RUNNER, is hidden and nothing is
// sent or recorded: the runner's credential speaks in observations, not asks.
#[tokio::test]
async fn the_runner_principal_cannot_send_any_request() {
    let world = World::new().await;
    let requests = AgentRequests::new(Arc::clone(&world.edge));
    let session = world.me.identity.session_cover();
    for request in AgentRequest::samples() {
        let before = world.edge.pages(&session).await.len();
        let served = world.edge.served();
        let denied = requests
            .request(&world.me.identity, request)
            .await
            .unwrap_err();
        assert!(matches!(denied, AppError::Denied(Unserved::Hidden { .. })));
        assert_eq!(world.edge.served(), served);
        assert_eq!(world.edge.pages(&session).await.len(), before);
    }
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16, totality of the closed type):
// every kind of AgentRequest is exercised against its expected outcome for the
// owner (applied, then unchanged) and for a child (refused with its code). The
// `match`es stop compiling when a kind is added without an expectation here.
#[tokio::test]
async fn every_agent_request_kind_has_its_owner_and_child_outcome() {
    for request in AgentRequest::samples() {
        let (owner_first, child_code) = match &request {
            AgentRequest::Stop { .. } => (Requested::Applied, "sessions-child-cannot-stop"),
        };
        let world = World::new().await;
        let child = world.child().await;
        let requests = AgentRequests::new(Arc::clone(&world.edge));
        assert_eq!(
            requests
                .request(&world.me.model_identity, request.clone())
                .await,
            Ok(owner_first),
            "{request:?}"
        );
        let Err(AppError::Refused { code, .. }) =
            requests.request(&child.model_identity, request).await
        else {
            panic!("a child's request was not refused");
        };
        assert_eq!(code, child_code);
    }
}
