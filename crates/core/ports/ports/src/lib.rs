//! The ports the tasks application needs from outside, beyond annals' own
//! store port: which project a call is about, where its store is, how ids
//! are minted, what time it is, how to wait, what is configured, Go
//! taskloom's logs to import, and what a directory's identity rests on, to
//! show and repair it. Each is a trait a driven adapter (or the composition
//! root) implements; nothing here does I/O.
//!
//! Every future a port returns is `Send`, as annals requires of its async
//! store port, so a server on a multi-threaded runtime can spawn a use case
//! generic over them (`tokio::spawn`, an MCP server's handlers).

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use annals::next::v1::Uuid;
use annals::next::{AsyncStore, StoreError, Timestamp};
use annals_diagnostic::Report;

mod project;

pub use project::{Change, IdSource, ProjectAdmin, Registered, Survey};

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
}

/// Where a project's tasks are kept.
pub trait TaskStores {
    type Store: AsyncStore + Send;

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
