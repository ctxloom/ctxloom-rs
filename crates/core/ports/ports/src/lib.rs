//! The ports the tasks application needs from outside, beyond annals' own
//! store port: which project a call is about, where its store is, how ids
//! are minted, whether its store can purge a stream or say whether
//! anything changed, what time it is, how to wait, what is configured, Go
//! taskloom's logs to import, what a directory's identity rests on, to show
//! and repair it, whether a project's database is sound, and agents' MCP
//! configurations, to register a server in. Each is a trait a driven
//! adapter (or the composition root) implements; nothing here does I/O.
//!
//! Every future a port returns is `Send`, as annals requires of its async
//! store port, so a server on a multi-threaded runtime can spawn a use case
//! generic over them (`tokio::spawn`, an MCP server's handlers).

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use annals::next::v1::Uuid;
use annals::next::{
    AsyncPurge, AsyncReadStore, AsyncStore, AsyncUnitOfWork, ChangeToken, StoreError, Timestamp,
};
use annals_diagnostic::Report;

mod agents;
mod project;

pub use agents::{AgentConfigs, McpServer, Rewritten, Scope};
pub use project::{Change, Checked, IdSource, ProjectAdmin, Registered, StoreCheck, Survey};

/// A resolved project: its id, the work root it was resolved from, and
/// where its store is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The project id ("jolly-mute-magma").
    pub id: String,
    /// The work root the id was resolved for: canonical, symlinks resolved.
    pub root: String,
    /// The project's database.
    pub store: PathBuf,
}

/// Which project a call is about.
///
/// Synchronous, and it blocks: resolving reads the filesystem, and a write
/// that binds a new project waits on the registry's file lock (`flock`). An
/// async server calls it through `spawn_blocking` (or its runtime's
/// equivalent), never directly on a runtime thread.
pub trait ProjectIdentity {
    /// The project a write goes to: `pinned` (an explicit id) if given,
    /// else the one already bound to the work root, else a new one, minted
    /// and bound.
    fn for_write(&mut self, pinned: Option<&str>) -> Result<Project, Report>;

    /// The project a read is about, never minting: `None` when the work
    /// root has no project yet.
    fn for_read(&mut self, pinned: Option<&str>) -> Result<Option<Project>, Report>;

    /// Every project with a directory of its own under the projects
    /// directory, by id: what a read across every project (`--global`)
    /// spans. A project's root is the path the registry first holds for
    /// it, canonical, or empty where it holds none.
    fn every_project(&mut self) -> Result<Vec<Project>, Report>;

    /// The working directory, canonical, when it lies in no work root of
    /// its own: no CTXLOOM_ROOT, and no git repository above it. A read
    /// that finds no project there reads every project instead, as Go's
    /// does; inside a work root (`None`) it reads none.
    fn rootless_directory(&mut self) -> Result<Option<String>, Report>;
}

/// Where a project's tasks are kept.
pub trait TaskStores {
    /// A project's store: it writes in units of work, and opens read-only
    /// units ([`AsyncReadStore`]) whose reads all see one moment, so a
    /// read of many streams never mixes two.
    type Store: AsyncStore + AsyncReadStore + Send;

    /// The project's store, to write: opened (and created) on first use and
    /// held after.
    fn open(
        &mut self,
        project: &Project,
    ) -> impl Future<Output = Result<&mut Self::Store, StoreError>> + Send;

    /// The project's store, to read: `None` if it has none yet. Nothing is
    /// created, so a read of a project never written (a mistyped
    /// `--project`) leaves no trace. A store already held for a write
    /// serves it too.
    fn open_existing(
        &mut self,
        project: &Project,
    ) -> impl Future<Output = Result<Option<&mut Self::Store>, StoreError>> + Send;
}

/// What a project's store answers a watcher: annals' [`ChangeToken`],
/// and which handle on the store answered it. annals compares tokens only
/// from one handle (another connection to the same database counts
/// differently), so a token from a store opened again since is never
/// equal to one from before, whatever their change tokens say.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HeldToken {
    /// The handle that answered: a new one each time a store is opened.
    pub handle: u64,
    /// What that handle answered.
    pub change: ChangeToken,
}

/// Task stores whose stores say cheaply whether anything was committed:
/// what a watch needs of [`TaskStores`], to read every stream again only
/// when something moved. Opt-in, as annals' watch is.
pub trait WatchedStores: TaskStores {
    /// The token of `project`'s store, read through the store held for it
    /// or opened as [`TaskStores::open_existing`] opens one (nothing
    /// created) and held after, so the next read is through the handle
    /// that answered; `None` while the project has no store. Equal tokens
    /// mean nothing was committed between them, through any handle; a new
    /// token may come with nothing new to read.
    fn change_token(
        &mut self,
        project: &Project,
    ) -> impl Future<Output = Result<Option<HeldToken>, StoreError>> + Send;
}

/// A task store that can purge a stream, in a unit of work: what a prune
/// needs of [`TaskStores::Store`]. Opt-in, as annals' purge is: a store
/// that cannot purge stays append-only by type, and every store whose
/// units of work purge (annals' [`AsyncPurge`] on the store and on its
/// unit) is one.
pub trait PurgingStore: AsyncStore + AsyncPurge + Send {
    /// A unit of work that can purge, as well as read and append.
    type PurgingUnit<'s>: AsyncUnitOfWork + AsyncPurge + Send
    where
        Self: 's;

    /// A unit of work over this store, as [`AsyncStore::begin`]: what it
    /// purges lands at its commit, with what it appends.
    fn begin_purging(&mut self) -> impl Future<Output = Self::PurgingUnit<'_>> + Send;
}

impl<S> PurgingStore for S
where
    S: AsyncStore + AsyncPurge + Send,
    for<'s> S::Unit<'s>: AsyncPurge,
{
    type PurgingUnit<'s>
        = S::Unit<'s>
    where
        Self: 's;

    fn begin_purging(&mut self) -> impl Future<Output = S::Unit<'_>> + Send {
        self.begin()
    }
}

/// Fresh, random names. Uniqueness is not promised: a caller checks.
pub trait Names {
    /// A task's harp id: two words ("swift-amber").
    fn task_id(&mut self) -> String;

    /// A project's id: three words ("jolly-mute-magma").
    fn project_id(&mut self) -> String;
}

/// The instant a command is recorded at.
pub trait Clock {
    fn now(&mut self) -> Timestamp;
}

/// Fresh command ids: UUIDv7 (RFC 9562), as annals expects of an
/// application.
pub trait CommandIds {
    fn command_id(&mut self) -> Uuid;
}

/// Waiting, between retries.
pub trait Delay {
    fn sleep(&mut self, duration: Duration) -> impl Future<Output = ()> + Send;
}

/// Randomness for spreading retries: contenders that wait the same time
/// collide again.
pub trait Jitter {
    /// A duration drawn uniformly from zero to `ceiling`, inclusive.
    fn up_to(&mut self, ceiling: Duration) -> Duration;
}

/// Taskloom's configuration, as far as the use cases read it: the tag
/// schema's declarations.
///
/// Synchronous, and it blocks on the filesystem, as [`ProjectIdentity`]
/// does.
pub trait Configuration {
    /// The tag schema's declarations as configured (the default schema's
    /// are not among them): every layer read, a higher one's list
    /// replacing a lower one's, and `overrides` (`<path>=<value>`, as
    /// `--config-set` gives them) applied last. Empty when no layer sets
    /// one.
    fn tag_schema(&mut self, overrides: &[String]) -> Result<Vec<String>, Report>;
}

/// One record of Go taskloom's task log, `~/.ctxloom/tasks/<project-id>.jsonl`:
/// one JSON line, decoded as Go decodes it, with absent fields empty.
#[derive(Clone, PartialEq, Debug)]
pub struct GoRecord {
    /// Its line in the log, from 1.
    pub line: u32,
    /// The line, trimmed: Go's fold skips a line repeating an earlier one
    /// byte for byte, and the importer derives the record's command id
    /// from it.
    pub raw: String,
    pub op: GoOp,
    /// The task's harp id.
    pub task: String,
    pub text: String,
    pub status: String,
    pub trigger: String,
    /// The session that added the task (an add), or acted on it.
    pub session: String,
    /// On an add written by Go's `repair`: the key of the displaced add it
    /// re-adds.
    pub repair_of: String,
    pub tags: Vec<String>,
    /// When it was recorded; Go's zero time (0001-01-01T00:00:00Z) when the
    /// line has none.
    pub ts: Timestamp,
}

/// What a Go record does.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GoOp {
    Add,
    Status,
    Text,
    Remove,
    Tag,
    Untag,
}

impl GoOp {
    /// The op as the log spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            GoOp::Add => "add",
            GoOp::Status => "status",
            GoOp::Text => "text",
            GoOp::Remove => "remove",
            GoOp::Tag => "tag",
            GoOp::Untag => "untag",
        }
    }
}

/// Go taskloom's per-project task logs, read and never written.
///
/// Synchronous, and it blocks on the filesystem, as [`ProjectIdentity`]
/// does.
pub trait GoTaskLogs {
    /// The ids of the projects that have a log, sorted.
    fn projects(&mut self) -> Result<Vec<String>, Report>;

    /// The project's records in the log's order, blank lines left out;
    /// `None` when it has no log. A line Go's fold would refuse (malformed
    /// JSON, no op or one Go does not know) refuses the whole log, as Go's
    /// fold does.
    fn records(&mut self, project_id: &str) -> Result<Option<Vec<GoRecord>>, Report>;
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
