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

use std::collections::BTreeMap;
use std::path::Path;

use isolation_ports::{Removal, Safety, Scratch, Standing, Workspaces};
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

impl Owner {
    fn name(self) -> &'static str {
        match self {
            Owner::Alive => "alive",
            Owner::Dead => "dead",
            Owner::Indeterminate => "indeterminate",
        }
    }

    /// Why a lock that does not prove the owner dead refuses.
    fn refusal(self) -> &'static str {
        match self {
            Owner::Alive => "its session is running: its lock is held",
            _ => "its session's liveness cannot be proven: no readable lock, and a missing lock is never a free one",
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
    let skipped = |reason: String| Decision {
        verdict: Verdict::Skipped,
        reason,
    };
    // The lock only ever refuses: only a dead owner permits anything, so an
    // alive or unprovable one falls out here rather than toward a removal by
    // omission.
    if owner != Owner::Dead {
        return skipped(owner.refusal().to_string());
    }
    match standing {
        Standing::Unresolved(why) => skipped(format!(
            "its owning repository could not be resolved: {why}"
        )),
        Standing::Resolved {
            safety: Safety::Unsafe(why),
            ..
        } => Decision {
            verdict: Verdict::Spared,
            reason: format!("session {harp} has ended, but the worktree {why}"),
        },
        Standing::Resolved {
            safety: Safety::Clean,
            ..
        } => Decision {
            verdict: Verdict::Reapable,
            reason: String::new(),
        },
    }
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

/// What Go's reaper says when a checkout will not go after being classified
/// reapable: it changed in between, and teardown re-checks.
const LEFT_AFTER_LISTING: &str =
    "went dirty, or otherwise became unsafe, between listing and removal; teardown left it in place";

impl<W: Workspaces, L: Liveness> Worktrees<W, L> {
    /// One checkout, classified: its row and, when reapable, where its
    /// repository is. One probe per session, however many it left: liveness
    /// is the session's.
    fn classified(
        &mut self,
        scratch: &Scratch,
        owners: &mut BTreeMap<String, Owner>,
    ) -> (Row, Option<String>) {
        let owner = *owners
            .entry(scratch.harp.clone())
            .or_insert_with(|| Owner::of(self.liveness.lock_state(&scratch.harp)));
        // A checkout whose owner is not provably dead is never looked at.
        let standing = if owner == Owner::Dead {
            self.workspaces.inspect(&scratch.path)
        } else {
            Standing::Unresolved(String::new())
        };
        let decision = classify(&scratch.harp, owner, &standing);
        let repo = match (&decision.verdict, standing) {
            (Verdict::Reapable, Standing::Resolved { repo_dir, .. }) => Some(repo_dir),
            _ => None,
        };
        let name = Path::new(&scratch.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let row = Row {
            harp: scratch.harp.clone(),
            worktree: name,
            path: scratch.path.clone(),
            owner_state: owner.name(),
            verdict: decision.verdict,
            reason: decision.reason,
        };
        (row, repo)
    }
}

impl<W: Workspaces, L: Liveness> WorktreeUse for Worktrees<W, L> {
    fn worktrees(&mut self, harp: Option<&str>, apply: bool) -> Result<Report, IsolationError> {
        let adapter = IsolationError::Adapter;
        if let Some(harp) = harp {
            if !self.workspaces.session_known(harp).map_err(adapter)? {
                return Err(IsolationError::NoSession {
                    harp: harp.to_string(),
                });
            }
        }
        let found = self
            .workspaces
            .scratch(harp)
            .map_err(IsolationError::Adapter)?;
        let mut owners = BTreeMap::new();
        let mut classified: Vec<(Row, Option<String>)> = found
            .iter()
            .map(|scratch| self.classified(scratch, &mut owners))
            .collect();
        if apply {
            for (row, repo) in &mut classified {
                let Some(repo) = repo else { continue };
                match self.workspaces.remove(repo, &row.path) {
                    Removal::Removed => {
                        row.verdict = Verdict::Reaped;
                        row.reason = String::new();
                    }
                    Removal::Left(_) => {
                        row.verdict = Verdict::Spared;
                        row.reason = LEFT_AFTER_LISTING.to_string();
                    }
                }
            }
        }
        let rows: Vec<Row> = classified.into_iter().map(|(row, _)| row).collect();
        let count = |v: Verdict| rows.iter().filter(|r| r.verdict == v).count() as u64;
        Ok(Report {
            reaped: count(Verdict::Reaped),
            spared: count(Verdict::Spared),
            skipped: count(Verdict::Skipped),
            worktrees: rows,
            applied: apply,
        })
    }
}
