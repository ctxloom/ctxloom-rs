//! Admission: what the agent listener does with each arrival. The listener
//! accepts only facts (the fact allowlists), the closed AgentRequests,
//! and reads/feeds; a generic Command envelope is refused by construction
//! (owner ruling 2026-11-04, S5 review Q3). A fact: identify; not a Command;
//! allowlist; reach (the edge's exposure); owns; serve. A denial is answered as
//! an unknown domain.

use std::sync::Arc;

use angzarr_embedded::v1::CommandBook;
use angzarr_embedded::Answered;
use run_ports::{Arrival, Edge};

use crate::error::AppError;
use crate::gate::{Credential, RunsGate};
use crate::requests::{AgentRequest, Requested};

pub struct Admission<E: Edge> {
    edge: Arc<E>,
    gate: RunsGate<E>,
}

impl<E: Edge> Admission<E> {
    pub fn new(edge: Arc<E>) -> Admission<E> {
        let gate = RunsGate::new(Arc::clone(&edge));
        Admission { edge, gate }
    }

    /// One fact from the agent listener. A Command body of ANY type is hidden.
    pub async fn admit(
        &self,
        arrival: Arrival,
        credential: &Credential,
        book: CommandBook,
    ) -> Result<Answered, AppError> {
        let _ = (&self.edge, &self.gate, arrival, credential, book);
        todo!("S5 stub")
    }

    /// One ask from the closed AgentRequests service: identify, then the request
    /// is stamped with the caller's identity and sent on its own session.
    pub async fn request(
        &self,
        arrival: Arrival,
        credential: &Credential,
        request: AgentRequest,
    ) -> Result<Requested, AppError> {
        let _ = (arrival, credential, request);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "admission.test.rs"]
mod tests;
