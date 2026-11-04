//! The run's enqueue, which mints its two credentials (D29; owner ruling
//! 2026-11-04, item 16): each token is returned once, only its SHA-256 is
//! journaled. STUBS (S5 red).

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

/// What the enqueue gives back: the run's root and its two tokens, shown once.
/// The runner's goes to the runner's 0600 file (launcher, S6); the model's to a
/// separate file only the model-side adapter reads (S6, S10). Owner ruling
/// 2026-11-04, item 16: two principals per run.
#[derive(Debug)]
pub struct Enqueued {
    pub run: Uuid,
    pub runner_token: Token,
    pub model_token: Token,
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
