//! `ctxloom clean`: the project's regenerable cache, and nothing else.
//!
//! The selection is the cache directory's three trees (Go's `paths.Layout`
//! narrowed to `.ctxloom/cache`): `lock.yaml` is derived too, but committed,
//! so removing it would dirty the tree rather than free anything; and the
//! local-only state (the project id, sessions, approvals) is rebuilt by
//! nothing, so not even `--yes` takes it.

use std::sync::Arc;

use ctxloom_ports::ProjectCache;
use serde::Serialize;

use crate::ReclaimError;

/// A cache tree below the app dir, and the command that rebuilds it.
pub const TARGETS: [(&str, &str); 3] = [
    ("cache/bundles", "ctxloom deps pull"),
    ("cache/repos", "ctxloom deps pull"),
    ("cache/context", "ctxloom run (a launch rewrites it)"),
];

/// One regenerable path.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CleanTarget {
    /// As the project shows it: `.ctxloom/cache/context`.
    pub rel: String,
    /// Where it resolves on this machine.
    pub path: String,
    /// Absent targets are reported, not hidden.
    pub present: bool,
    /// What removing it reclaims; zero when absent.
    pub bytes: u64,
    /// The command that rebuilds it.
    pub rebuild: String,
    /// Whether this run removed it.
    pub removed: bool,
}

/// One clean run: what it would remove, or did.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CleanReport {
    pub applied: bool,
    pub targets: Vec<CleanTarget>,
    pub bytes: u64,
}

/// The cache cleaner.
pub struct Cleaner {
    cache: Arc<dyn ProjectCache>,
}

impl Cleaner {
    pub fn new(cache: Arc<dyn ProjectCache>) -> Cleaner {
        Cleaner { cache }
    }

    /// Plan the clean, and with `apply` do it.
    pub fn clean(&self, apply: bool) -> Result<CleanReport, ReclaimError> {
        let app = self.cache.app_dir();
        let mut report = CleanReport {
            applied: apply,
            targets: Vec::new(),
            bytes: 0,
        };
        for (rel, rebuild) in TARGETS {
            let measured = self.cache.measure(rel).map_err(ReclaimError::Cache)?;
            let mut target = CleanTarget {
                rel: format!(".ctxloom/{rel}"),
                path: format!("{}/{rel}", app.trim_end_matches('/')),
                present: measured.is_some(),
                bytes: measured.unwrap_or(0),
                rebuild: rebuild.to_string(),
                removed: false,
            };
            if apply && target.present {
                self.cache.remove(rel).map_err(ReclaimError::Cache)?;
                target.removed = true;
            }
            report.bytes += target.bytes;
            report.targets.push(target);
        }
        Ok(report)
    }
}
