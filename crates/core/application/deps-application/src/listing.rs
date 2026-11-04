//! `deps list`, `deps hold` and `deps unhold`.

use deps::address::same_repository;
use deps::reference::RemoteRef;
use deps::short;

use crate::views::{DepRow, DepsListing, HoldResult};
use crate::{AppError, Deps, Notice, Outcome};

impl Deps {
    /// `deps list`: what is installed, from the lock alone. It is the one
    /// question still answerable with no network, an expired credential or a
    /// remote that has been deleted, and so it says nothing of whether
    /// anything newer exists upstream.
    pub fn list(&self) -> Outcome<DepsListing> {
        Outcome {
            result: self.listing(),
            notices: Vec::new(),
        }
    }

    fn listing(&self) -> Result<DepsListing, AppError> {
        let (lock, _) = self.lock()?;
        // A registry that will not load is not fatal here: the origin is a
        // convenience over an identity the lock carries in full.
        let registry = self
            .registry()
            .unwrap_or_else(|_| deps::registry::Registry::empty());
        let mut rows: Vec<DepRow> = lock
            .entries
            .iter()
            .map(|(identity, entry)| {
                let remote = RemoteRef::parse(identity);
                let url = if entry.url.is_empty() {
                    remote.as_ref().map(|r| r.url.clone()).unwrap_or_default()
                } else {
                    entry.url.clone()
                };
                let name = remote
                    .as_ref()
                    .and_then(|r| r.bundle.rsplit('/').next().map(str::to_string))
                    .unwrap_or_else(|| identity.clone());
                DepRow {
                    name,
                    reference: identity.clone(),
                    sha: entry.sha.clone(),
                    constraint: entry.requested_version.clone(),
                    held: entry.held,
                    origin: registry
                        .remotes()
                        .into_iter()
                        .find(|r| same_repository(&r.url, &url))
                        .map(|r| r.name)
                        .unwrap_or_default(),
                    url,
                }
            })
            .collect();
        // Sorted, so two runs over the same lock print the same thing.
        rows.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.reference.cmp(&b.reference))
        });
        Ok(DepsListing {
            count: rows.len() as u64,
            deps: rows,
        })
    }

    /// `deps hold` (`held`) and `deps unhold`: freeze one locked dependency
    /// against `upgrade`, or release it. A hold is local policy: it edits the
    /// lock, not the bundle. Naming a bundle of this project holds nothing,
    /// and says so; naming nothing at all is refused.
    pub fn hold(&self, name: &str, held: bool) -> Outcome<HoldResult> {
        let mut notices = Vec::new();
        let result = self.holding(name, held, &mut notices);
        Outcome { result, notices }
    }

    fn holding(
        &self,
        name: &str,
        held: bool,
        notices: &mut Vec<Notice>,
    ) -> Result<HoldResult, AppError> {
        let verb = if held { "hold" } else { "unhold" };
        let (mut lock, text) = self.lock()?;
        let registry = self
            .registry()
            .unwrap_or_else(|_| deps::registry::Registry::empty());
        let local = |base: &str| self.files.local_bundle(base).ok().flatten().is_some();
        let expanded = short::expand(name, &|alias| registry.get(alias).map(|r| r.url), &local);
        let identity = RemoteRef::parse(&expanded).map(|r| r.identity);
        if let Some(entry) = identity.and_then(|id| lock.entries.get_mut(&id)) {
            if entry.held != held {
                entry.held = held;
                self.save_lock(text.as_deref(), &lock, false)?;
            }
            return Ok(HoldResult {
                status: if held { "held" } else { "released" },
                name: name.to_string(),
            });
        }
        match self.files.local_bundle(name) {
            Ok(Some(_)) => {
                notices.push(Notice::text(format!(
                    "{name:?} is a local bundle, not tracked in the active lockfile; nothing to {verb}. `deps upgrade` never advances a local bundle, so it is already frozen."
                )));
                Ok(HoldResult {
                    status: "not_tracked",
                    name: name.to_string(),
                })
            }
            _ => Err(AppError::NothingToHold {
                name: name.to_string(),
                verb: verb.to_string(),
            }),
        }
    }
}

#[cfg(test)]
#[path = "listing.test.rs"]
mod tests;
