//! `deps check`: the installed closure against its remotes. CHECK READS;
//! UPGRADE WRITES: this changes nothing in the project or its pins, and
//! refreshes only ctxloom's clone cache.
//!
//! The check is constraint-aware: an entry is out of date only when a newer
//! commit actually satisfies what its manifest asked for. A sha or an exact
//! tag is a pin and is never out of date. An entry that could NOT be checked
//! (an unreachable remote, an unparseable reference) is reported as unchecked,
//! never folded into "up to date": a check that could not fetch the remote has
//! only the clone it fetched last time to read, and an answer read from that is
//! about the past.

use std::collections::BTreeMap;

use deps::constraint::{self, entry_kind};
use deps::lock::Lock;
use deps::reference::RemoteRef;
use deps::registry::Registry;

use crate::cache::ByAddress;
use crate::closure::{Mode, RepoView, Walk};
use crate::views::{CheckReport, CheckedRef, RefreshFailure, UncheckedRow, UpdateRow};
use crate::{AppError, Deps, Outcome};

impl Deps {
    /// `deps check`, over the whole closure.
    pub fn check(&self) -> Outcome<CheckReport> {
        Outcome {
            result: self.checking(),
            notices: Vec::new(),
        }
    }

    fn checking(&self) -> Result<CheckReport, AppError> {
        let (lock, _) = self.lock()?;
        let mut report = CheckReport {
            entries: 0,
            updates: Vec::new(),
            unchecked: Vec::new(),
            refresh_failures: Vec::new(),
            skipped_empty: 0,
            missing_default_profiles: Vec::new(),
            missing_defaults_error: String::new(),
        };
        if lock.is_empty() {
            return Ok(report);
        }
        let registry = self.registry()?;
        let lock = self.closure_lock(&registry, lock);
        report.entries = lock.entries.len() as u64;
        report.refresh_failures = self.refresh_all(&registry, &lock);
        let failed: BTreeMap<&str, &str> = report
            .refresh_failures
            .iter()
            .map(|f| (f.url.as_str(), f.error.as_str()))
            .collect();
        let mut updates = Vec::new();
        let mut unchecked = Vec::new();
        for (identity, entry) in &lock.entries {
            if entry.sha.is_empty() {
                report.skipped_empty += 1;
                continue;
            }
            // A sha or an exact tag never goes outdated: re-resolving it
            // yields the same commit, so the network is not asked.
            let kind = entry_kind(entry.kind, &entry.version, &entry.requested_version);
            if kind.is_pin() {
                continue;
            }
            match self.latest(identity, &entry.requested_version, &failed) {
                Ok(latest) if latest != entry.sha => updates.push(UpdateRow {
                    reference: identity.clone(),
                    kind: "bundle",
                    current_sha: entry.sha.clone(),
                    latest_sha: latest,
                    selector: kind.name().to_string(),
                    requested: entry.requested_version.clone(),
                    resolved: entry.version.clone(),
                }),
                Ok(_) => {}
                Err(row) => unchecked.push(row),
            }
        }
        report.updates = updates;
        report.unchecked = unchecked;
        Ok(report)
    }

    /// The newest commit `expression` allows in the repository of
    /// `identity`, or why the entry could not be checked.
    fn latest(
        &self,
        identity: &str,
        expression: &str,
        failed: &BTreeMap<&str, &str>,
    ) -> Result<String, UncheckedRow> {
        let row = |url: &str, reason: &'static str, error: String| UncheckedRow {
            reference: identity.to_string(),
            url: url.to_string(),
            constraint: expression.to_string(),
            reason,
            error,
        };
        let Some(remote) = RemoteRef::parse(identity) else {
            return Err(row("", "unparseable", format!("cannot parse {identity}")));
        };
        if let Some(error) = failed.get(remote.url.as_str()) {
            return Err(row(&remote.url, "not_refreshed", (*error).to_string()));
        }
        let view = RepoView {
            remotes: self.remotes.as_ref(),
            url: &remote.url,
        };
        constraint::resolve(expression, &view)
            .map(|found| found.sha)
            .map_err(|cause| row(&remote.url, "unresolvable", cause.to_string()))
    }

    /// Fetch each repository the lock names once; the ones that could not be
    /// fetched (an unregistered one is not fetched at all).
    fn refresh_all(&self, registry: &Registry, lock: &Lock) -> Vec<RefreshFailure> {
        let mut seen = std::collections::BTreeSet::new();
        let mut failures = Vec::new();
        for (identity, entry) in &lock.entries {
            let Some(remote) = RemoteRef::parse(identity) else {
                continue;
            };
            if entry.sha.is_empty() || !seen.insert(remote.url.clone()) {
                continue;
            }
            let outcome = if registry.lookup_url(&remote.url).is_none() {
                Err(format!(
                    "remote not registered: {} is reached only by reference",
                    remote.url
                ))
            } else {
                ByAddress(self.remotes.as_ref()).refresh(&remote.url)
            };
            if let Err(error) = outcome {
                failures.push(RefreshFailure {
                    url: remote.url,
                    error,
                });
            }
        }
        failures
    }

    /// The lock narrowed to the entries the project's closure reaches, so a
    /// check never offers an update for an entry an upgrade would remove. The
    /// whole lock where the closure cannot be known.
    fn closure_lock(&self, registry: &Registry, lock: Lock) -> Lock {
        let Ok(locals) = self.local_profiles() else {
            return lock;
        };
        let mut walk = Walk::new(self, registry, &lock, &locals, Mode::Carried);
        walk.walk_all();
        if walk.refusal().is_some() || !walk.closure.unexpanded().is_empty() {
            return lock;
        }
        let reached: Vec<String> = walk
            .closure
            .pins()
            .into_iter()
            .map(|p| p.identity)
            .collect();
        Lock {
            entries: lock
                .entries
                .iter()
                .filter(|(identity, _)| reached.contains(identity))
                .map(|(identity, entry)| (identity.clone(), entry.clone()))
                .collect(),
        }
    }

    /// `deps check <reference>`: one reference's status. A repository that
    /// could not be fetched is refused, not answered from its stale clone.
    pub fn check_reference(&self, reference: &str) -> Outcome<CheckedRef> {
        Outcome {
            result: self.checking_reference(reference),
            notices: Vec::new(),
        }
    }

    fn checking_reference(&self, reference: &str) -> Result<CheckedRef, AppError> {
        let registry = self.registry()?;
        let expanded = deps::short::expand(
            reference,
            &|alias| registry.get(alias).map(|r| r.url),
            &|_| false,
        );
        let remote = RemoteRef::parse(&expanded).ok_or_else(|| AppError::NotRemoteReference {
            reference: reference.to_string(),
        })?;
        if registry.lookup_url(&remote.url).is_none() {
            return Err(AppError::NotRegistered { url: remote.url });
        }
        ByAddress(self.remotes.as_ref())
            .refresh(&remote.url)
            .map_err(|detail| AppError::CloneNotRefreshed {
                url: remote.url.clone(),
                detail,
            })?;
        let (lock, _) = self.lock()?;
        let current = lock
            .entries
            .get(&remote.identity)
            .map(|e| (e.sha.clone(), e.requested_version.clone()))
            .unwrap_or_default();
        let view = RepoView {
            remotes: self.remotes.as_ref(),
            url: &remote.url,
        };
        let latest = constraint::resolve(&current.1, &view)
            .map(|found| found.sha)
            .map_err(|cause| AppError::Unreadable {
                what: format!("the latest version of {reference}"),
                detail: cause.to_string(),
            })?;
        Ok(CheckedRef {
            reference: reference.to_string(),
            in_lockfile: !current.0.is_empty(),
            up_to_date: !current.0.is_empty() && current.0 == latest,
            current_sha: current.0,
            latest_sha: latest,
            refresh_failures: Vec::new(),
        })
    }
}

#[cfg(test)]
#[path = "check.test.rs"]
mod tests;
