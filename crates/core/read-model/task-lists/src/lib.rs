//! Task lists: read models a projector folds from a project's task streams.
//! A list is purely derived, so it is no aggregate: each list is a projector
//! that decides membership from each task's state after an event, and
//! numbers its own changes. A UI resumes a list from that number.
//!
//! The store is the collection: every task stream in a project's store is
//! that project's, so a list needs no membership metadata on the streams, and
//! catches up by enumerating the store's task streams.

use std::collections::{BTreeMap, BTreeSet};

use annals::next::v1::{Cover, Uuid};
use annals::next::{
    ids, projection, AsyncReadStore, AsyncReadUnit, Committed, Envelope, EventStore, KernelError,
    Projection, Registry, Slice, Stream, Timestamp,
};
use tasks::{RemovalReason, TaskEvent, TaskState, Tasks, TASK_DOMAIN};

/// Which live tasks a list holds.
pub type Membership = Box<dyn Fn(&TaskState) -> bool + Send>;

/// One change to a list.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// A task entered the list, or a listed task changed: its state now.
    Upsert(TaskState),
    /// A task left the list.
    Remove { harp_id: String },
}

/// A change and the list's sequence it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Delta {
    pub sequence: u64,
    pub change: Change,
}

/// One list of a project's tasks.
pub struct TaskList {
    membership: Membership,
    /// The listed tasks, by harp id.
    entries: BTreeMap<String, TaskState>,
    /// Every change, in order: the change numbered `n` is at `n - 1`.
    deltas: Vec<Delta>,
    /// How far each task stream (by root) is projected, and which stream
    /// it was: a redelivered event changes nothing, and a stream started
    /// again on a freed harp is seen as new.
    projected: BTreeMap<Vec<u8>, Seen>,
    /// The tasks whose stream's last projected event is a prune's removal,
    /// by harp id: removed, and not yet seen purged.
    unpurged: BTreeMap<String, TaskState>,
}

impl TaskList {
    /// The list of the live tasks `membership` holds.
    pub fn new(membership: Membership) -> TaskList {
        TaskList {
            membership,
            entries: BTreeMap::new(),
            deltas: Vec::new(),
            projected: BTreeMap::new(),
            unpurged: BTreeMap::new(),
        }
    }

    /// The listed tasks, by harp id.
    pub fn entries(&self) -> Vec<&TaskState> {
        self.entries.values().collect()
    }

    /// The tasks a prune removed whose streams were still held when last
    /// read, by harp id: a prune left half done, its purge failed after its
    /// removal landed, for a prune to finish. A purge records no event, so
    /// a list kept across one still names the task until it next catches
    /// up and finds the stream gone: whoever purges checks the stream
    /// first.
    pub fn unpurged(&self) -> Vec<&TaskState> {
        self.unpurged.values().collect()
    }

    /// The list's sequence: the number of its newest change, 0 before any.
    pub fn sequence(&self) -> u64 {
        self.deltas.len() as u64
    }

    /// The changes after `cursor`, in order; `None` for a cursor past the
    /// list's sequence, which the reader resolves by taking the list whole.
    pub fn since(&self, cursor: u64) -> Option<&[Delta]> {
        self.deltas.get(usize::try_from(cursor).ok()?..)
    }

    /// Project every task stream in `store` not yet projected: a fresh list
    /// built after a restart, or one catching up. A task whose stream the
    /// store no longer holds (pruned whole since the last catch-up, its
    /// removal never seen) leaves the list, and its stream is forgotten,
    /// so a task minted on its harp later is seen from its start.
    pub fn catch_up<S: EventStore>(
        &mut self,
        store: &mut S,
        registry: &Registry,
    ) -> Result<(), Box<KernelError>> {
        let covers = store.covers(TASK_DOMAIN).map_err(store_error)?;
        let held = roots(&covers);
        for cover in covers {
            let slice = store.read(&cover).map_err(store_error)?;
            self.project_slice(cover, slice, registry)?;
        }
        self.forget_gone(&held);
        Ok(())
    }

    /// [`TaskList::catch_up`], over the async store port: a database's.
    /// The streams are listed and read in one read-only unit of work, so
    /// what is projected is one moment of the store: a write or a purge
    /// landing meanwhile, through any handle, is not seen. A task whose
    /// stream is gone leaves the list, as in [`TaskList::catch_up`].
    pub async fn catch_up_async<S: AsyncReadStore>(
        &mut self,
        store: &mut S,
        registry: &Registry,
    ) -> Result<(), Box<KernelError>> {
        let mut reads = store.begin_read().await;
        let covers = reads.covers(TASK_DOMAIN).await.map_err(store_error)?;
        let held = roots(&covers);
        for cover in covers {
            let slice = reads.read_from(&cover, 0).await.map_err(store_error)?;
            self.project_slice(cover, slice, registry)?;
        }
        self.forget_gone(&held);
        Ok(())
    }

    /// Forget every task whose stream the store no longer holds (`held`
    /// are the streams it does, by root): a prune that landed whole, its
    /// removal and its purge, since this list last read. Each such task
    /// the list held leaves it, as its removal would have taken it.
    fn forget_gone(&mut self, held: &BTreeSet<Vec<u8>>) {
        let gone = |harp: &str| !held.contains(&root_of(harp));
        self.projected.retain(|root, _| held.contains(root));
        self.unpurged.retain(|harp, _| !gone(harp));
        let left: Vec<String> = self.entries.keys().filter(|h| gone(h)).cloned().collect();
        for harp_id in left {
            self.entries.remove(&harp_id);
            self.record(Change::Remove { harp_id });
        }
    }

    /// Project one task stream as read, if the store held it.
    fn project_slice(
        &mut self,
        cover: Cover,
        slice: Option<Slice>,
        registry: &Registry,
    ) -> Result<(), Box<KernelError>> {
        let Some(slice) = slice else {
            return Ok(());
        };
        let stream = Stream::<Tasks>::replay(cover, &slice.pages, registry)?;
        stream.dispatch(0, registry, &mut [&mut projection(self)])?;
        Ok(())
    }

    fn record(&mut self, change: Change) {
        let sequence = self.sequence() + 1;
        self.deltas.push(Delta { sequence, change });
    }
}

impl Projection<Tasks> for TaskList {
    fn project(&mut self, committed: &Committed<'_, Tasks>) {
        let cover = committed.cover();
        // Lists show the main timeline; a what-if edition is not the project.
        if cover.edition.is_some() {
            return;
        }
        let root = root(cover);
        let sequence = committed.sequence();
        // A pruned stream is purged, and a task minted on the freed harp
        // starts it again from sequence 0, perhaps all between two
        // catch-ups: a first event not the one seen first is a new stream,
        // projected from its start (its first state replaces the old
        // task's), not skipped as already seen.
        let first = (sequence == 0).then(|| Fingerprint::of(committed.envelope()));
        if let Some(seen) = self.projected.get(&root) {
            let restarted = first.is_some() && first != seen.first;
            if !restarted && sequence <= seen.last {
                return;
            }
        }
        let first = first.or_else(|| self.projected.get(&root)?.first.clone());
        self.projected.insert(
            root,
            Seen {
                first,
                last: sequence,
            },
        );
        let task = committed.state;
        if is_pruned(committed.event()) {
            self.unpurged.insert(task.harp_id.clone(), task.clone());
        } else {
            self.unpurged.remove(&task.harp_id);
        }

        let listed = !task.removed && (self.membership)(task);
        if listed {
            if self.entries.get(&task.harp_id) != Some(task) {
                self.entries.insert(task.harp_id.clone(), task.clone());
                self.record(Change::Upsert(task.clone()));
            }
        } else if self.entries.remove(&task.harp_id).is_some() {
            self.record(Change::Remove {
                harp_id: task.harp_id.clone(),
            });
        }
    }
}

/// How far one task stream is projected.
struct Seen {
    /// Its first event's fingerprint; none when the list began projecting
    /// it past its start (live, never caught up), so its first event, if
    /// it comes, is projected again.
    first: Option<Fingerprint>,
    /// The last sequence projected.
    last: u64,
}

/// What tells one stream on a harp from another: its first event's cause
/// (the command that minted the task, or the correlation of a process's
/// event) and when it was recorded. Both are on every recorded event and
/// read back as written; a stream minted again is a new command at a
/// later time. The sequence cannot tell them apart (each starts at 0),
/// nor the payload (a task minted again may be the same).
#[derive(Clone, PartialEq, Debug)]
struct Fingerprint {
    cause: Uuid,
    recorded_at: Timestamp,
}

impl Fingerprint {
    fn of(envelope: &Envelope<TaskEvent>) -> Fingerprint {
        Fingerprint {
            cause: envelope
                .command_id()
                .unwrap_or(envelope.correlation_id())
                .clone(),
            recorded_at: envelope.recorded_at(),
        }
    }
}

/// Whether `event` is a prune's removal: the last its stream holds before
/// it is purged.
fn is_pruned(event: &TaskEvent) -> bool {
    matches!(event, TaskEvent::TaskRemoved(removed) if removed.reason == RemovalReason::Pruned)
}

/// The roots of the main-timeline streams among `covers`.
fn roots(covers: &[Cover]) -> BTreeSet<Vec<u8>> {
    covers
        .iter()
        .filter(|cover| cover.edition.is_none())
        .map(root)
        .collect()
}

/// A task stream's root, as the list keys its streams by.
fn root(cover: &Cover) -> Vec<u8> {
    cover
        .root
        .as_ref()
        .map(|r| r.value.to_vec())
        .unwrap_or_default()
}

/// The root of the stream of the task `harp_id` names.
fn root_of(harp_id: &str) -> Vec<u8> {
    ids::compute_root(TASK_DOMAIN, harp_id)
        .map(|r| r.value.to_vec())
        .unwrap_or_default()
}

fn store_error(error: annals::next::StoreError) -> Box<KernelError> {
    Box::new(KernelError::Store(error))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
