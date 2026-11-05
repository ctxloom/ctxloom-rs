//! The isolation decision table, as a pure read model (plan 5.5, 5.6).
//!
//! Two axes: the workspace (`none | worktree`, from orchestration) and the
//! runtime (`host | container-rootless | container-rootful`, from the agent
//! binding): six combinations. [`decide`] turns what is demanded and what
//! this machine offers into a [`Plan`], or a fatal [`Finding`].
//!
//! The rules, kept exactly:
//!
//! - each step of the degrade chain drops ONE axis, and only the workspace
//!   axis degrades: a worktree that cannot be had falls to the live directory
//!   with a warning;
//! - a container that cannot be served with the DEMANDED ownership is a
//!   fatal isolation finding (exit 3) in BOTH modes: `--degraded` does not
//!   rescue it, and an ownership mismatch is never a substitution of the
//!   other mode;
//! - a container is never dropped to the host (Go 0.7: `--degraded` falls
//!   back to the host for nothing).

mod finding;
mod table;

#[cfg(any(test, feature = "test-support"))]
pub use finding::diagnostic_samples;
pub use finding::{Finding, EN_US};
pub use table::{decide, Demand, Facts, Ownership, Plan, Runtime, Warning, Workspace};

#[cfg(test)]
#[path = "finding.test.rs"]
mod finding_tests;

#[cfg(test)]
#[path = "table.test.rs"]
mod table_tests;
