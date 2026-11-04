//! What the commands answer, in the shapes they print as json (Go's
//! `operations` results and the `cli` views): every list is always present.

use serde::Serialize;

pub use pin_changes::PinChange;

fn not(flag: &bool) -> bool {
    !*flag
}

/// A registered remote, as a listing shows it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct RemoteRow {
    pub name: String,
    pub url: String,
}

/// `remote list`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct RemoteListing {
    pub remotes: Vec<RemoteRow>,
    pub count: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default: String,
}

/// `remote create`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Created {
    pub status: &'static str,
    pub name: String,
    pub url: String,
    /// The clone that failed, said: the remote stays registered.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub warning: String,
}

/// A remote as an edit shows it before and after.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct RemoteState {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub forge: String,
}

/// `remote edit`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct EditedRemote {
    pub status: &'static str,
    pub name: String,
    pub before: RemoteState,
    pub after: RemoteState,
    pub changed: Vec<String>,
    #[serde(skip_serializing_if = "not")]
    pub default_pointer_updated: bool,
}

/// `remote remove`: what it would remove, or what it removed.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(untagged)]
pub enum RemoveResult {
    Preview {
        status: &'static str,
        applied: bool,
        target: String,
        apply: String,
    },
    Removed {
        status: &'static str,
        name: String,
    },
}

/// `remote default`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct DefaultSet {
    pub status: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// One bundle a remote publishes.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct BrowseItem {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub path: String,
    pub is_dir: bool,
    pub pull_ref: String,
}

/// `remote show`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Browsed {
    pub remote: String,
    pub url: String,
    pub items: Vec<BrowseItem>,
    pub count: u64,
    pub warnings: Vec<String>,
}

/// One installed dependency, as the listing shows it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct DepRow {
    pub name: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub sha: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub constraint: String,
    pub held: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub origin: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
}

/// `deps list`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct DepsListing {
    pub deps: Vec<DepRow>,
    pub count: u64,
}

/// One reference a pull went over, and what it did with it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct SyncItem {
    pub reference: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// `installed`, `reinstalled`, `skipped` or `failed`.
    pub status: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

/// An item that failed, with the fix its failure names.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct FailedItem {
    pub reference: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub status: &'static str,
    pub error: String,
    pub fix: String,
}

/// A pin kept at its commit although the manifest asks for something else.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct ConstraintChangeView {
    pub identity: String,
    pub pinned: String,
    pub declared: String,
    pub sha: String,
}

/// A repository a reconcile could not establish the state of.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct UncheckedRemote {
    pub url: String,
    pub reason: String,
    pub refs: Vec<String>,
}

/// What a reconcile found.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct ReconcileView {
    pub gone: Vec<String>,
    pub unreachable: Vec<UncheckedRemote>,
}

/// `deps pull`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct PullReport {
    pub status: &'static str,
    pub total: u64,
    pub installed: u64,
    pub reinstalled: u64,
    pub errors: u64,
    pub synced: Vec<SyncItem>,
    pub skipped: Vec<SyncItem>,
    pub failed: Vec<FailedItem>,
    pub removed: Vec<String>,
    pub pruned_checkouts: Vec<String>,
    pub incomplete: bool,
    pub unreachable: Vec<String>,
    pub constraint_changes: Vec<ConstraintChangeView>,
    pub changes: Vec<PinChange>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reconcile: Option<ReconcileView>,
}

/// An entry a newer commit satisfies the constraint for.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct UpdateRow {
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub current_sha: String,
    pub latest_sha: String,
    pub selector: String,
    pub requested: String,
    pub resolved: String,
}

/// An entry that was neither verified current nor found outdated.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct UncheckedRow {
    #[serde(rename = "ref")]
    pub reference: String,
    pub url: String,
    pub constraint: String,
    /// `unparseable`, `unresolvable` or `not_refreshed`.
    pub reason: &'static str,
    pub error: String,
}

/// A clone that could not be refreshed before a check.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct RefreshFailure {
    pub url: String,
    pub error: String,
}

/// `deps check`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CheckReport {
    pub entries: u64,
    pub updates: Vec<UpdateRow>,
    pub unchecked: Vec<UncheckedRow>,
    pub refresh_failures: Vec<RefreshFailure>,
    pub skipped_empty: u64,
    pub missing_default_profiles: Vec<String>,
    pub missing_defaults_error: String,
}

/// `deps check <reference>`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct CheckedRef {
    #[serde(rename = "ref")]
    pub reference: String,
    pub in_lockfile: bool,
    pub current_sha: String,
    pub latest_sha: String,
    pub up_to_date: bool,
    pub refresh_failures: Vec<RefreshFailure>,
}

/// `deps upgrade`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct UpgradeReport {
    pub applied: bool,
    pub changes: Vec<PinChange>,
    pub incomplete: bool,
    pub nothing_declared: bool,
    pub removed: Vec<String>,
}

/// `deps hold` and `deps unhold`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct HoldResult {
    /// `held`, `released`, or `not_tracked` for a bundle of this project.
    pub status: &'static str,
    pub name: String,
}

/// `bundle push`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Pushed {
    pub status: &'static str,
    pub path: String,
    pub remote: String,
    pub target_path: String,
    pub title: String,
    pub create_pr: bool,
    pub size_bytes: u64,
    pub commit_sha: String,
}

/// `bundle move`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Moved {
    pub status: &'static str,
    pub name: String,
    pub source: String,
    pub dest_kind: &'static str,
    pub dest: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub remote: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub commit_sha: String,
}

#[cfg(test)]
#[path = "views.test.rs"]
mod tests;
