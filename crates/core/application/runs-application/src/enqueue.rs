//! The run's enqueue, which mints its credential (D29): the token is returned
//! once, only its SHA-256 is journaled. STUBS (S5 red).

use std::sync::Arc;

use angzarr_embedded::v1::Uuid;
use run_ports::{Edge, Token, TokenSource};

use crate::error::AppError;

#[derive(Clone, Debug, PartialEq)]
pub struct EnqueueRequest {
    /// The session's stable id, as UUID text.
    pub session: String,
    pub incarnation: u32,
    pub mode: runs::RunMode,
}

/// What the enqueue gives back: the run's root and its token, shown once (to be
/// written to the 0600 file by the launcher, S6).
#[derive(Debug)]
pub struct Enqueued {
    pub run: Uuid,
    pub token: Token,
}

pub struct Enqueuer<E: Edge, T: TokenSource> {
    edge: Arc<E>,
    source: Arc<T>,
}

impl<E: Edge, T: TokenSource> Enqueuer<E, T> {
    pub fn new(edge: Arc<E>, source: Arc<T>) -> Enqueuer<E, T> {
        Enqueuer { edge, source }
    }

    pub async fn enqueue(&self, request: EnqueueRequest) -> Result<Enqueued, AppError> {
        let _ = (&self.edge, &self.source, request);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "enqueue.test.rs"]
mod tests;
