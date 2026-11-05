//! `deps pull`: make the installation match upstream, and never move a pin.
//!
//! Installed remote content is a projection of remote state: every byte of it
//! can be fetched again from the address it came from, none of it is authored
//! here. So dropping a bundle a remote stopped publishing is synchronization,
//! as installing one it started publishing is, and asks for no `--yes`. What
//! makes it safe is THE EVIDENCE RULE (see `deps::plan`).

use std::collections::{BTreeMap, BTreeSet};

use deps::lock::Lock;
use deps::plan::{self, Pin, Probe};
use deps::reference::RemoteRef;
use deps_ports::{RepoTree, Why};
use pin_changes::{diff_pin, PinChange, Side, Tree, TreeFile};

use crate::cache::{self, ByAddress};
use crate::closure::{Mode, Walk};
use crate::views::{
    ConstraintChangeView, FailedItem, PullReport, ReconcileView, SyncItem, UncheckedRemote,
};
use crate::{AppError, Deps, Notice, Outcome};

/// The fix a failed install names.
const INSTALL_FIX: &str =
    "check network/auth and retry (ctxloom deps pull), or drop the reference from its profile";

impl PullReport {
    /// The error the command exits on: dependencies that could not be
    /// installed (the payload is printed first).
    pub fn failure(&self) -> Option<AppError> {
        (self.errors > 0).then(|| AppError::PullFailed {
            count: self.errors,
            refs: self
                .failed
                .iter()
                .map(|f| f.reference.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        })
    }
}

/// What installing the closure's pins did.
#[derive(Default)]
struct Installs {
    synced: Vec<SyncItem>,
    skipped: Vec<SyncItem>,
    failed: Vec<FailedItem>,
    installed: u64,
    reinstalled: u64,
    /// The trees read, by identity: what a first pin is disclosed from.
    trees: BTreeMap<String, RepoTree>,
    /// The identities made a pin for the first time.
    first: Vec<String>,
}

impl Deps {
    /// `deps pull`. `force` reinstalls each reference at its pin instead of
    /// skipping what is installed; without `lock` the lockfile is left alone.
    pub fn pull(&self, force: bool, lock: bool) -> Outcome<PullReport> {
        let mut notices = Vec::new();
        let result = self.pulling(force, lock, &mut notices);
        Outcome { result, notices }
    }

    fn pulling(
        &self,
        force: bool,
        write_lock: bool,
        notices: &mut Vec<Notice>,
    ) -> Result<PullReport, AppError> {
        let registry = self.registry()?;
        let (active, text) = self.lock()?;
        let locals = self.local_profiles()?;
        let mut walk = Walk::new(self, &registry, &active, &locals, Mode::Lock);
        walk.walk_all();
        let refusal = walk.refusal();
        notices.append(&mut walk.notices);
        if let Some(refusal) = refusal {
            return Err(refusal);
        }
        let conflicts = walk.closure.conflicts();
        if !conflicts.is_empty() {
            return Err(AppError::Conflict {
                text: plan::conflict_text(&conflicts),
            });
        }
        let pins = walk.closure.pins();
        let unexpanded = walk.closure.unexpanded();
        if pins.is_empty() && unexpanded.is_empty() {
            return Ok(empty_report());
        }
        let mut installs = self.install_pins(&pins, &active, force);
        let mut next = plan::relock(&active, &installable(&pins, &installs, &active));
        if !unexpanded.is_empty() {
            let kept = plan::preserve_unreached(&active, &mut next);
            if kept > 0 {
                notices.push(Notice::text(format!(
                    "dependency closure is incomplete ({} parent profile(s) unreachable); preserving {kept} existing lockfile entry(ies)",
                    unexpanded.len()
                )));
            }
        }
        let removed = plan::dropped(&active, &next);
        let mut reconcile = None;
        let mut pruned = Vec::new();
        if write_lock {
            let mut fetched = std::mem::take(&mut walk.fetched);
            let decided = self.reconcile(&next, &mut fetched);
            for gone in &decided.gone {
                next.entries.remove(gone);
            }
            if !(next.is_empty() && active.is_empty()) {
                self.save_lock(text.as_deref(), &next, !decided.gone.is_empty())?;
            }
            pruned = self.prune(&next, installs.failed.is_empty() && unexpanded.is_empty());
            reconcile = Some(decided);
        }
        let changes = self.first_pins(&mut installs, &next, notices);
        let constraint_changes = plan::constraint_changes(&next, &walk.declared)
            .into_iter()
            .map(|c| ConstraintChangeView {
                identity: c.identity,
                pinned: c.pinned,
                declared: c.declared,
                sha: c.sha,
            })
            .collect();
        Ok(report(
            installs,
            pins.len() as u64,
            removed,
            pruned,
            unexpanded,
            constraint_changes,
            changes,
            reconcile,
        ))
    }

    /// Install each pin whose tree is not installed at its pin (or each, when
    /// forced). An existing pin is never moved: a reference whose lock entry
    /// is at the pin is skipped, kept at its locked commit.
    fn install_pins(&self, pins: &[Pin], active: &Lock, force: bool) -> Installs {
        let mut out = Installs::default();
        for pin in pins {
            let pinned = active
                .entries
                .get(&pin.identity)
                .is_some_and(|entry| entry.sha == pin.hash);
            let present = cache::slot_of(&pin.identity)
                .is_ok_and(|slot| self.files.is_installed(&slot, &pin.hash));
            if pinned && present && !force {
                out.skipped.push(item(pin, "skipped", ""));
                continue;
            }
            match self.install(pin) {
                Ok(tree) => {
                    out.trees.insert(pin.identity.clone(), tree);
                    if pinned {
                        out.reinstalled += 1;
                        out.synced.push(item(pin, "reinstalled", ""));
                    } else {
                        out.installed += 1;
                        out.first.push(pin.identity.clone());
                        out.synced.push(item(pin, "installed", ""));
                    }
                }
                Err(detail) => out.failed.push(FailedItem {
                    reference: pin.identity.clone(),
                    kind: "bundle",
                    status: "failed",
                    error: detail,
                    fix: INSTALL_FIX.to_string(),
                }),
            }
        }
        out
    }

    /// Read the pinned bundle out of its clone and install it.
    fn install(&self, pin: &Pin) -> Result<RepoTree, Why> {
        let remote = RemoteRef::parse(&pin.identity)
            .ok_or_else(|| format!("{} is not a remote bundle reference", pin.identity))?;
        let tree = ByAddress(self.remotes.as_ref())
            .read_bundle(&pin.url, &pin.hash, &remote.bundle)?
            .ok_or_else(|| {
                format!(
                    "bundle {} not found in {} at {}",
                    remote.bundle,
                    pin.url,
                    pin.hash.get(..7).unwrap_or(&pin.hash)
                )
            })?;
        if !tree.contains_key("bundle.yaml") {
            return Err(format!(
                "{} has no bundle.yaml at {}: it is not a bundle",
                pin.identity,
                pin.hash.get(..7).unwrap_or(&pin.hash)
            ));
        }
        self.files
            .install(&cache::slot(&remote)?, &pin.hash, &tree)?;
        Ok(tree)
    }

    /// Decide what upstream no longer serves, under the evidence rule. The
    /// repositories already fetched this run are not fetched again.
    fn reconcile(
        &self,
        lock: &Lock,
        fetched: &mut BTreeMap<String, Result<(), Why>>,
    ) -> plan::Reconcile {
        let installed: Vec<String> = lock.entries.keys().cloned().collect();
        let mut reach = |url: &str| {
            fetched
                .entry(url.to_string())
                .or_insert_with(|| ByAddress(self.remotes.as_ref()).refresh(url))
                .clone()
        };
        let mut content = |reference: &str| {
            let Some(remote) = RemoteRef::parse(reference) else {
                return Probe::Failed(format!("{reference} does not parse"));
            };
            match ByAddress(self.remotes.as_ref()).has_bundle(&remote.url, &remote.bundle) {
                Ok(true) => Probe::Present,
                Ok(false) => Probe::Gone,
                Err(detail) => Probe::Failed(detail),
            }
        };
        plan::reconcile(&installed, &mut reach, &mut content)
    }

    /// Remove the installed trees the lock no longer names, when the pull
    /// installed everything it was asked to; the identities whose trees went.
    fn prune(&self, lock: &Lock, whole: bool) -> Vec<String> {
        if !whole {
            return Vec::new();
        }
        let keep: Vec<(String, String)> = lock
            .entries
            .iter()
            .map(|(identity, entry)| (identity.clone(), entry.sha.clone()))
            .collect();
        self.files.prune(&keep).unwrap_or_default()
    }

    /// What each first pin brings in, disclosed from the tree installed.
    fn first_pins(
        &self,
        installs: &mut Installs,
        lock: &Lock,
        notices: &mut Vec<Notice>,
    ) -> Vec<PinChange> {
        let mut changes = Vec::new();
        for identity in &installs.first {
            let (Some(entry), Some(tree)) =
                (lock.entries.get(identity), installs.trees.get(identity))
            else {
                continue;
            };
            let tree = disclosed(tree);
            let (change, unreadable) = diff_pin(
                identity,
                &entry.url,
                None,
                Side {
                    sha: &entry.sha,
                    version: &entry.version,
                    tree: &tree,
                },
            );
            notices.extend(
                unreadable
                    .iter()
                    .map(|u| Notice::unlisted(&u.identity, &u.sha, &u.cause)),
            );
            changes.push(change);
        }
        changes
    }
}

/// A tree as the disclosure reads it: committed executability only.
pub(crate) fn disclosed(tree: &RepoTree) -> Tree {
    tree.iter()
        .map(|(path, file)| {
            (
                path.clone(),
                TreeFile {
                    data: file.data.clone(),
                    executable: file.executable,
                },
            )
        })
        .collect()
}

/// The pins the new lock is built from: those that installed, and those that
/// were pinned before (a failed install of a new pin writes no entry).
fn installable(pins: &[Pin], installs: &Installs, active: &Lock) -> Vec<Pin> {
    let failed: BTreeSet<&str> = installs
        .failed
        .iter()
        .map(|f| f.reference.as_str())
        .collect();
    pins.iter()
        .filter(|pin| {
            !failed.contains(pin.identity.as_str()) || active.entries.contains_key(&pin.identity)
        })
        .cloned()
        .collect()
}

fn item(pin: &Pin, status: &'static str, error: &str) -> SyncItem {
    SyncItem {
        reference: pin.identity.clone(),
        kind: "bundle",
        status,
        error: error.to_string(),
    }
}

fn empty_report() -> PullReport {
    PullReport {
        status: "empty",
        total: 0,
        installed: 0,
        reinstalled: 0,
        errors: 0,
        synced: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
        removed: Vec::new(),
        pruned_checkouts: Vec::new(),
        incomplete: false,
        unreachable: Vec::new(),
        constraint_changes: Vec::new(),
        changes: Vec::new(),
        message: "No remote references found in profiles".to_string(),
        reconcile: None,
    }
}

#[allow(clippy::too_many_arguments, reason = "the report's own parts")]
fn report(
    installs: Installs,
    total: u64,
    removed: Vec<String>,
    pruned: Vec<String>,
    unexpanded: Vec<String>,
    constraint_changes: Vec<ConstraintChangeView>,
    changes: Vec<PinChange>,
    reconcile: Option<plan::Reconcile>,
) -> PullReport {
    let errors = installs.failed.len() as u64;
    PullReport {
        status: if errors > 0 {
            "completed_with_errors"
        } else {
            "completed"
        },
        total,
        installed: installs.installed,
        reinstalled: installs.reinstalled,
        errors,
        message: format!(
            "Synced {total} items: {} installed, {} reinstalled, {} skipped, {errors} failed",
            installs.installed,
            installs.reinstalled,
            installs.skipped.len()
        ),
        synced: installs.synced,
        skipped: installs.skipped,
        failed: installs.failed,
        removed,
        pruned_checkouts: pruned,
        incomplete: !unexpanded.is_empty(),
        unreachable: unexpanded,
        constraint_changes,
        changes,
        reconcile: reconcile.map(|r| ReconcileView {
            gone: r.gone,
            unreachable: r
                .unreachable
                .into_iter()
                .map(|u| UncheckedRemote {
                    url: u.url,
                    reason: u.reason,
                    refs: u.refs,
                })
                .collect(),
        }),
    }
}

#[cfg(test)]
#[path = "pull.test.rs"]
mod tests;
