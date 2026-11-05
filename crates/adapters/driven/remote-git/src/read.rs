//! Reading a clone: versions, trees and listings, from its objects and
//! remote-tracking refs alone.

use std::path::Path;

use deps_ports::{RepoFile, RepoTree, Why};

use crate::command::Git;
use crate::BUNDLES;

pub(crate) fn tags(git: &Git, dir: &Path) -> Result<Vec<String>, Why> {
    Ok(git
        .text(dir, &["tag", "--list"])?
        .lines()
        .map(str::to_string)
        .collect())
}

/// A name as a commit: a branch (its remote-tracking tip), then a tag, then a
/// commit name.
pub(crate) fn resolve_ref(git: &Git, dir: &Path, name: &str) -> Result<String, Why> {
    let mut candidates = vec![
        format!("refs/remotes/origin/{name}^{{commit}}"),
        format!("refs/tags/{name}^{{commit}}"),
    ];
    if name.len() >= 4 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
        candidates.push(format!("{name}^{{commit}}"));
    }
    for candidate in candidates {
        if let Ok(sha) = git.text(dir, &["rev-parse", "--verify", "--quiet", &candidate]) {
            if !sha.is_empty() {
                return Ok(sha);
            }
        }
    }
    Err(format!("unknown revision {name}"))
}

pub(crate) fn resolve_tag(git: &Git, dir: &Path, name: &str) -> Result<String, Why> {
    let wanted = format!("refs/tags/{name}^{{commit}}");
    match git.text(dir, &["rev-parse", "--verify", "--quiet", &wanted]) {
        Ok(sha) if !sha.is_empty() => Ok(sha),
        _ => Err(format!("unknown tag {name}")),
    }
}

/// The default branch: what the remote's HEAD points at, else `main` or
/// `master` where the clone has one.
pub(crate) fn default_branch(git: &Git, dir: &Path) -> Result<String, Why> {
    if let Ok(head) = git.text(
        dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        if let Some(branch) = head.strip_prefix("origin/") {
            return Ok(branch.to_string());
        }
    }
    for candidate in ["main", "master"] {
        let reference = format!("refs/remotes/origin/{candidate}");
        if git
            .text(dir, &["show-ref", "--verify", "--quiet", &reference])
            .is_ok()
        {
            return Ok(candidate.to_string());
        }
    }
    Err("cannot determine the repository's default branch".to_string())
}

/// The commit at the tip of the default branch.
fn tip(git: &Git, dir: &Path) -> Result<String, Why> {
    let branch = default_branch(git, dir)?;
    resolve_ref(git, dir, &branch)
}

/// The entries of `git ls-tree -r -z` below `prefix` at `sha`: (mode, object,
/// path).
fn listing(
    git: &Git,
    dir: &Path,
    sha: &str,
    prefix: &str,
) -> Result<Vec<(String, String, String)>, Why> {
    let out = git.run(
        Some(dir),
        &["ls-tree", "-r", "-z", "--full-tree", sha, "--", prefix],
        None,
    )?;
    let mut entries = Vec::new();
    for record in out.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let record = String::from_utf8_lossy(record);
        let Some((meta, path)) = record.split_once('\t') else {
            continue;
        };
        let mut fields = meta.split(' ');
        if let (Some(mode), Some("blob"), Some(object)) =
            (fields.next(), fields.next(), fields.next())
        {
            entries.push((mode.to_string(), object.to_string(), path.to_string()));
        }
    }
    Ok(entries)
}

/// The files of the bundle at `sha`, by path relative to it; none where the
/// commit has no such directory. A link is not a file of the tree.
pub(crate) fn bundle(
    git: &Git,
    dir: &Path,
    sha: &str,
    bundle: &str,
) -> Result<Option<RepoTree>, Why> {
    let prefix = format!("{BUNDLES}/{bundle}/");
    let entries: Vec<_> = listing(git, dir, sha, &prefix)?
        .into_iter()
        .filter(|(mode, _, _)| mode != "120000")
        .collect();
    if entries.is_empty() {
        return Ok(None);
    }
    let objects: Vec<&str> = entries
        .iter()
        .map(|(_, object, _)| object.as_str())
        .collect();
    let contents = blobs(git, dir, &objects)?;
    let mut tree = RepoTree::new();
    for ((mode, _, path), data) in entries.iter().zip(contents) {
        let relative = path.strip_prefix(&prefix).unwrap_or(path).to_string();
        tree.insert(
            relative,
            RepoFile {
                data,
                executable: mode == "100755",
            },
        );
    }
    Ok(Some(tree))
}

/// The contents of `objects`, in order, from one `cat-file --batch`.
fn blobs(git: &Git, dir: &Path, objects: &[&str]) -> Result<Vec<Vec<u8>>, Why> {
    let request = objects.iter().map(|o| format!("{o}\n")).collect::<String>();
    let out = git.run(
        Some(dir),
        &["cat-file", "--batch"],
        Some(request.as_bytes()),
    )?;
    let mut at = 0;
    let mut contents = Vec::with_capacity(objects.len());
    for object in objects {
        let end = out[at..]
            .iter()
            .position(|b| *b == b'\n')
            .map(|i| at + i)
            .ok_or_else(|| format!("git cat-file: no answer for {object}"))?;
        let header = String::from_utf8_lossy(&out[at..end]).into_owned();
        let size = match header.split(' ').collect::<Vec<_>>().as_slice() {
            [_, "blob", size] => size.parse::<usize>().ok(),
            _ => None,
        }
        .ok_or_else(|| format!("git cat-file: {header}"))?;
        let start = end + 1;
        contents.push(
            out.get(start..start + size)
                .ok_or_else(|| format!("git cat-file: {object} is cut short"))?
                .to_vec(),
        );
        at = start + size + 1;
    }
    Ok(contents)
}

/// The names of the bundles at the default branch's tip: a directory holding a
/// `bundle.yaml`, however deep, and not one inside another bundle.
pub(crate) fn bundles(git: &Git, dir: &Path) -> Result<Vec<String>, Why> {
    let prefix = format!("{BUNDLES}/");
    let mut found: Vec<String> = listing(git, dir, &tip(git, dir)?, &prefix)?
        .into_iter()
        .filter_map(|(_, _, path)| {
            let relative = path.strip_prefix(&prefix)?;
            let bundle = relative.strip_suffix("/bundle.yaml")?;
            Some(bundle.to_string())
        })
        .collect();
    found.sort();
    let all = found.clone();
    found.retain(|name| {
        !all.iter()
            .any(|outer| name.starts_with(&format!("{outer}/")))
    });
    Ok(found)
}

/// Whether the default branch's tip has the bundle's manifest.
pub(crate) fn has_bundle(git: &Git, dir: &Path, bundle: &str) -> Result<bool, Why> {
    let path = format!("{BUNDLES}/{bundle}/bundle.yaml");
    let out = git.run(
        Some(dir),
        &["ls-tree", "--name-only", &tip(git, dir)?, "--", &path],
        None,
    )?;
    Ok(!out.is_empty())
}
