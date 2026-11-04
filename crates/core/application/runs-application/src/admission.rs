//! Admission: what the agent listener does with each arrival. The listener
//! accepts only facts (the fact allowlists), the closed AgentRequests,
//! and reads/feeds; a generic Command envelope is refused by construction
//! (owner ruling 2026-11-04, S5 review Q3). A fact: identify; not a Command;
//! allowlist; reach (the edge's exposure); owns; serve. A denial is answered as
//! an unknown domain.

use std::sync::Arc;

use angzarr_embedded::v1::{command_page, CommandBook};
use angzarr_embedded::Answered;
use run_ports::{Arrival, Edge, Unserved};

use crate::error::AppError;
use crate::gate::{Credential, Gate, RunsGate};
use crate::requests::{AgentRequest, AgentRequests, Requested};

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
        // Authentication first: a credential that does not identify learns
        // nothing of which types or domains exist.
        let who = self.gate.identify(credential).await?;
        // A fact only: a Command envelope of any type is hidden, and so is a
        // fact of a type this principal does not author.
        let permitted = match book.page.as_ref().and_then(|page| page.body.as_ref()) {
            Some(command_page::Body::Fact(fact)) => fact
                .payload
                .as_ref()
                .is_some_and(|any| self.gate.allowlist(&who).permits_fact(&any.type_url)),
            _ => false,
        };
        let owned = book
            .cover
            .as_ref()
            .is_some_and(|cover| self.gate.owns(&who, cover));
        if !permitted || !owned {
            let domain = book.cover.map(|c| c.domain).unwrap_or_default();
            return Err(AppError::Denied(Unserved::Hidden { domain }));
        }
        self.edge
            .serve(book, arrival)
            .await
            .map_err(AppError::from_unserved)
    }

    /// One ask from the closed AgentRequests service: identify, then the request
    /// is stamped with the caller's identity and sent on its own session.
    pub async fn request(
        &self,
        arrival: Arrival,
        credential: &Credential,
        request: AgentRequest,
    ) -> Result<Requested, AppError> {
        let _ = arrival;
        let who = self.gate.identify(credential).await?;
        AgentRequests::new(Arc::clone(&self.edge))
            .request(&who, request)
            .await
    }
}

#[cfg(test)]
#[path = "admission.test.rs"]
mod tests;
