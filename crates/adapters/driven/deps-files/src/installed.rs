//! The trees installed from remotes.

use std::path::{Path, PathBuf};

use deps_ports::{BundleSlot, RepoFile, RepoTree, Why};
use file_system::Perms;
use os::Mode;

use crate::ProjectFiles;

/// The file at the root of an installed tree naming the identity it is the
/// tree of.
pub const IDENTITY_MARKER: &str = ".identity";

/// Where installed trees are, below the app dir.
const BELOW_APP_DIR: &str = "cache/bundles";

fn root(files: &ProjectFiles) -> PathBuf {
    files.app_dir.join(BELOW_APP_DIR)
}

/// The directory holding every commit installed for `slot`'s bundle.
fn bundle_dir(files: &ProjectFiles, slot: &BundleSlot) -> PathBuf {
    let mut dir = root(files);
    for segment in slot
        .repository
        .segments()
        .iter()
        .chain(slot.bundle.segments())
    {
        dir.push(segment);
    }
    dir
}

/// Where the tree installed for `identity` at `sha` is (or would be).
pub(crate) fn path(files: &ProjectFiles, slot: &BundleSlot, sha: &str) -> String {
    bundle_dir(files, slot).join(sha).display().to_string()
}

pub(crate) fn is_installed(files: &ProjectFiles, slot: &BundleSlot, sha: &str) -> bool {
    let manifest = bundle_dir(files, slot).join(sha).join("bundle.yaml");
    files.fs.exists(&manifest).unwrap_or(false)
}

/// Install the tree, replacing whatever was installed for the identity: the new
/// tree is written beside, then the old ones go and it is renamed into place.
pub(crate) fn install(
    files: &ProjectFiles,
    slot: &BundleSlot,
    sha: &str,
    tree: &RepoTree,
) -> Result<(), Why> {
    let base = bundle_dir(files, slot);
    files
        .fs
        .create_dir_all(&base, Mode::from_bits(0o755))
        .map_err(|e| e.to_string())?;
    // The old trees are found before the staging directory exists, so it is
    // never among them.
    let old_trees = installed_trees(files, &base);
    let staging = files
        .fs
        .make_temp_dir(&base, ".install-")
        .map_err(|e| e.to_string())?;
    let written = write_tree(files, &staging, &slot.identity, tree);
    if let Err(why) = written {
        let _ = files.fs.remove_dir_all(&staging);
        return Err(why);
    }
    for old in old_trees {
        files
            .fs
            .remove_dir_all(&base.join(old))
            .map_err(|e| e.to_string())?;
    }
    files
        .fs
        .rename(&staging, &base.join(sha))
        .map_err(|e| e.to_string())
}

fn write_tree(
    files: &ProjectFiles,
    dir: &Path,
    identity: &str,
    tree: &RepoTree,
) -> Result<(), Why> {
    for (path, file) in tree {
        let at = dir.join(path);
        if let Some(parent) = at.parent() {
            files
                .fs
                .create_dir_all(parent, Mode::from_bits(0o755))
                .map_err(|e| e.to_string())?;
        }
        let mode = if file.executable { 0o755 } else { 0o644 };
        files
            .fs
            .write_atomic(&at, &file.data, Perms::Exact(Mode::from_bits(mode)))
            .map_err(|e| e.to_string())?;
    }
    files
        .fs
        .write_atomic(
            &dir.join(IDENTITY_MARKER),
            identity.as_bytes(),
            Perms::Exact(Mode::from_bits(0o644)),
        )
        .map_err(|e| e.to_string())
}

/// The trees installed in `base`: its subdirectories holding the identity
/// marker (a nested bundle's directory holds none, only its own trees do).
fn installed_trees(files: &ProjectFiles, base: &Path) -> Vec<String> {
    let Ok(entries) = files.fs.read_dir(base) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .filter(|name| {
            files
                .fs
                .exists(&base.join(name).join(IDENTITY_MARKER))
                .unwrap_or(false)
        })
        .collect()
}

pub(crate) fn read(
    files: &ProjectFiles,
    slot: &BundleSlot,
    sha: &str,
) -> Result<Option<RepoTree>, Why> {
    let dir = bundle_dir(files, slot).join(sha);
    if !files.fs.exists(&dir.join("bundle.yaml")).unwrap_or(false) {
        return Ok(None);
    }
    let mut tree = RepoTree::new();
    collect(files, &dir, &dir, &mut tree)?;
    Ok(Some(tree))
}

fn collect(files: &ProjectFiles, top: &Path, dir: &Path, tree: &mut RepoTree) -> Result<(), Why> {
    for entry in files.fs.read_dir(dir).map_err(|e| e.to_string())? {
        let path = dir.join(&entry.name);
        match files.kind_of(&path) {
            Some(file_system::Kind::Dir) => collect(files, top, &path, tree)?,
            Some(file_system::Kind::File) => {
                let relative = path
                    .strip_prefix(top)
                    .unwrap_or(&path)
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                if relative == IDENTITY_MARKER {
                    continue;
                }
                let meta = files.fs.metadata(&path).map_err(|e| e.to_string())?;
                tree.insert(
                    relative,
                    RepoFile {
                        data: files.fs.read(&path).map_err(|e| e.to_string())?,
                        executable: meta.executable,
                    },
                );
            }
            _ => {}
        }
    }
    Ok(())
}

/// Remove every installed tree whose (identity, commit) is not in `keep`;
/// the identities that have nothing left.
pub(crate) fn prune(files: &ProjectFiles, keep: &[(String, String)]) -> Result<Vec<String>, Why> {
    let mut trees = Vec::new();
    find(files, &root(files), &mut trees);
    let mut gone = Vec::new();
    for dir in trees {
        let sha = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let identity = files
            .read_text(&dir.join(IDENTITY_MARKER))?
            .unwrap_or_default();
        if keep.iter().any(|(i, s)| *i == identity && *s == sha) {
            continue;
        }
        files.fs.remove_dir_all(&dir).map_err(|e| e.to_string())?;
        if !keep.iter().any(|(i, _)| *i == identity) && !gone.contains(&identity) {
            gone.push(identity);
        }
        remove_empty_parents(files, &dir);
    }
    gone.retain(|identity| !identity.is_empty());
    Ok(gone)
}

/// The directories below `dir` that are an installed tree: they hold a marker.
fn find(files: &ProjectFiles, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = files.fs.read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = dir.join(&entry.name);
        if files.kind_of(&path) != Some(file_system::Kind::Dir) {
            continue;
        }
        if files
            .fs
            .exists(&path.join(IDENTITY_MARKER))
            .unwrap_or(false)
        {
            out.push(path);
        } else {
            find(files, &path, out);
        }
    }
}

/// An emptied directory goes, up to the cache root.
fn remove_empty_parents(files: &ProjectFiles, dir: &Path) {
    let stop = root(files);
    let mut at = dir.parent();
    while let Some(parent) = at.filter(|p| *p != stop && p.starts_with(&stop)) {
        match files.fs.read_dir(parent) {
            Ok(entries) if entries.is_empty() => {
                if files.fs.remove_dir_all(parent).is_err() {
                    return;
                }
                at = parent.parent();
            }
            _ => return,
        }
    }
}
