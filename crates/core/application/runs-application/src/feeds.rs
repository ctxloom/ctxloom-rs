//! Feeds: per-stream catch-up from a resume point (plan 6; listener plan 5).
//! The cursor is `(cover, next_sequence)` of ONE stream; nothing here is a
//! position across streams. STUBS (S5 red). No transport: in process, a
//! [`Follow`] is polled.

use std::sync::Arc;

use angzarr_embedded::v1::{Cover, EventPage, Uuid};
use run_ports::Edge;

use crate::error::AppError;
use crate::gate::Credential;
use crate::identity::Identity;

/// Where one stream's reader is: the next sequence it wants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub domain: String,
    pub root: Uuid,
    pub next_sequence: u64,
}

impl Cursor {
    /// The start of `cover`'s stream.
    pub fn start(cover: &Cover) -> Cursor {
        Cursor {
            domain: cover.domain.clone(),
            root: cover.root.clone().unwrap_or_default(),
            next_sequence: 0,
        }
    }

    pub fn cover(&self) -> Cover {
        Cover {
            domain: self.domain.clone(),
            root: Some(self.root.clone()),
            edition: None,
        }
    }
}

pub struct Feeds<E: Edge> {
    edge: Arc<E>,
}

impl<E: Edge> Feeds<E> {
    pub fn new(edge: Arc<E>) -> Feeds<E> {
        Feeds { edge }
    }

    /// One read of `cover` from `from`, for `who`. A stream `who` does not own
    /// reads as an absent stream: empty, never an error (listener plan 4.3).
    pub async fn read(
        &self,
        who: &Identity,
        cover: &Cover,
        from: u64,
    ) -> Result<Vec<EventPage>, AppError> {
        let _ = (&self.edge, who, cover, from);
        todo!("S5 stub")
    }

    /// `who`'s session inbox from `from`: the catch-up that finds mail from
    /// outside the run's workflow (D10; plan 5.2).
    pub async fn inbox(&self, who: &Identity, from: u64) -> Result<Vec<EventPage>, AppError> {
        let _ = (who, from);
        todo!("S5 stub")
    }

    /// Follow one stream from `cursor`, re-identifying on every poll.
    pub fn follow(&self, credential: Credential, cursor: Cursor) -> Follow<E> {
        Follow {
            edge: Arc::clone(&self.edge),
            credential,
            cursor,
        }
    }
}

/// A resumable reader of one stream.
pub struct Follow<E: Edge> {
    edge: Arc<E>,
    credential: Credential,
    cursor: Cursor,
}

impl<E: Edge> Follow<E> {
    /// The pages at or above the cursor, which then moves past them: no gap and
    /// no duplicate by construction. Empty when nothing is new. Fails
    /// `Unauthenticated` once the run has ended (D29: a token dies with its
    /// run).
    pub async fn next(&mut self) -> Result<Vec<EventPage>, AppError> {
        let _ = (&self.edge, &self.credential);
        todo!("S5 stub")
    }

    /// Where this reader is: save it to resume after a dropped connection.
    pub fn cursor(&self) -> Cursor {
        self.cursor.clone()
    }
}

#[cfg(test)]
#[path = "feeds.test.rs"]
mod tests;
