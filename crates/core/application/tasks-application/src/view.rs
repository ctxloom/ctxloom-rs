//! A task as the application answers with it: the folded state and the
//! values derived from it when read.

use annals::next::Timestamp;
use sha2::{Digest, Sha256};
use tasks::{Tag, TaskState, DEFERRED, TO_DO};

use crate::Project;

/// The status of a task being worked on.
pub const IN_PROGRESS: &str = "In Progress";
/// The status of a finished task.
pub const DONE: &str = "Done";
/// The status of a task put away.
pub const ARCHIVED: &str = "Archived";

/// The default statuses, in their display order (Go's `statusTaxonomy`).
/// The set is open: a task may hold any status not starting with `@`.
pub const DEFAULT_STATUSES: [&str; 5] = [IN_PROGRESS, TO_DO, DEFERRED, DONE, ARCHIVED];

/// The status class of every status that is not terminal.
pub const OPEN: &str = "@open";
/// The status class of every terminal status.
pub const TERMINAL: &str = "@terminal";
/// The status classes a status filter takes beside statuses.
pub const STATUS_CLASSES: [&str; 2] = [OPEN, TERMINAL];

/// Whether `status` is a finished one: Done or Archived. A status outside
/// the default set is not.
pub fn is_terminal(status: &str) -> bool {
    status == DONE || status == ARCHIVED
}

/// Whether a task in `status` is active: shown by a listing by default,
/// being neither terminal nor Deferred.
pub fn is_active(status: &str) -> bool {
    !is_terminal(status) && status != DEFERRED
}

/// One status, as `statuses` describes it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatusInfo {
    pub name: String,
    /// Its place in the display order, from 0.
    pub order: usize,
    pub terminal: bool,
    /// Whether a task needs a revive trigger to enter it: Deferred.
    pub requires_trigger: bool,
}

impl StatusInfo {
    /// `name`, at `order`.
    pub fn of(name: &str, order: usize) -> StatusInfo {
        StatusInfo {
            name: name.to_string(),
            order,
            terminal: is_terminal(name),
            requires_trigger: name == DEFERRED,
        }
    }
}

/// A task read, and the project it was read in.
#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub project: Project,
    pub task: Task,
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

/// Which of a task's tags a person is shown: tagma's display hiding
/// (SPEC §7), from the tag schema's `hide` facts, with `tagma:*` hidden
/// unless a fact says otherwise. Storage, matching and JSON are never
/// touched: only what a person reads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Shown(tagma_core::HideConfig);

impl Shown {
    /// What `schema`'s hide facts show.
    pub fn of(schema: &crate::TagSchema) -> Shown {
        Shown(tagma_core::HideConfig::from_patterns(schema.hide_facts()))
    }

    /// Whether `tag` is shown.
    pub fn shows(&self, tag: &Tag) -> bool {
        !tagma_core::tag_hidden(&tag.to_core(), &self.0)
    }

    /// Those of `tags` shown, in their order.
    pub fn shown<'a>(&self, tags: &'a [Tag]) -> Vec<&'a Tag> {
        tags.iter().filter(|t| self.shows(t)).collect()
    }
}

/// What no hide fact shows: everything but `tagma:*`.
impl Default for Shown {
    fn default() -> Self {
        Shown::of(&crate::TagSchema::default())
    }
}

#[cfg(test)]
#[path = "view.test.rs"]
mod tests;
