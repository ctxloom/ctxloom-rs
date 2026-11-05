//! The `Tasks` port: what ctxloom needs of tasks. ctxloom keeps no task or
//! plan functionality of its own (owner ruling, 2026-11-04); taskloom owns
//! them, and a driven adapter (`taskloom-cli-client`) asks it. The types are
//! plain: nothing here knows how taskloom was asked.

use std::time::Duration;

mod error;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{TasksError, EN_US};

/// A task taskloom holds Deferred: parked until its revive trigger fires.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeferredTask {
    /// The task's harp id (`unlearned-irregular`).
    pub harp_id: String,
    pub text: String,
    /// The revive condition, in the words it was written in.
    pub trigger: String,
    /// The task's tags, each as taskloom spells it (`queue:20261104`).
    pub tags: Vec<String>,
    /// When the task became Deferred, as the time since the Unix epoch (the
    /// core reads no clock, so it holds no clock type); none where taskloom
    /// does not say (an older taskloom does not emit it).
    pub deferred_since: Option<Duration>,
}

/// The tasks taskloom holds for the project.
pub trait Tasks {
    /// Every task now Deferred.
    fn deferred(&self) -> Result<Vec<DeferredTask>, TasksError>;
}

#[cfg(test)]
#[path = "error.test.rs"]
mod error_tests;
