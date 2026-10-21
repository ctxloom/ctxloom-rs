//! The tasks use cases: add a task, retag one, and list a project's tasks.
//!
//! Each resolves its project through [`ProjectIdentity`], opens the
//! project's store through [`TaskStores`], and runs its command through
//! annals' kernel; a list is the task-list projector caught up from the
//! store. The ports are the only way out: this crate does no I/O, keeps no
//! clock and starts no runtime.

mod error;
mod retry;
mod view;

use std::future::Future;

use annals::next::v1::{command_page, Command, CommandBook, CommandPage, Cover};
use annals::next::{ids, AsyncEventStore, Payload, Registry, Rulebook, Stream, Upcaster};
use annals_diagnostic::Report;
use ports::{Clock, CommandIds, Delay, Jitter, Names, ProjectIdentity, TaskStores};
use task_lists::TaskList;
use tasks::{
    AddTask, Retag, TaskCommand, TaskRejection, TaskState, Tasks, DEFERRED, STATUS_CLASS_SIGIL,
    TASK_DOMAIN,
};

pub use annals::next::Timestamp;
pub use error::{diagnostic_samples, AppError, CATALOGS, EN_US};
pub use ports::Project;
pub use retry::Backoff;
pub use view::{is_terminal, text_hash, Task, ARCHIVED, DONE};

/// How many task ids an add mints before giving up, each refused because a
/// task already holds it: Go's `harp.UniqueFrom`.
pub const HARP_ATTEMPTS: u32 = 100;

/// A task to add.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct NewTask {
    /// An explicit project id, overriding resolution.
    pub project: Option<String>,
    pub text: String,
    /// Empty means To Do.
    pub status: String,
    /// Required when the status is Deferred.
    pub trigger: String,
    pub tags: Vec<String>,
    /// The session adding it, if one is.
    pub origin_session: String,
}

/// Tags to add to and remove from a task: the adds first, then the
/// removes, so a tag named in both ends up removed.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TagChange {
    pub project: Option<String>,
    pub harp_id: String,
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

/// Which tasks to list. By default the open ones: neither terminal nor
/// Deferred. `all` lists every task; naming statuses lists exactly the
/// tasks in them.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ListQuery {
    pub project: Option<String>,
    pub statuses: Vec<String>,
    pub all: bool,
}

impl ListQuery {
    /// Whether the listing holds `task`.
    pub fn admits(&self, task: &TaskState) -> bool {
        if self.all {
            return true;
        }
        if !self.statuses.is_empty() {
            return self.statuses.contains(&task.status);
        }
        !is_terminal(&task.status) && task.status != DEFERRED
    }
}

/// A task written, and the project it was written in.
#[derive(Clone, PartialEq, Debug)]
pub struct Written {
    pub project: Project,
    pub task: Task,
}

/// A project's tasks, in the order they were added; no project when the
/// work root has none yet.
#[derive(Clone, PartialEq, Debug)]
pub struct Listing {
    pub project: Option<Project>,
    pub tasks: Vec<Task>,
}

/// The use cases, as a driving adapter calls them. Their futures are
/// `Send`, so a server can spawn them.
pub trait TaskService {
    fn add(&mut self, task: NewTask) -> impl Future<Output = Result<Written, AppError>> + Send;
    fn tag(&mut self, change: TagChange) -> impl Future<Output = Result<Written, AppError>> + Send;
    fn list(&mut self, query: ListQuery) -> impl Future<Output = Result<Listing, AppError>> + Send;
}

/// The registry the tasks domain runs under.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register::<Tasks>(Rulebook::new("v1", ()), Upcaster::new("tasks", "tasks"))
        .expect("the tasks domain registers once");
    registry
}

/// The use cases over their ports: `clock` stamps commands and mints their
/// ids; `delay` waits, with jitter, between retries.
pub struct Service<I, S, N, C, D> {
    pub identity: I,
    pub stores: S,
    pub names: N,
    pub clock: C,
    pub delay: D,
    pub backoff: Backoff,
    registry: Registry,
}

impl<I, S, N, C, D> Service<I, S, N, C, D> {
    pub fn new(identity: I, stores: S, names: N, clock: C, delay: D) -> Self {
        Service {
            identity,
            stores,
            names,
            clock,
            delay,
            backoff: Backoff::DEFAULT,
            registry: registry(),
        }
    }
}

impl<I, S, N, C, D> TaskService for Service<I, S, N, C, D>
where
    I: ProjectIdentity + Send,
    S: TaskStores + Send,
    N: Names + Send,
    C: Clock + CommandIds + Send,
    D: Delay + Jitter + Send,
{
    async fn add(&mut self, task: NewTask) -> Result<Written, AppError> {
        let project = self
            .identity
            .for_write(task.project.as_deref())
            .map_err(AppError::Identity)?;
        let store = self.stores.open(&project).await.map_err(failed)?;
        let mut runner = retry::Runner {
            registry: &self.registry,
            clock: &mut self.clock,
            delay: &mut self.delay,
            backoff: self.backoff,
        };
        for _ in 0..HARP_ATTEMPTS {
            let harp_id = self.names.task_id();
            let command_id = runner.clock.command_id();
            let add = AddTask {
                harp_id: harp_id.clone(),
                text: task.text.clone(),
                status: task.status.clone(),
                trigger: task.trigger.clone(),
                tags: task.tags.clone(),
                origin_session: task.origin_session.clone(),
            };
            let book = book(&harp_id, add.into());
            match runner.run(store, &book, &command_id).await? {
                retry::Outcome::Committed => {
                    let task = read(store, &self.registry, &harp_id)
                        .await?
                        .ok_or(AppError::Vanished { harp_id })?;
                    return Ok(Written { project, task });
                }
                retry::Outcome::Refused(TaskRejection::TaskExists { .. }) => continue,
                retry::Outcome::Refused(rejection) => return Err(rejected(&rejection)),
            }
        }
        Err(AppError::HarpsExhausted {
            attempts: HARP_ATTEMPTS,
        })
    }

    async fn tag(&mut self, change: TagChange) -> Result<Written, AppError> {
        // A change to an existing task resolves its project as a read does:
        // where there is none, or no store yet, there is no task to change,
        // and nothing is minted or created to say so.
        let project = self
            .identity
            .for_read(change.project.as_deref())
            .map_err(AppError::Identity)?
            .ok_or(AppError::NoProject)?;
        if self
            .stores
            .open_existing(&project)
            .await
            .map_err(failed)?
            .is_none()
        {
            return Err(rejected(&TaskRejection::NoTask));
        }
        let store = self.stores.open(&project).await.map_err(failed)?;
        let command_id = self.clock.command_id();
        let retag = Retag {
            add: change.add,
            remove: change.remove,
        };
        let book = book(&change.harp_id, retag.into());
        let mut runner = retry::Runner {
            registry: &self.registry,
            clock: &mut self.clock,
            delay: &mut self.delay,
            backoff: self.backoff,
        };
        match runner.run(store, &book, &command_id).await? {
            retry::Outcome::Committed => {}
            retry::Outcome::Refused(rejection) => return Err(rejected(&rejection)),
        }
        let task =
            read(store, &self.registry, &change.harp_id)
                .await?
                .ok_or(AppError::Vanished {
                    harp_id: change.harp_id,
                })?;
        Ok(Written { project, task })
    }

    async fn list(&mut self, query: ListQuery) -> Result<Listing, AppError> {
        // `@open` and `@terminal` will name classes of statuses; until they
        // do, a status starting with the sigil is refused, as a write
        // refuses it, rather than matching no task.
        if let Some(class) = query
            .statuses
            .iter()
            .map(|s| s.trim())
            .find(|s| s.starts_with(STATUS_CLASS_SIGIL))
        {
            return Err(rejected(&TaskRejection::StatusReserved {
                status: class.to_string(),
            }));
        }
        let Some(project) = self
            .identity
            .for_read(query.project.as_deref())
            .map_err(AppError::Identity)?
        else {
            return Ok(Listing {
                project: None,
                tasks: Vec::new(),
            });
        };
        let Some(store) = self.stores.open_existing(&project).await.map_err(failed)? else {
            return Ok(Listing {
                project: Some(project),
                tasks: Vec::new(),
            });
        };
        let admitted = query.clone();
        let mut list = TaskList::new(Box::new(move |task| admitted.admits(task)));
        list.catch_up_async(store, &self.registry)
            .await
            .map_err(|e| AppError::Failed(Report::of(&*e)))?;
        let mut tasks: Vec<Task> = list.entries().into_iter().map(Task::of).collect();
        tasks.sort_by(|a, b| {
            let added = |t: &Task| (t.created_at.seconds, t.created_at.nanos);
            added(a)
                .cmp(&added(b))
                .then_with(|| a.harp_id.cmp(&b.harp_id))
        });
        Ok(Listing {
            project: Some(project),
            tasks,
        })
    }
}

/// The stream of the task `harp_id` names.
fn cover(harp_id: &str) -> Cover {
    Cover {
        domain: TASK_DOMAIN.to_string(),
        root: ids::compute_root(TASK_DOMAIN, harp_id).ok(),
        edition: None,
    }
}

/// `command` for the task `harp_id`. Its command id is left for the
/// kernel to take from the [`Sampled`](annals::next::Sampled) the runner
/// gives it, the same on every attempt.
fn book(harp_id: &str, command: TaskCommand) -> CommandBook {
    CommandBook {
        cover: Some(cover(harp_id)),
        page: Some(CommandPage {
            body: Some(command_page::Body::Command(Command {
                payload: Some(command.to_any()),
            })),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

/// The task `harp_id` names, as the store holds it now.
async fn read<S: AsyncEventStore>(
    store: &mut S,
    registry: &Registry,
    harp_id: &str,
) -> Result<Option<Task>, AppError> {
    let cover = cover(harp_id);
    let Some(slice) = store.read_from(&cover, 0).await.map_err(failed)? else {
        return Ok(None);
    };
    let stream = Stream::<Tasks>::replay(cover, &slice.pages, registry)
        .map_err(|e| AppError::Failed(Report::of(&e)))?;
    Ok(Some(Task::of(stream.state())))
}

fn failed(error: annals::next::StoreError) -> AppError {
    AppError::Failed(Report::of(&error))
}

fn rejected(rejection: &TaskRejection) -> AppError {
    AppError::Rejected(Report::of(rejection))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "fakes.test.rs"]
mod fakes;
