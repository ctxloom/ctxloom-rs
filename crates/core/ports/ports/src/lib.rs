//! The ports the tasks application needs from outside, beyond annals' own
//! store port: which project a call is about, where its store is, how ids
//! are minted, what time it is, and how to wait. Each is a trait a driven
//! adapter (or the composition root) implements; nothing here does I/O.

use std::future::Future;
use std::path::PathBuf;
use std::time::Duration;

use annals::next::v1::Uuid;
use annals::next::{AsyncStore, StoreError, Timestamp};
use annals_diagnostic::Report;

/// A resolved project: its id, the work root it was resolved from, and
/// where its store is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The project id ("jolly-mute-magma").
    pub id: String,
    /// The work root the id was resolved for, as the user would type it.
    pub root: String,
    /// The project's database.
    pub store: PathBuf,
}

/// Which project a call is about.
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
    type Store: AsyncStore;

    /// The project's store, opened (and created) on first use and held
    /// after.
    fn open(
        &mut self,
        project: &Project,
    ) -> impl Future<Output = Result<&mut Self::Store, StoreError>>;
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
    fn sleep(&mut self, duration: Duration) -> impl Future<Output = ()>;
}

/// Randomness for spreading retries: contenders that wait the same time
/// collide again.
pub trait Jitter {
    /// A duration drawn uniformly from zero to `ceiling`, inclusive.
    fn up_to(&mut self, ceiling: Duration) -> Duration;
}
