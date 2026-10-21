//! The tasks use cases: add a task, move, edit or retag one, read a
//! project's tasks (or every project's), watch them change, prune old
//! finished ones, and import Go taskloom's logs.
//!
//! Each resolves its project through [`ProjectIdentity`], opens the
//! project's store through [`TaskStores`], and runs its command through
//! annals' kernel; a list is the task-list projector caught up from the
//! store. The ports are the only way out: this crate does no I/O, keeps no
//! clock and starts no runtime.

mod error;
mod import;
mod lint;
mod priority;
mod prune;
mod query;
mod read;
mod retry;
mod schema;
mod semver;
mod tags;
mod view;
mod watch;

use std::future::Future;

use annals::next::v1::{command_page, Command, CommandBook, CommandPage, Cover};
use annals::next::{
    ids, AsyncReadStore, AsyncReadUnit, Payload, Registry, Rulebook, Stream, Upcaster,
};
use annals_diagnostic::Report;
use ports::{Clock, CommandIds, Configuration, Delay, Jitter, Names, ProjectIdentity, TaskStores};
use tasks::{AddTask, EditText, Retag, SetStatus, TaskCommand, TaskRejection, Tasks, TASK_DOMAIN};

pub use annals::next::Timestamp;
pub use error::{diagnostic_samples, AppError, CATALOGS, EN_US};
pub use import::{
    anomaly_key, record_command_id, rfc3339_nano, ImportReport, ImportRequest, ImportService, Lost,
    NoGoLogs, ProjectImport, Why,
};
pub use lint::{Finding, LintReport, Violation, SCHEMA_SUBJECT};
pub use ports::{GoOp, Project};
pub use priority::{PriorityNote, PriorityWarning};
pub use prune::{
    cut, Dangling, PruneRefusal, PruneRequest, PruneService, Pruned, Pruning, Via,
    DEFAULT_OLDER_THAN, DEFAULT_PRUNE_STATUSES,
};
pub use query::{QueryError, QUERY_GRAMMAR};
pub use read::{Details, Hidden, Listing, Reach, Statuses, Summary, TagCount, Vocabulary};
pub use retry::Backoff;
pub use schema::{
    Range, SchemaError, TagSchema, DEFAULT_TAG_SCHEMA, FACETS, FACET_NAMESPACE, SEMVER,
};
pub use semver::{Semver, Version};
pub use tags::{admit, collapse, parse as parse_tag, Tag, TagRefusal, OPERATOR_WORDS};
pub use view::{
    is_active, is_terminal, text_hash, Row, Shown, StatusInfo, Task, ARCHIVED, DEFAULT_STATUSES,
    DONE, IN_PROGRESS, OPEN, STATUS_CLASSES, TERMINAL,
};
pub use watch::{Changed, Watch, WatchRequest, WatchService};

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
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// Tags to add to and remove from a task: the adds first, then the
/// removes, so a tag named in both ends up removed. Each is a tag in
/// tagma's grammar; one that does not parse refuses the change, as does an
/// added one the tag schema refuses. An added value of a scalar target
/// displaces the value the task holds.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TagChange {
    pub project: Option<String>,
    pub harp_id: String,
    pub add: Vec<String>,
    pub remove: Vec<String>,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// A task moved to a status. An empty trigger keeps the one it holds;
/// Deferred needs one, given or held.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct StatusChange {
    pub project: Option<String>,
    pub harp_id: String,
    pub status: String,
    pub trigger: String,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// A task's text replaced whole; its status and trigger untouched.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TextEdit {
    pub project: Option<String>,
    pub harp_id: String,
    pub text: String,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// The tasks to show, by id, every one of which must be found: in the
/// work root's project, or with `global` in every project.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ShowQuery {
    pub project: Option<String>,
    pub global: bool,
    pub harp_ids: Vec<String>,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// Where a read that takes no filter reads: the work root's project (or
/// the one pinned), or with `global` every project.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Scope {
    pub project: Option<String>,
    pub global: bool,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// Which tasks to list. By default the active ones: neither terminal nor
/// Deferred. `all` lists every task; naming statuses (or the classes
/// `@open` and `@terminal`) lists exactly the tasks in them. A term keeps
/// those whose text mentions it, case-insensitively, and a tag query those
/// it selects. `global` reads every project.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ListQuery {
    pub project: Option<String>,
    pub statuses: Vec<String>,
    pub all: bool,
    /// A tag query, postfix or infix (see [`QUERY_GRAMMAR`]); empty for
    /// none.
    pub tag_query: String,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
    /// A substring of the text, any case; empty for none.
    pub term: String,
    /// Every project's tasks, not the work root's project's.
    pub global: bool,
    /// At most this many tasks, the rest counted as omitted; 0 for no
    /// limit.
    pub limit: usize,
    /// The order: empty for the listing's own, or [`PRIORITY`], Go's one
    /// sort.
    pub sort: String,
}

/// The sort by derived priority, Go's `--sort priority`.
pub const PRIORITY: &str = "priority";

/// A task written, the project it was written in, and which of its tags a
/// person is shown.
#[derive(Clone, PartialEq, Debug)]
pub struct Written {
    pub project: Project,
    pub task: Task,
    pub shown: Shown,
}

/// The use cases, as a driving adapter calls them. Their futures are
/// `Send`, so a server can spawn them.
pub trait TaskService {
    fn add(&mut self, task: NewTask) -> impl Future<Output = Result<Written, AppError>> + Send;
    fn tag(&mut self, change: TagChange) -> impl Future<Output = Result<Written, AppError>> + Send;
    fn list(&mut self, query: ListQuery) -> impl Future<Output = Result<Listing, AppError>> + Send;
    fn set_status(
        &mut self,
        change: StatusChange,
    ) -> impl Future<Output = Result<Written, AppError>> + Send;
    fn edit(&mut self, edit: TextEdit) -> impl Future<Output = Result<Written, AppError>> + Send;
    fn show(&mut self, query: ShowQuery) -> impl Future<Output = Result<Details, AppError>> + Send;
    fn summary(&mut self, scope: Scope) -> impl Future<Output = Result<Summary, AppError>> + Send;
    fn statuses(&mut self, scope: Scope)
        -> impl Future<Output = Result<Statuses, AppError>> + Send;
    fn tags(
        &mut self,
        query: ListQuery,
    ) -> impl Future<Output = Result<Vocabulary, AppError>> + Send;
    fn lint(&mut self, scope: Scope) -> impl Future<Output = Result<LintReport, AppError>> + Send;
    /// The tag schema a write is held to: the default one, extended by
    /// what is configured, `config_set` applied. No project is resolved.
    fn tag_schema(
        &mut self,
        config_set: Vec<String>,
    ) -> impl Future<Output = Result<TagSchema, AppError>> + Send;
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
/// ids; `delay` waits, with jitter, between retries; `go_logs` are the Go
/// logs an import reads, none until [`Service::with_go_logs`] gives some;
/// `configuration` is what is configured, nothing until
/// [`Service::with_configuration`] gives it.
pub struct Service<I, S, N, C, D, L = NoGoLogs, G = NotConfigured> {
    pub identity: I,
    pub stores: S,
    pub names: N,
    pub clock: C,
    pub delay: D,
    pub go_logs: L,
    pub configuration: G,
    pub backoff: Backoff,
    registry: Registry,
}

/// Configuration, as a service without any has it: nothing configured, so
/// the tag schema is the default one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NotConfigured;

impl Configuration for NotConfigured {
    // Equivalent: `Ok(vec![])` is the empty list this returns.
    #[cfg_attr(test, mutants::skip)]
    fn tag_schema(&mut self, _: &[String]) -> Result<Vec<String>, Report> {
        Ok(Vec::new())
    }
}

impl<I, S, N, C, D> Service<I, S, N, C, D> {
    pub fn new(identity: I, stores: S, names: N, clock: C, delay: D) -> Self {
        Service {
            identity,
            stores,
            names,
            clock,
            delay,
            go_logs: NoGoLogs,
            configuration: NotConfigured,
            backoff: Backoff::DEFAULT,
            registry: registry(),
        }
    }
}

impl<I, S, N, C, D, L, G> Service<I, S, N, C, D, L, G> {
    /// The same service, importing from `go_logs`.
    pub fn with_go_logs<M>(self, go_logs: M) -> Service<I, S, N, C, D, M, G> {
        Service {
            identity: self.identity,
            stores: self.stores,
            names: self.names,
            clock: self.clock,
            delay: self.delay,
            go_logs,
            configuration: self.configuration,
            backoff: self.backoff,
            registry: self.registry,
        }
    }

    /// The same service, configured by `configuration`.
    pub fn with_configuration<H>(self, configuration: H) -> Service<I, S, N, C, D, L, H> {
        Service {
            identity: self.identity,
            stores: self.stores,
            names: self.names,
            clock: self.clock,
            delay: self.delay,
            go_logs: self.go_logs,
            configuration,
            backoff: self.backoff,
            registry: self.registry,
        }
    }
}

impl<I, S, N, C, D, L, G: Configuration> Service<I, S, N, C, D, L, G> {
    /// The tag schema: the default one, extended by what is configured,
    /// `overrides` applied.
    fn schema(&mut self, overrides: &[String]) -> Result<TagSchema, AppError> {
        let configured = self
            .configuration
            .tag_schema(overrides)
            .map_err(AppError::Configuration)?;
        TagSchema::resolved(&configured).map_err(|e| AppError::Configuration(Report::of(&e)))
    }
}

impl<I, S, N, C, D, L, G> Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity,
    S: TaskStores,
    C: Clock + CommandIds,
    D: Delay + Jitter,
{
    /// The project of a change to an existing task, resolved as a read
    /// resolves it: where there is none, or no store yet, there is no task
    /// to change, and nothing is minted or created to say so.
    async fn existing(&mut self, pinned: Option<&str>) -> Result<Project, AppError> {
        let project = self
            .identity
            .for_read(pinned)
            .map_err(AppError::Identity)?
            .ok_or(AppError::NoProject)?;
        match self.stores.open_existing(&project).await.map_err(failed)? {
            Some(_) => Ok(project),
            None => Err(rejected(&TaskRejection::NoTask)),
        }
    }

    /// Run `command` on the task `harp_id` in `project`'s store, under a
    /// fresh command id, retried as annals says; the task after it.
    async fn commit(
        &mut self,
        project: &Project,
        harp_id: &str,
        command: TaskCommand,
    ) -> Result<Task, AppError> {
        let store = self.stores.open(project).await.map_err(failed)?;
        let command_id = self.clock.command_id();
        let book = book(harp_id, command);
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
        read(store, &self.registry, harp_id)
            .await?
            .ok_or_else(|| AppError::Vanished {
                harp_id: harp_id.to_string(),
            })
    }
}

impl<I, S, N, C, D, L, G> TaskService for Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity + Send,
    S: TaskStores + Send,
    N: Names + Send,
    C: Clock + CommandIds + Send,
    D: Delay + Jitter + Send,
    L: Send,
    G: Configuration + Send,
{
    async fn add(&mut self, task: NewTask) -> Result<Written, AppError> {
        let schema = self.schema(&task.config_set)?;
        let tags = tags::admit_all(&task.tags, &schema).map_err(|r| refused(&r))?;
        let (tags, _) = tags::collapse(&schema, &[], tags);
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
                tags: tags.clone(),
                origin_session: task.origin_session.clone(),
            };
            let book = book(&harp_id, add.into());
            match runner.run(store, &book, &command_id).await? {
                retry::Outcome::Committed => {
                    let task = read(store, &self.registry, &harp_id)
                        .await?
                        .ok_or(AppError::Vanished { harp_id })?;
                    let shown = Shown::of(&schema);
                    return Ok(Written {
                        project,
                        task,
                        shown,
                    });
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
        // Nothing to add or remove is refused before anything is read, as
        // Go's `tag` refuses it.
        if change
            .add
            .iter()
            .chain(&change.remove)
            .all(|t| t.trim().is_empty())
        {
            return Err(rejected(&TaskRejection::TagRequired));
        }
        let schema = self.schema(&change.config_set)?;
        let add = tags::admit_all(&change.add, &schema).map_err(|r| refused(&r))?;
        let mut remove = tags::parse_all(&change.remove).map_err(|r| refused(&r))?;
        let project = self.existing(change.project.as_deref()).await?;
        let store = self.stores.open(&project).await.map_err(failed)?;
        // A scalar target's new value displaces the one the task holds:
        // resolved here, against the task as it is, so decide never reads
        // the schema.
        let held = read(store, &self.registry, &change.harp_id)
            .await?
            .map(|task| task.tags)
            .unwrap_or_default();
        let (add, displaced) = tags::collapse(&schema, &held, add);
        remove.extend(displaced);
        let retag = Retag { add, remove };
        let task = self.commit(&project, &change.harp_id, retag.into()).await?;
        Ok(Written {
            project,
            task,
            shown: Shown::of(&schema),
        })
    }

    async fn set_status(&mut self, change: StatusChange) -> Result<Written, AppError> {
        let schema = self.schema(&change.config_set)?;
        let project = self.existing(change.project.as_deref()).await?;
        let set = SetStatus {
            status: change.status,
            trigger: change.trigger,
        };
        let task = self.commit(&project, &change.harp_id, set.into()).await?;
        Ok(Written {
            project,
            task,
            shown: Shown::of(&schema),
        })
    }

    async fn edit(&mut self, edit: TextEdit) -> Result<Written, AppError> {
        let schema = self.schema(&edit.config_set)?;
        let project = self.existing(edit.project.as_deref()).await?;
        let command = EditText { text: edit.text };
        let task = self.commit(&project, &edit.harp_id, command.into()).await?;
        Ok(Written {
            project,
            task,
            shown: Shown::of(&schema),
        })
    }

    async fn list(&mut self, query: ListQuery) -> Result<Listing, AppError> {
        self.listing(query).await
    }

    async fn show(&mut self, query: ShowQuery) -> Result<Details, AppError> {
        self.details(query).await
    }

    async fn summary(&mut self, scope: Scope) -> Result<Summary, AppError> {
        self.summed(scope).await
    }

    async fn statuses(&mut self, scope: Scope) -> Result<Statuses, AppError> {
        self.taxonomy(scope).await
    }

    async fn tags(&mut self, query: ListQuery) -> Result<Vocabulary, AppError> {
        self.vocabulary(query).await
    }

    async fn lint(&mut self, scope: Scope) -> Result<LintReport, AppError> {
        self.linted(scope).await
    }

    async fn tag_schema(&mut self, config_set: Vec<String>) -> Result<TagSchema, AppError> {
        self.schema(&config_set)
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

/// The task `harp_id` names, as the store holds it now, read in a
/// read-only unit of work; `None` when its stream holds no task.
async fn read<S: AsyncReadStore>(
    store: &mut S,
    registry: &Registry,
    harp_id: &str,
) -> Result<Option<Task>, AppError> {
    let cover = cover(harp_id);
    let mut reads = store.begin_read().await;
    let Some(slice) = reads.read_from(&cover, 0).await.map_err(failed)? else {
        return Ok(None);
    };
    let stream = Stream::<Tasks>::replay(cover, &slice.pages, registry)
        .map_err(|e| AppError::Failed(Report::of(&e)))?;
    // A stream that replays to no task (read back with no events, as a
    // store whose reads are not held to one moment can answer one purged
    // mid-read) is no task, never a blank one.
    let state = stream.state();
    Ok((!state.harp_id.is_empty()).then(|| Task::of(state)))
}

fn failed(error: annals::next::StoreError) -> AppError {
    AppError::Failed(Report::of(&error))
}

fn refused(refusal: &TagRefusal) -> AppError {
    AppError::TagRefused(Report::of(refusal))
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
