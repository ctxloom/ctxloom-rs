//! The agent's refusable asks (D7): a CLOSED request type of their own (owner
//! ruling 2026-11-04, S5 review Q3: a distinct service with a fixed request
//! enum, not generic Command bodies), identity-stamped. S5
//! carries one, the coordinator-only `Stop`, to prove the admission rule that a
//! child cannot send a coordinator-only command; the others (`SendMessage`,
//! `SpawnChild`, `Report`) arrive with their aggregates.

use std::sync::Arc;

use angzarr_embedded::v1::CommandBook;
use angzarr_embedded::CommandBookExt;
use run_ports::{Edge, Unserved};

use crate::error::AppError;
use crate::identity::{Identity, Principal};

#[derive(Clone, Debug, PartialEq)]
pub enum AgentRequest {
    /// Order the caller's session's runs to stop. Coordinator-only: refused from
    /// a child (Go's I2).
    Stop { reason: String },
}

impl AgentRequest {
    /// One request of every kind (a test's way to enumerate the closed type).
    #[cfg(any(test, feature = "test-support"))]
    pub fn samples() -> Vec<AgentRequest> {
        vec![AgentRequest::Stop {
            reason: "user".to_string(),
        }]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requested {
    Applied,
    /// The request changed nothing (a second stop).
    Unchanged,
}

pub struct AgentRequests<E: Edge> {
    edge: Arc<E>,
}

impl<E: Edge> AgentRequests<E> {
    pub fn new(edge: Arc<E>) -> AgentRequests<E> {
        AgentRequests { edge }
    }

    /// Send `request` on `who`'s own session, with `who`'s identity stamped.
    /// Requests are the MODEL principal's: a runner identity is `Denied`
    /// (hidden) and nothing is sent (owner ruling 2026-11-04, item 16).
    pub async fn request(
        &self,
        who: &Identity,
        request: AgentRequest,
    ) -> Result<Requested, AppError> {
        // Requests are the model's: the runner speaks in observations.
        if who.principal != Principal::Model {
            return Err(AppError::Denied(Unserved::Hidden {
                domain: sessions::SESSION_DOMAIN.to_string(),
            }));
        }
        // The requester is the identity, never a field of the request.
        let (command, cover) = match request {
            AgentRequest::Stop { reason } => (
                sessions::SessionCommand::from(sessions::StopSession {
                    requester_depth: who.depth,
                    reason,
                }),
                who.session_cover(),
            ),
        };
        let answered = self
            .edge
            .handle(CommandBook::command_of(cover, &command))
            .await
            .map_err(AppError::from_failure)?;
        let recorded = answered.events.is_some_and(|book| !book.pages.is_empty());
        Ok(match recorded {
            true => Requested::Applied,
            false => Requested::Unchanged,
        })
    }
}

#[cfg(test)]
#[path = "requests.test.rs"]
mod tests;
