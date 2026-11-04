//! `session worktrees list|purge`: the scratch checkouts a session left
//! behind, each classified against its owning SESSION's liveness lock.
//!
//! Go: `isolation.ClassifyOrphanedWorktrees` and `ReapWorktrees` (release/0.7).
//! Liveness is the session's, never the worktree's, so every checkout under
//! one session shares one verdict on its owner. The lock only ever refuses:
//!
//! | the owner | the checkout | the verdict |
//! |---|---|---|
//! | alive or unprovable | (never inspected) | skipped |
//! | dead | its repository cannot be resolved | skipped |
//! | dead | uncommitted work, ignored files, or an unknowable state | spared, in place |
//! | dead | clean | reapable; reaped on `yes`, re-checked at removal |
//!
//! A long-lived worktree ctxloom did not create lives outside the sessions'
//! work directories and is never listed here.

use isolation_ports::{Removal, Safety, Standing, Workspaces};
use serde::Serialize;
use session_ports::{Liveness, LockState};

use crate::IsolationError;

/// What the owning session's lock said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Alive,
    Dead,
    Indeterminate,
}

impl Owner {
    pub fn of(state: LockState) -> Owner {
        match state {
            LockState::Held => Owner::Alive,
            LockState::Free => Owner::Dead,
            LockState::Unproven => Owner::Indeterminate,
        }
    }
}

/// A checkout's fate.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Orphaned and clean: removable; not yet acted on.
    Reapable,
    /// Removed.
    Reaped,
    /// Orphaned but carrying real (or unknowable) work: left in place.
    Spared,
    /// Its owner is alive or cannot be proven dead, or its repository cannot
    /// be resolved: never touched.
    Skipped,
}

/// A verdict and the reason behind it (never empty for spared or skipped).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Decision {
    pub verdict: Verdict,
    pub reason: String,
}

/// One checkout's classification, changing nothing.
pub fn classify(harp: &str, owner: Owner, standing: &Standing) -> Decision {
    let _ = (harp, owner, standing);
    todo!("S11 green")
}

/// One checkout, as the report shows it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Row {
    pub harp: String,
    /// The checkout's directory name.
    pub worktree: String,
    pub path: String,
    pub owner_state: &'static str,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

/// The `session worktrees` answer.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Report {
    pub worktrees: Vec<Row>,
    pub reaped: u64,
    pub spared: u64,
    pub skipped: u64,
    /// True only when a removal was asked for and run: every other report is
    /// a plan, and nothing on disk changed.
    pub applied: bool,
}

/// What the worktree commands call.
pub trait WorktreeUse {
    /// Classify `harp`'s scratch checkouts (every session's when none), and
    /// with `apply` remove exactly those classified reapable.
    ///
    /// The default answers an empty report: a wiring without workspaces has
    /// none to classify.
    fn worktrees(&mut self, harp: Option<&str>, apply: bool) -> Result<Report, IsolationError> {
        let _ = (harp, apply);
        Ok(Report {
            worktrees: Vec::new(),
            reaped: 0,
            spared: 0,
            skipped: 0,
            applied: apply,
        })
    }
}

/// The worktree use cases over a workspaces adapter and the sessions'
/// liveness locks.
pub struct Worktrees<W, L> {
    workspaces: W,
    liveness: L,
}

impl<W: Workspaces, L: Liveness> Worktrees<W, L> {
    pub fn new(workspaces: W, liveness: L) -> Worktrees<W, L> {
        Worktrees {
            workspaces,
            liveness,
        }
    }
}

impl<W: Workspaces, L: Liveness> WorktreeUse for Worktrees<W, L> {
    fn worktrees(&mut self, harp: Option<&str>, apply: bool) -> Result<Report, IsolationError> {
        let _ = (&mut self.workspaces, &mut self.liveness, harp, apply);
        let _ = (Removal::Removed, Safety::Clean);
        todo!("S11 green")
    }
}
