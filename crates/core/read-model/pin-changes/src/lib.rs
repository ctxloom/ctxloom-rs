//! The disclosure `deps upgrade` shows before `--yes` applies it, and `deps
//! pull` shows for the first pins it makes: every item of a bundle added,
//! removed or changed, what each hook and MCP server runs before and after,
//! and the diff of every changed script (Go's `operations.PinChange`).
//!
//! A pin is the decision to run what a bundle ships, so moving one is shown
//! before it happens. Env and header VALUES may be credentials: they are shown
//! only by name and a fingerprint of the value (the first 8 hex characters of
//! its SHA-256), so a changed value is visible and a secret is not. The
//! fingerprint is taken at the source, so no rendering, text or json, can
//! carry a raw secret.

mod diff;
mod item;

use std::collections::BTreeMap;

use deps::version::{classify, Compat};
use library::model::Bundle;
use library::tree::{read_tree, Files};
use library::BundleError;
use serde::Serialize;

pub use diff::unified_diff;
pub use item::{diff_items, fingerprint};

/// One file of a bundle's tree.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TreeFile {
    pub data: Vec<u8>,
    /// Whether the file is executable: declared so by its package's sidecar,
    /// or committed so.
    pub executable: bool,
}

/// A bundle's tree: its files by path relative to the bundle.
pub type Tree = BTreeMap<String, TreeFile>;

/// How an item or a file differs between two pins.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Removed,
    Modified,
}

/// What a hook or an MCP server executes or dials. Env and headers hold each
/// value's fingerprint, never the value.
#[derive(Clone, Default, PartialEq, Eq, Debug, Serialize)]
pub struct ExecSpec {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

/// An executable item's spec at the old pin (none when added) and at the new
/// one (none when removed).
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct ExecDelta {
    pub before: Option<ExecSpec>,
    pub after: Option<ExecSpec>,
}

/// One item that differs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct ItemChange {
    /// `hook`, `mcp`, `skill`, `command`, `fragment` or `profile`.
    pub kind: &'static str,
    pub name: String,
    pub change: Change,
    /// What a hook or MCP server runs; none for every other kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecDelta>,
}

/// One file of the tree that differs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct FileChange {
    pub path: String,
    pub change: Change,
    /// A unified diff, set only for a script: an executable file, or one
    /// under a `scripts/` directory.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub diff: String,
}

/// What moving (or first making) one pin brings in.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct PinChange {
    pub identity: String,
    pub url: String,
    /// Empty for a first pin, whose items and files are then the bundle's
    /// whole initial content.
    pub from_sha: String,
    pub to_sha: String,
    pub from_version: String,
    pub to_version: String,
    /// What the move is worth to whoever depends on the bundle, by semver
    /// where the versions parse as it: from the tags both sides chose, else
    /// (a dependency that follows a branch has no tag) from the versions the
    /// two bundles declare. None for a first pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<Compat>,
    pub items: Vec<ItemChange>,
    pub files: Vec<FileChange>,
}

/// A tree that could not be read as a bundle: the change is listed by its
/// header and its files alone, and this says why.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unreadable {
    pub identity: String,
    /// The commit whose tree it was.
    pub sha: String,
    pub cause: BundleError,
}

/// The pin a change is read from: its commit, the tag it chose, its tree.
pub struct Side<'a> {
    pub sha: &'a str,
    pub version: &'a str,
    pub tree: &'a Tree,
}

/// What moving `identity` of `url` from `from` (none for a first pin) to `to`
/// brings in, and why the content of either side could not be read as a
/// bundle, if it could not: the change is then listed by its header and its
/// files alone.
pub fn diff_pin(
    identity: &str,
    url: &str,
    from: Option<Side<'_>>,
    to: Side<'_>,
) -> (PinChange, Vec<Unreadable>) {
    let mut unreadable = Vec::new();
    let mut bundle_of = |side: &Side<'_>| {
        let files: Files = side
            .tree
            .iter()
            .map(|(path, file)| (path.clone(), file.data.clone()))
            .collect();
        read_tree(identity, &files).unwrap_or_else(|cause| {
            unreadable.push(Unreadable {
                identity: identity.to_string(),
                sha: side.sha.to_string(),
                cause,
            });
            Bundle::default()
        })
    };
    let from_bundle = from.as_ref().map(&mut bundle_of).unwrap_or_default();
    let to_bundle = bundle_of(&to);
    let compat = from.as_ref().map(|side| {
        if !side.version.is_empty() && !to.version.is_empty() {
            classify(side.version, to.version)
        } else {
            classify(&from_bundle.version, &to_bundle.version)
        }
    });
    let empty = Tree::new();
    let from_tree = from.as_ref().map_or(&empty, |side| side.tree);
    let change = PinChange {
        identity: identity.to_string(),
        url: url.to_string(),
        from_sha: from.as_ref().map(|s| s.sha.to_string()).unwrap_or_default(),
        to_sha: to.sha.to_string(),
        from_version: from
            .as_ref()
            .map(|s| s.version.to_string())
            .unwrap_or_default(),
        to_version: to.version.to_string(),
        compat,
        items: diff_items(&from_bundle, &to_bundle),
        files: diff_files(from_tree, to.tree),
    };
    (change, unreadable)
}

/// The files that differ between two trees, a first tree's paths (sorted)
/// then the paths only the second has (sorted). A script is shown as a diff.
fn diff_files(from: &Tree, to: &Tree) -> Vec<FileChange> {
    let paths = from
        .keys()
        .chain(to.keys().filter(|path| !from.contains_key(*path)));
    let mut out = Vec::new();
    for path in paths {
        let (before, after) = (from.get(path), to.get(path));
        let change = match (before, after) {
            (None, Some(_)) => Change::Added,
            (Some(_), None) => Change::Removed,
            (Some(b), Some(a)) if b != a => Change::Modified,
            _ => continue,
        };
        let is_script = [before, after].into_iter().flatten().any(|file| {
            file.executable || path.starts_with("scripts/") || path.contains("/scripts/")
        });
        let text = |file: Option<&TreeFile>| {
            file.map(|f| String::from_utf8_lossy(&f.data).into_owned())
                .unwrap_or_default()
        };
        out.push(FileChange {
            path: path.clone(),
            change,
            diff: if is_script {
                unified_diff(path, &text(before), &text(after))
            } else {
                String::new()
            },
        });
    }
    out
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
