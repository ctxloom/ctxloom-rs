//! Task lists: read models a projector folds from a project's task streams.
//! A list is purely derived, so it is no aggregate: each list is a projector
//! that decides membership from each task's state after an event, and
//! numbers its own changes. A UI resumes a list from that number.
//!
//! The store is the collection: every task stream in a project's store is
//! that project's, so a list needs no membership metadata on the streams, and
//! catches up by enumerating the store's task streams.

use std::collections::BTreeMap;

use annals::next::v1::Cover;
use annals::next::{
    projection, AsyncReadStore, AsyncReadUnit, Committed, EventStore, KernelError, Projection,
    Registry, Slice, Stream,
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
    /// The last stream sequence projected, per task stream (by root), so a
    /// redelivered event changes nothing. A pruned stream's is forgotten.
    projected: BTreeMap<Vec<u8>, u64>,
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
    /// a list kept across one still names the task: whoever purges checks
    /// the stream first.
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
    /// built after a restart, or one catching up.
    pub fn catch_up<S: EventStore>(
        &mut self,
        store: &mut S,
        registry: &Registry,
    ) -> Result<(), Box<KernelError>> {
        for cover in store.covers(TASK_DOMAIN).map_err(store_error)? {
            let slice = store.read(&cover).map_err(store_error)?;
            self.project_slice(cover, slice, registry)?;
        }
        Ok(())
    }

    /// [`TaskList::catch_up`], over the async store port: a database's.
    /// The streams are listed and read in one read-only unit of work, so
    /// what is projected is one moment of the store: a write or a purge
    /// landing meanwhile, through any handle, is not seen.
    pub async fn catch_up_async<S: AsyncReadStore>(
        &mut self,
        store: &mut S,
        registry: &Registry,
    ) -> Result<(), Box<KernelError>> {
        let mut reads = store.begin_read().await;
        for cover in reads.covers(TASK_DOMAIN).await.map_err(store_error)? {
            let slice = reads.read_from(&cover, 0).await.map_err(store_error)?;
            self.project_slice(cover, slice, registry)?;
        }
        Ok(())
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
        let root = cover
            .root
            .as_ref()
            .map(|r| r.value.to_vec())
            .unwrap_or_default();
        let sequence = committed.sequence();
        if self
            .projected
            .get(&root)
            .is_some_and(|&last| sequence <= last)
        {
            return;
        }
        // A pruned stream is purged after its removal is projected, and a
        // task minted on the freed harp starts it again from its first
        // sequence: forget where it was, so that task is not skipped as
        // already seen.
        let task = committed.state;
        if is_pruned(committed.event()) {
            self.projected.remove(&root);
            self.unpurged.insert(task.harp_id.clone(), task.clone());
        } else {
            self.projected.insert(root, sequence);
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

/// Whether `event` is a prune's removal: the last its stream holds before
/// it is purged.
fn is_pruned(event: &TaskEvent) -> bool {
    matches!(event, TaskEvent::TaskRemoved(removed) if removed.reason == RemovalReason::Pruned)
}

fn store_error(error: annals::next::StoreError) -> Box<KernelError> {
    Box::new(KernelError::Store(error))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
