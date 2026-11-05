//! The isolation use cases (S11):
//!
//! - `session worktrees list|purge` ([`Worktrees`]): the scratch checkouts a
//!   session left behind, classified against their session's lock; a purge
//!   removes only what is provably safe, and reports first.
//! - `container check|build|prune|scaffold|tooling` ([`Containers`]).
//!
//! The isolation DECISION table is a pure read model (`isolation`); the live
//! container launch arrives with the launcher (S6).

mod container;
mod error;
mod worktrees;

pub use container::{
    CheckReport, ContainerUse, Containers, Declaration, ScaffoldReport, ToolingReport,
    TOOLING_INSTRUCTIONS,
};
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{IsolationError, CATALOGS, EN_US};
pub use worktrees::{classify, Decision, Owner, Report, Row, Verdict, WorktreeUse, Worktrees};

#[cfg(test)]
#[path = "worktrees.test.rs"]
mod worktrees_tests;

#[cfg(test)]
#[path = "container.test.rs"]
mod container_tests;

#[cfg(test)]
#[path = "error.test.rs"]
mod error_tests;
