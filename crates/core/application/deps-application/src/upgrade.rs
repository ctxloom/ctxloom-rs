//! `deps upgrade`: the one command that moves an existing pin. It shows every
//! pin that would move to the newest commit its constraint allows, and what
//! each move brings in; with `--yes` it writes the advance to the lock and
//! moves each moved bundle's installed tree with it.
//!
//! There is no review gate: the lock is pure dependency pinning, and the
//! preview is the review. A held entry stays frozen. Apply recomputes rather
//! than replaying a preview, so a remote that moved since the preview is what
//! lands, and what is shown.

use std::collections::BTreeSet;

use deps::lock::Lock;
use deps::plan::{self, Pin};
use deps::reference::RemoteRef;
use pin_changes::{diff_pin, PinChange, Side};

use crate::cache;
use crate::closure::{Mode, Walk};
use crate::pull::disclosed;
use crate::views::UpgradeReport;
use crate::{AppError, Deps, Notice, Outcome};

impl Deps {
    /// `deps upgrade`: a preview, or with `apply` the advance itself.
    pub fn upgrade(&self, apply: bool) -> Outcome<UpgradeReport> {
        let mut notices = Vec::new();
        let result = self.upgrading(apply, &mut notices);
        Outcome { result, notices }
    }

    fn upgrading(&self, apply: bool, notices: &mut Vec<Notice>) -> Result<UpgradeReport, AppError> {
        let registry = self.registry()?;
        let (active, text) = self.lock()?;
        let locals = self.local_profiles()?;
        let mut walk = Walk::new(self, &registry, &active, &locals, Mode::Upgrade);
        // Every repository the lock records is refreshed, those reached only
        // through transitive parents included; an unregistered one is not
        // fetched, and the walk that reaches it refuses it.
        for entry in active.entries.values() {
            if registry.lookup_url(&entry.url).is_some() {
                walk.freshen(&entry.url);
            }
        }
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
        let unexpanded = walk.closure.unexpanded().len();
        let fetch_failed = walk.fetched.values().any(Result::is_err);
        let decided = plan::upgrade(&active, &walk.closure.pins(), unexpanded, fetch_failed);
        if unexpanded > 0 {
            notices.push(Notice::text(format!(
                "dependency closure is incomplete ({unexpanded} parent profile(s) unreachable); preserving the existing lockfile entries it owns"
            )));
        }
        let changes = self.disclose(&active, &decided.moved, notices);
        let nothing = decided.next.is_empty() && active.is_empty();
        if !nothing {
            self.check_lock(text.as_deref(), &decided.next, false)?;
        }
        if apply && !nothing {
            self.save_lock(text.as_deref(), &decided.next, false)?;
            self.move_trees(&decided.moved, &decided.next, notices);
        }
        Ok(UpgradeReport {
            applied: apply,
            changes,
            incomplete: decided.incomplete,
            nothing_declared: decided.nothing_declared,
            removed: decided.removed,
        })
    }

    /// What each pin that moves (or is first made) brings in: the tree at
    /// the old commit against the tree at the new. A tree that cannot be read
    /// leaves the change listed by its header, the reason said.
    fn disclose(&self, active: &Lock, moved: &[Pin], notices: &mut Vec<Notice>) -> Vec<PinChange> {
        let mut changes = Vec::new();
        for pin in moved {
            let Some(remote) = RemoteRef::parse(&pin.identity) else {
                continue;
            };
            let to = self.tree_at(&remote, &pin.hash);
            let prior = active.entries.get(&pin.identity);
            let from = prior.map(|entry| (entry, self.tree_at(&remote, &entry.sha)));
            let read = |result: &Result<_, String>, sha: &str, notices: &mut Vec<Notice>| {
                if let Err(detail) = result {
                    notices.push(Notice::unlisted(
                        &pin.identity,
                        sha,
                        &AppError::Unreadable {
                            what: pin.identity.clone(),
                            detail: detail.clone(),
                        },
                    ));
                }
            };
            read(&to, &pin.hash, notices);
            if let Some((entry, tree)) = &from {
                read(tree, &entry.sha, notices);
            }
            let to_tree = to.unwrap_or_default();
            let from_tree = from
                .as_ref()
                .map(|(entry, tree)| (entry, tree.clone().unwrap_or_default()));
            let (change, unreadable) = diff_pin(
                &pin.identity,
                &pin.url,
                from_tree.as_ref().map(|(entry, tree)| Side {
                    sha: &entry.sha,
                    version: &entry.version,
                    tree,
                }),
                Side {
                    sha: &pin.hash,
                    version: &pin.version,
                    tree: &to_tree,
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

    /// The bundle's tree at a commit: the installed one where it is, else the
    /// clone's.
    fn tree_at(&self, remote: &RemoteRef, sha: &str) -> Result<pin_changes::Tree, String> {
        if let Ok(Some(tree)) =
            cache::slot(remote).and_then(|slot| self.files.read_installed(&slot, sha))
        {
            return Ok(disclosed(&tree));
        }
        match self.remotes.read_bundle(&remote.url, sha, &remote.bundle) {
            Ok(Some(tree)) => Ok(disclosed(&tree)),
            Ok(None) => Err(format!("no such bundle at {}", sha.get(..7).unwrap_or(sha))),
            Err(detail) => Err(detail),
        }
    }

    /// A moved pin moves its installed tree with it; the trees the lock no
    /// longer names go. Best effort: an upgrade whose lock is already
    /// correct is not aborted by the cache, and the next pull repairs it.
    fn move_trees(&self, moved: &[Pin], next: &Lock, notices: &mut Vec<Notice>) {
        for pin in moved {
            let Some(remote) = RemoteRef::parse(&pin.identity) else {
                continue;
            };
            let outcome = cache::slot(&remote).and_then(|slot| {
                self.remotes
                    .read_bundle(&pin.url, &pin.hash, &remote.bundle)
                    .and_then(|tree| tree.ok_or_else(|| "no such bundle".to_string()))
                    .and_then(|tree| self.files.install(&slot, &pin.hash, &tree))
            });
            if let Err(detail) = outcome {
                notices.push(Notice::with_fix(
                    format!(
                        "{} was upgraded to {} but its cached tree could not be moved to that commit ({detail})",
                        pin.identity, pin.hash
                    ),
                    "run `ctxloom deps pull` to re-install it",
                ));
            }
        }
        let keep: BTreeSet<(String, String)> = next
            .entries
            .iter()
            .map(|(identity, entry)| (identity.clone(), entry.sha.clone()))
            .collect();
        let keep: Vec<(String, String)> = keep.into_iter().collect();
        let _ = self.files.prune(&keep);
    }
}

#[cfg(test)]
#[path = "upgrade.test.rs"]
mod tests;
