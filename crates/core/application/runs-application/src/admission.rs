//! Admission: what the agent listener does with each arrival, in order
//! (listener plan 4.2): identify; allowlist; reach (the edge's exposure);
//! owns; stamp; serve. A denial is answered as an unknown domain. STUBS.

use std::sync::Arc;

use angzarr_embedded::v1::CommandBook;
use angzarr_embedded::Answered;
use run_ports::{Arrival, Edge};

use crate::error::AppError;
use crate::gate::{Credential, RunsGate};

pub struct Admission<E: Edge> {
    edge: Arc<E>,
    gate: RunsGate<E>,
}

impl<E: Edge> Admission<E> {
    pub fn new(edge: Arc<E>) -> Admission<E> {
        let gate = RunsGate::new(Arc::clone(&edge));
        Admission { edge, gate }
    }

    /// One request from the agent listener.
    pub async fn admit(
        &self,
        arrival: Arrival,
        credential: &Credential,
        book: CommandBook,
    ) -> Result<Answered, AppError> {
        let _ = (&self.edge, &self.gate, arrival, credential, book);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "admission.test.rs"]
mod tests;
