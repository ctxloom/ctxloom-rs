//! The agent's refusable asks (D7). RED (S5): stubs.

use std::sync::Arc;

use crate::fakes::World;
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
    let result = requests.request(&child.identity, stop()).await;
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
        requests.request(&world.me.identity, stop()).await,
        Ok(Requested::Applied)
    );
    assert_eq!(
        requests.request(&world.me.identity, stop()).await,
        Ok(Requested::Unchanged)
    );
}
