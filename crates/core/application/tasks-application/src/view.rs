//! A task as the application answers with it: the folded state and the
//! values derived from it when read.

use annals::next::Timestamp;
use sha2::{Digest, Sha256};
use tasks::{Tag, TaskState};

/// The status of a finished task.
pub const DONE: &str = "Done";
/// The status of a task put away.
pub const ARCHIVED: &str = "Archived";

/// Whether `status` is a finished one: Done or Archived. A status outside
/// the default set is not.
pub fn is_terminal(status: &str) -> bool {
    status == DONE || status == ARCHIVED
}

/// One task, as read.
#[derive(Clone, PartialEq, Debug)]
pub struct Task {
    pub harp_id: String,
    pub text: String,
    pub status: String,
    /// Whether the status is terminal.
    pub checked: bool,
    /// The first 12 hex digits of the SHA-256 of the trimmed text.
    pub text_hash: String,
    pub trigger: String,
    pub origin_session: String,
    /// Normalized: unique, sorted by their canonical spelling (their
    /// `Display`).
    pub tags: Vec<Tag>,
    pub created_at: Timestamp,
}

impl Task {
    /// The task `state` holds.
    pub fn of(state: &TaskState) -> Task {
        Task {
            harp_id: state.harp_id.clone(),
            text: state.text.clone(),
            status: state.status.clone(),
            checked: is_terminal(&state.status),
            text_hash: text_hash(&state.text),
            trigger: state.trigger.clone(),
            origin_session: state.origin_session.clone(),
            tags: state.tags.clone(),
            created_at: state.created_at,
        }
    }
}

/// The first 12 hex digits of the SHA-256 of `text`, trimmed: a task's text
/// identity, as Go's `hashText`.
pub fn text_hash(text: &str) -> String {
    Sha256::digest(text.trim().as_bytes())
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "view.test.rs"]
mod tests;
