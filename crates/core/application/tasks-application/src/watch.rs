//! Watching a project's tasks: the task-list projector over every task,
//! caught up from the project's store on each poll, its changes since the
//! last poll the answer. The cursor is the projector's own sequence, so a
//! poll that finds nothing new answers nothing.
//!
//! A poll re-reads the store's task streams; the projector skips what it
//! has already projected. A cheaper test for "anything committed since"
//! (SQLite's `PRAGMA data_version`) needs a call annals-store-sql does not
//! offer yet.

use std::future::Future;

use annals_diagnostic::Report;
use ports::{Project, ProjectIdentity, TaskStores};
use task_lists::{Change, TaskList};

use crate::{failed, AppError, Service, Task};

/// Which project to watch: `project` pins one, overriding resolution.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WatchRequest {
    pub project: Option<String>,
}

/// One change to a watched project's tasks.
#[derive(Clone, PartialEq, Debug)]
pub enum Changed {
    /// A task was added, or changed: the task now.
    Task(Task),
    /// A task was removed.
    Removed { harp_id: String },
}

/// A project being watched: every task in it as of the last poll.
pub struct Watch {
    pub project: Project,
    list: TaskList,
    /// The list's sequence the last poll answered up to.
    cursor: u64,
}

impl Watch {
    /// A watch of `project` that has seen nothing yet: the first poll
    /// answers every task.
    pub fn of(project: Project) -> Watch {
        Watch {
            project,
            list: TaskList::new(Box::new(|_| true)),
            cursor: 0,
        }
    }
}

/// Watching, as a driving adapter calls it. The futures are `Send`, as
/// [`crate::TaskService`]'s are.
pub trait WatchService {
    /// Start watching the project `request` names, caught up with its
    /// store as it is now; `None` when the work root has no project yet.
    /// Resolving never mints: a watch binds no project.
    fn watch(
        &mut self,
        request: WatchRequest,
    ) -> impl Future<Output = Result<Option<Watch>, AppError>> + Send;

    /// The changes committed to `watch`'s project since the last poll (or
    /// since it began), in the order the projector saw them; none while
    /// the project has no store.
    fn poll(
        &mut self,
        watch: &mut Watch,
    ) -> impl Future<Output = Result<Vec<Changed>, AppError>> + Send;
}

impl<I, S, N, C, D, L, G> WatchService for Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity + Send,
    S: TaskStores + Send,
    N: Send,
    C: Send,
    D: Send,
    L: Send,
    G: Send,
{
    async fn watch(&mut self, request: WatchRequest) -> Result<Option<Watch>, AppError> {
        let Some(project) = self
            .identity
            .for_read(request.project.as_deref())
            .map_err(AppError::Identity)?
        else {
            return Ok(None);
        };
        let mut watch = Watch::of(project);
        self.poll(&mut watch).await?;
        Ok(Some(watch))
    }

    async fn poll(&mut self, watch: &mut Watch) -> Result<Vec<Changed>, AppError> {
        let Some(store) = self
            .stores
            .open_existing(&watch.project)
            .await
            .map_err(failed)?
        else {
            return Ok(Vec::new());
        };
        watch
            .list
            .catch_up_async(store, &self.registry)
            .await
            .map_err(|e| AppError::Failed(Report::of(&*e)))?;
        let changes = watch
            .list
            .since(watch.cursor)
            .unwrap_or_default()
            .iter()
            .map(|delta| match &delta.change {
                Change::Upsert(state) => Changed::Task(Task::of(state)),
                Change::Remove { harp_id } => Changed::Removed {
                    harp_id: harp_id.clone(),
                },
            })
            .collect();
        watch.cursor = watch.list.sequence();
        Ok(changes)
    }
}

#[cfg(test)]
#[path = "watch.test.rs"]
mod tests;
