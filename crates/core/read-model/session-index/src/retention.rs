//! When a session's transcript may be destroyed: Go's `sessions.Retention`
//! (release/0.7, retention.go). The transcript is the session's record, so
//! every destroyer asks first.

use angzarr_embedded::Timestamp;

use crate::SessionRecord;
use sessions::Origin;

/// Why a transcript must be kept.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Retained {
    /// The session is marked keep (`ctxloom session keep`): any age.
    Kept,
    /// The session ended after the cutoff; `ended` is when, or `None` when
    /// nothing records when it ended at all.
    Recent { ended: Option<Timestamp> },
    /// No cutoff was stated: refused, never read as "everything is old
    /// enough".
    NoBound,
}

/// The rule: a transcript may go once its session ended at or before
/// `cutoff` (now minus `session_transcript_retention`), unless marked keep.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Retention {
    pub cutoff: Option<Timestamp>,
}

/// When `record` ended, for retention: the LATER of its recorded end and
/// `last_active` (when its conversation was last written). A session resumed
/// after its recorded end, or one that died before recording an end, did not
/// end before its conversation was last written. `None` when neither is
/// known.
pub fn ended(record: Option<&SessionRecord>, last_active: Option<Timestamp>) -> Option<Timestamp> {
    let recorded = record.and_then(|r| r.ended_at);
    match (recorded, last_active) {
        (Some(a), Some(b)) => Some(later(a, b)),
        (a, b) => a.or(b),
    }
}

/// The later of two instants.
fn later(a: Timestamp, b: Timestamp) -> Timestamp {
    if key(a) >= key(b) {
        a
    } else {
        b
    }
}

/// An instant as it orders.
fn key(at: Timestamp) -> (i64, i32) {
    (at.seconds, at.nanos)
}

impl Retention {
    /// Why `record`'s transcript must be kept, or `None` when it is
    /// reclaimable. `last_active` is when its conversation was last written.
    ///
    /// An internal one-shot (`Origin::Oneshot`) is a machine's single turn
    /// that nobody resumes, never a human's record: reclaimable at any age,
    /// unless marked keep.
    pub fn retains(
        &self,
        record: Option<&SessionRecord>,
        last_active: Option<Timestamp>,
    ) -> Option<Retained> {
        let Some(cutoff) = self.cutoff else {
            return Some(Retained::NoBound);
        };
        if record.is_some_and(|r| r.kept) {
            return Some(Retained::Kept);
        }
        if record.is_some_and(|r| r.origin == Some(Origin::Oneshot)) {
            return None;
        }
        let ended = ended(record, last_active);
        match ended {
            Some(end) if key(end) <= key(cutoff) => None,
            _ => Some(Retained::Recent { ended }),
        }
    }
}

#[cfg(test)]
#[path = "retention.test.rs"]
mod tests;
