//! The tasks domain. One task is one aggregate: its stream's root is derived
//! from its harp id, so a harp id is unique because a stream is, and a
//! removed task's stream keeps its harp reserved.
//!
//! The domain types, their wire conversions, the [`TaskCommand`] and
//! [`TaskEvent`] payloads and [`TaskState`]'s annals impl are generated from
//! `proto/ctxloom/tasks/v1/tasks.proto`; this crate holds the rules.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/ctxloom.tasks.v1.domain.rs"));
}
mod rejection;

use std::collections::BTreeSet;
use std::convert::Infallible;

use annals::next::{ids, Domain, IngestCtx, Tx};

pub use generated::*;
pub use rejection::{diagnostic_samples, TaskRejection};

/// The status a task gets when none is given.
pub const TO_DO: &str = "To Do";
/// The status whose tasks wait on a revive trigger.
pub const DEFERRED: &str = "Deferred";
/// The prefix reserved for status classes (`@open`); no status starts with it.
pub const STATUS_CLASS_SIGIL: char = '@';

/// The tasks aggregate.
pub struct Tasks;

impl Domain for Tasks {
    type State = TaskState;
    type Command = TaskCommand;
    type Fact = Infallible;
    type Event = TaskEvent;
    type Rejection = TaskRejection;
    type Rules = ();

    const NAME: &'static str = TASK_DOMAIN;

    fn decide(command: &TaskCommand, tx: &mut Tx<'_, Self>) -> Result<(), TaskRejection> {
        match command {
            TaskCommand::AddTask(add) => decide_add(add, tx),
            TaskCommand::SetStatus(set) => {
                let held = live(tx.state())?;
                let status = valid_status(&set.status)?.ok_or(TaskRejection::StatusRequired)?;
                let given = set.trigger.trim();
                let trigger = if given.is_empty() {
                    held.trigger.clone()
                } else {
                    given.to_string()
                };
                if status == DEFERRED && trigger.is_empty() {
                    return Err(TaskRejection::TriggerRequired);
                }
                let at = tx.recorded_at();
                let deferred_since = if status == DEFERRED {
                    Some(at)
                } else {
                    held.deferred_since
                };
                tx.emit(
                    StatusChanged {
                        status,
                        trigger,
                        at,
                        deferred_since,
                    }
                    .into(),
                );
                Ok(())
            }
            TaskCommand::EditText(edit) => {
                live(tx.state())?;
                let text = required_text(&edit.text)?;
                tx.emit(TextEdited { text }.into());
                Ok(())
            }
            TaskCommand::Retag(retag) => {
                let held: BTreeSet<String> = live(tx.state())?.tags.iter().cloned().collect();
                let add = normalized(&retag.add);
                let remove = normalized(&retag.remove);
                if add.is_empty() && remove.is_empty() {
                    return Err(TaskRejection::TagRequired);
                }
                let after: BTreeSet<String> = held.union(&add).cloned().collect();
                let after: BTreeSet<String> = after.difference(&remove).cloned().collect();
                if after != held {
                    let tags = after.into_iter().collect();
                    tx.emit(Retagged { tags }.into());
                }
                Ok(())
            }
            TaskCommand::RemoveTask(remove) => {
                live(tx.state())?;
                tx.emit(
                    TaskRemoved {
                        reason: remove.reason,
                    }
                    .into(),
                );
                Ok(())
            }
        }
    }

    fn ingest(_: &TaskState, fact: &Infallible, _: &IngestCtx<'_, Self>) -> Vec<TaskEvent> {
        match *fact {}
    }

    fn apply(state: &mut TaskState, event: &TaskEvent, (): &()) {
        match event {
            TaskEvent::TaskAdded(added) => {
                state.harp_id = added.harp_id.clone();
                state.text = added.text.clone();
                state.status = added.status.clone();
                state.trigger = added.trigger.clone();
                state.tags = added.tags.clone();
                state.origin_session = added.origin_session.clone();
                state.created_at = added.at;
                state.status_set_at = added.at;
                state.deferred_since = added.deferred_since;
            }
            TaskEvent::StatusChanged(changed) => {
                state.status = changed.status.clone();
                state.trigger = changed.trigger.clone();
                state.status_set_at = changed.at;
                state.deferred_since = changed.deferred_since;
            }
            TaskEvent::TextEdited(edited) => state.text = edited.text.clone(),
            TaskEvent::Retagged(retagged) => state.tags = retagged.tags.clone(),
            TaskEvent::TaskRemoved(_) => state.removed = true,
        }
    }
}

fn decide_add(add: &AddTask, tx: &mut Tx<'_, Tasks>) -> Result<(), TaskRejection> {
    let held = tx.state();
    if !held.harp_id.is_empty() {
        return Err(TaskRejection::TaskExists {
            harp_id: held.harp_id.clone(),
        });
    }
    let harp_id = add.harp_id.trim();
    if harp_id.is_empty() {
        return Err(TaskRejection::HarpIdRequired);
    }
    // The stream is addressed by the root its harp id derives; the harp id
    // the event records must be that one, or the task takes another's.
    if tx.stream().root.as_ref() != ids::compute_root(TASK_DOMAIN, harp_id).ok().as_ref() {
        return Err(TaskRejection::NotItsStream {
            harp_id: harp_id.to_string(),
        });
    }
    let text = required_text(&add.text)?;
    let status = valid_status(&add.status)?.unwrap_or_else(|| TO_DO.to_string());
    let trigger = add.trigger.trim().to_string();
    if status == DEFERRED && trigger.is_empty() {
        return Err(TaskRejection::TriggerRequired);
    }
    let at = tx.recorded_at();
    let deferred_since = (status == DEFERRED).then_some(at);
    tx.emit(
        TaskAdded {
            harp_id: harp_id.to_string(),
            text,
            status,
            trigger,
            tags: normalized(&add.tags).into_iter().collect(),
            origin_session: add.origin_session.clone(),
            at,
            deferred_since,
        }
        .into(),
    );
    Ok(())
}

/// The task, if the stream holds one that is not removed.
fn live(state: &TaskState) -> Result<&TaskState, TaskRejection> {
    if state.harp_id.is_empty() {
        return Err(TaskRejection::NoTask);
    }
    if state.removed {
        return Err(TaskRejection::Removed {
            harp_id: state.harp_id.clone(),
        });
    }
    Ok(state)
}

fn required_text(text: &str) -> Result<String, TaskRejection> {
    match text.trim() {
        "" => Err(TaskRejection::TextRequired),
        trimmed => Ok(trimmed.to_string()),
    }
}

/// The trimmed status, `None` when there is none, refused when it claims
/// the status-class sigil.
fn valid_status(status: &str) -> Result<Option<String>, TaskRejection> {
    match status.trim() {
        "" => Ok(None),
        s if s.starts_with(STATUS_CLASS_SIGIL) => Err(TaskRejection::StatusReserved {
            status: s.to_string(),
        }),
        s => Ok(Some(s.to_string())),
    }
}

/// Tags trimmed, empties dropped, deduplicated and sorted.
fn normalized(tags: &[String]) -> BTreeSet<String> {
    tags.iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
