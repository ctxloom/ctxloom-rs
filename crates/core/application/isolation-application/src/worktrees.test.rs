//! The worktree use cases. Ported from Go's `worktree_reap_test.go`
//! (`ClassifyOrphanedWorktrees`, `ReapWorktrees`) and the scenarios of
//! cli/session.feature and journeys/j001300_closeout.feature (release/0.7,
//! f36e8dfb21); the plan's override is named where one applies.

use std::cell::RefCell;
use std::collections::BTreeMap;

use isolation_ports::{Failure, Prepared, Removal, Request, Safety, Scratch, Standing, Workspaces};
use session_ports::{Liveness, LockState};

use super::{classify, Owner, Verdict, WorktreeUse, Worktrees};
use crate::IsolationError;

fn clean() -> Standing {
    Standing::Resolved {
        repo_dir: "/repo".to_string(),
        safety: Safety::Clean,
    }
}

fn dirty() -> Standing {
    Standing::Resolved {
        repo_dir: "/repo".to_string(),
        safety: Safety::Unsafe("has uncommitted changes (or unknown state)".to_string()),
    }
}

// Go: TestClassifyOrphanedWorktrees_*: the lock only ever refuses.
#[test]
fn a_live_owner_is_skipped_without_the_checkout_being_looked_at() {
    let decision = classify("teal-running-wren", Owner::Alive, &clean());
    assert_eq!(decision.verdict, Verdict::Skipped);
    assert!(decision.reason.contains("running"), "{}", decision.reason);
}

// j001300 "Purging a session nothing can prove the liveness of removes
// nothing": a MISSING lock is never a free one.
#[test]
fn an_owner_nothing_can_prove_dead_is_skipped_like_a_live_one() {
    let decision = classify("slate-unproven-vole", Owner::Indeterminate, &clean());
    assert_eq!(decision.verdict, Verdict::Skipped);
    assert!(
        decision.reason.contains("cannot be proven"),
        "{}",
        decision.reason
    );
}

#[test]
fn a_dead_owner_whose_repository_cannot_be_resolved_is_skipped_saying_why() {
    let standing = Standing::Unresolved("not a git repository".to_string());
    let decision = classify("amber-quiet-heron", Owner::Dead, &standing);
    assert_eq!(decision.verdict, Verdict::Skipped);
    assert!(
        decision.reason.contains("could not be resolved"),
        "{}",
        decision.reason
    );
    assert!(decision.reason.contains("not a git repository"));
}

// j001300 "THE SAFETY SCENARIO": uncommitted work is spared in place, and the
// reason says the session ended but the tree holds work.
#[test]
fn a_dead_owner_with_uncommitted_work_is_spared_with_the_reason() {
    let decision = classify("amber-quiet-heron", Owner::Dead, &dirty());
    assert_eq!(decision.verdict, Verdict::Spared);
    assert_eq!(
        decision.reason,
        "session amber-quiet-heron has ended, but the worktree has uncommitted changes (or unknown state)"
    );
}

#[test]
fn a_dead_owner_with_a_clean_checkout_is_reapable() {
    let decision = classify("amber-quiet-heron", Owner::Dead, &clean());
    assert_eq!(decision.verdict, Verdict::Reapable);
    assert!(decision.reason.is_empty());
}

#[test]
fn a_lock_state_is_an_owner() {
    assert_eq!(Owner::of(LockState::Held), Owner::Alive);
    assert_eq!(Owner::of(LockState::Free), Owner::Dead);
    assert_eq!(Owner::of(LockState::Unproven), Owner::Indeterminate);
}

/// A workspaces adapter that knows these checkouts and records its removals.
struct Fake {
    scratch: Vec<(String, String)>,
    standing: BTreeMap<String, Standing>,
    /// Paths whose removal is refused ("went dirty between listing and removal").
    stubborn: Vec<String>,
    known: Vec<String>,
    removed: RefCell<Vec<String>>,
    inspected: RefCell<Vec<String>>,
}

impl Fake {
    fn new() -> Fake {
        Fake {
            scratch: Vec::new(),
            standing: BTreeMap::new(),
            stubborn: Vec::new(),
            known: Vec::new(),
            removed: RefCell::new(Vec::new()),
            inspected: RefCell::new(Vec::new()),
        }
    }

    fn checkout(mut self, harp: &str, name: &str, standing: Standing) -> Fake {
        let path = format!("/home/.ctxloom/sessions/{harp}/work/ctxloom-wt-{name}");
        self.scratch.push((harp.to_string(), path.clone()));
        self.standing.insert(path, standing);
        if !self.known.iter().any(|k| k == harp) {
            self.known.push(harp.to_string());
        }
        self
    }
}

impl Workspaces for &Fake {
    fn prepare(&self, _: &Request) -> Result<Prepared, Failure> {
        unreachable!("the worktree commands prepare nothing")
    }

    fn session_known(&self, harp: &str) -> Result<bool, Failure> {
        Ok(self.known.iter().any(|k| k == harp))
    }

    fn scratch(&self, harp: Option<&str>) -> Result<Vec<Scratch>, Failure> {
        Ok(self
            .scratch
            .iter()
            .filter(|(h, _)| harp.is_none_or(|only| only == h))
            .map(|(h, p)| Scratch {
                harp: h.clone(),
                path: p.clone(),
            })
            .collect())
    }

    fn inspect(&self, path: &str) -> Standing {
        self.inspected.borrow_mut().push(path.to_string());
        self.standing[path].clone()
    }

    fn remove(&self, _repo_dir: &str, path: &str) -> Removal {
        if self.stubborn.iter().any(|p| p == path) {
            return Removal::Left("teardown left it".to_string());
        }
        self.removed.borrow_mut().push(path.to_string());
        Removal::Removed
    }
}

/// Liveness from a table; counts its probes.
struct Locks {
    states: BTreeMap<String, LockState>,
    probes: RefCell<Vec<String>>,
}

impl Liveness for &Locks {
    fn lock_state(&mut self, harp: &str) -> LockState {
        self.probes.borrow_mut().push(harp.to_string());
        self.states
            .get(harp)
            .copied()
            .unwrap_or(LockState::Unproven)
    }
}

fn locks(states: &[(&str, LockState)]) -> Locks {
    Locks {
        states: states.iter().map(|(h, s)| (h.to_string(), *s)).collect(),
        probes: RefCell::new(Vec::new()),
    }
}

// j001300 "The scratch worktrees are listed before anything is removed":
// each checkout with its harp, its owner and its verdict; nothing removed.
#[test]
fn listing_names_each_checkout_with_its_harp_its_owner_and_its_verdict() {
    let fake = Fake::new()
        .checkout("amber-quiet-heron", "clean", clean())
        .checkout("amber-quiet-heron", "wip", dirty());
    let held = locks(&[("amber-quiet-heron", LockState::Free)]);
    let mut worktrees = Worktrees::new(&fake, &held);
    let report = worktrees.worktrees(None, false).unwrap();
    assert!(!report.applied);
    assert_eq!(report.worktrees.len(), 2);
    let clean_row = report
        .worktrees
        .iter()
        .find(|r| r.worktree == "ctxloom-wt-clean")
        .unwrap();
    assert_eq!(clean_row.harp, "amber-quiet-heron");
    assert_eq!(clean_row.owner_state, "dead");
    assert_eq!(clean_row.verdict, Verdict::Reapable);
    let wip_row = report
        .worktrees
        .iter()
        .find(|r| r.worktree == "ctxloom-wt-wip")
        .unwrap();
    assert_eq!(wip_row.verdict, Verdict::Spared);
    assert_eq!(
        (report.reaped, report.spared, report.skipped),
        (0, 1, 0),
        "a plan reaps nothing"
    );
    assert!(fake.removed.borrow().is_empty(), "listing removes nothing");
}

// Go: one probe per HARP, not per checkout.
#[test]
fn a_session_is_probed_once_however_many_checkouts_it_left() {
    let fake = Fake::new()
        .checkout("amber-quiet-heron", "a", clean())
        .checkout("amber-quiet-heron", "b", clean());
    let held = locks(&[("amber-quiet-heron", LockState::Free)]);
    Worktrees::new(&fake, &held).worktrees(None, false).unwrap();
    assert_eq!(held.probes.borrow().len(), 1);
}

// Go: a live or unprovable owner's checkout is never inspected (git is not
// asked about it).
#[test]
fn a_skipped_checkout_is_never_inspected() {
    let fake = Fake::new().checkout("teal-running-wren", "own", clean());
    let held = locks(&[("teal-running-wren", LockState::Held)]);
    Worktrees::new(&fake, &held).worktrees(None, false).unwrap();
    assert!(fake.inspected.borrow().is_empty());
}

// j001300 "Purging removes only what it can prove is safe, and says why it
// left the rest".
#[test]
fn purging_removes_only_the_clean_orphan_and_says_why_it_spared_the_rest() {
    let fake = Fake::new()
        .checkout("amber-quiet-heron", "clean", clean())
        .checkout("amber-quiet-heron", "wip", dirty())
        .checkout("teal-running-wren", "own", clean())
        .checkout("slate-unproven-vole", "own", clean());
    let held = locks(&[
        ("amber-quiet-heron", LockState::Free),
        ("teal-running-wren", LockState::Held),
    ]);
    let report = Worktrees::new(&fake, &held)
        .worktrees(Some("amber-quiet-heron"), true)
        .unwrap();
    assert!(report.applied);
    assert_eq!((report.reaped, report.spared, report.skipped), (1, 1, 0));
    assert_eq!(
        *fake.removed.borrow(),
        vec!["/home/.ctxloom/sessions/amber-quiet-heron/work/ctxloom-wt-clean".to_string()]
    );
    let spared = report
        .worktrees
        .iter()
        .find(|r| r.verdict == Verdict::Spared)
        .unwrap();
    assert!(spared.reason.contains("uncommitted changes"));
}

// Go: ReapWorktrees re-checks at removal; a tree that went dirty between
// listing and removal is spared, not removed.
#[test]
fn a_checkout_that_teardown_leaves_is_spared_with_the_toctou_reason() {
    let mut fake = Fake::new().checkout("amber-quiet-heron", "clean", clean());
    fake.stubborn
        .push("/home/.ctxloom/sessions/amber-quiet-heron/work/ctxloom-wt-clean".to_string());
    let held = locks(&[("amber-quiet-heron", LockState::Free)]);
    let report = Worktrees::new(&fake, &held)
        .worktrees(Some("amber-quiet-heron"), true)
        .unwrap();
    assert_eq!((report.reaped, report.spared), (0, 1));
    assert_eq!(
        report.worktrees[0].reason,
        "went dirty, or otherwise became unsafe, between listing and removal; teardown left it in place"
    );
}

// j001300 "Purging a session that is still running removes nothing".
#[test]
fn purging_a_running_session_removes_nothing_and_skips_everything() {
    let fake = Fake::new().checkout("teal-running-wren", "clean", clean());
    let held = locks(&[("teal-running-wren", LockState::Held)]);
    let report = Worktrees::new(&fake, &held)
        .worktrees(Some("teal-running-wren"), true)
        .unwrap();
    assert_eq!((report.reaped, report.spared, report.skipped), (0, 0, 1));
    assert!(fake.removed.borrow().is_empty());
}

// session.feature: a session with no directory is an ordinary error; one
// with no checkouts is an empty, successful listing.
#[test]
fn naming_a_session_with_no_directory_is_an_error_and_one_with_no_checkouts_is_empty() {
    let fake = Fake::new();
    let held = locks(&[]);
    let mut worktrees = Worktrees::new(&fake, &held);
    let error = worktrees.worktrees(Some("nobody"), false).unwrap_err();
    assert!(matches!(error, IsolationError::NoSession { harp } if harp == "nobody"));

    let fake = Fake {
        known: vec!["amber-swift-owl".to_string()],
        ..Fake::new()
    };
    let report = Worktrees::new(&fake, &held)
        .worktrees(Some("amber-swift-owl"), false)
        .unwrap();
    assert!(report.worktrees.is_empty());
}

// The json document: Go's `sessionWorktreeReport` keys.
#[test]
fn the_report_serializes_with_go_s_keys() {
    let fake = Fake::new().checkout("amber-quiet-heron", "clean", clean());
    let held = locks(&[("amber-quiet-heron", LockState::Free)]);
    let report = Worktrees::new(&fake, &held).worktrees(None, false).unwrap();
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"worktrees\""), "{json}");
    assert!(json.contains("\"owner_state\":\"dead\""), "{json}");
    assert!(json.contains("\"verdict\":\"reapable\""), "{json}");
    assert!(json.contains("\"applied\":false"), "{json}");
}
