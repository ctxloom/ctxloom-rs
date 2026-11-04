//! The agent's refusable asks (D7): a CLOSED request type of their own (owner
//! ruling 2026-11-04, S5 review Q3: a distinct service with a fixed request
//! enum, not generic Command bodies), identity-stamped. S5
//! carries one, the coordinator-only `Stop`, to prove the admission rule that a
//! child cannot send a coordinator-only command; the others (`SendMessage`,
//! `SpawnChild`, `Report`) arrive with their aggregates. STUBS (S5 red).

use std::sync::Arc;

use run_ports::Edge;

use crate::error::AppError;
use crate::identity::Identity;

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
        todo!("S5 stub")
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
        let _ = (&self.edge, who, request);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "requests.test.rs"]
mod tests;
