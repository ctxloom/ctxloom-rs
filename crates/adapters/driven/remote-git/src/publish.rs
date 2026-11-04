//! Publishing: files committed to the default branch as one commit, through
//! the user's own git, which owns their identity and credentials. ctxloom
//! supplies neither.

use std::collections::BTreeMap;
use std::path::Path;

use deps_ports::Why;
use file_system::{Fs, Perms};
use os::Mode;

use crate::command::{run, text};
use crate::read::default_branch;

pub(crate) fn publish(
    fs: &dyn Fs,
    cache: &Path,
    url: &str,
    files: &BTreeMap<String, Vec<u8>>,
    message: &str,
) -> Result<String, Why> {
    fs.create_dir_all(cache, Mode::from_bits(0o755))
        .map_err(|e| format!("cannot make {}: {e}", cache.display()))?;
    let work = fs
        .make_temp_dir(cache, "publish-")
        .map_err(|e| format!("cannot make a working clone: {e}"))?;
    let outcome = land(fs, &work, url, files, message);
    let _ = fs.remove_dir_all(&work);
    outcome
}

fn land(
    fs: &dyn Fs,
    work: &Path,
    url: &str,
    files: &BTreeMap<String, Vec<u8>>,
    message: &str,
) -> Result<String, Why> {
    let target = work.to_string_lossy();
    run(None, &["clone", "--", url, &target], None)?;
    let branch = default_branch(work)?;
    run(
        Some(work),
        &[
            "checkout",
            "--quiet",
            "-B",
            &branch,
            &format!("origin/{branch}"),
        ],
        None,
    )?;
    for (path, data) in files {
        let at = work.join(path);
        if let Some(parent) = at.parent() {
            fs.create_dir_all(parent, Mode::from_bits(0o755))
                .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
        }
        fs.write_atomic(&at, data, Perms::Exact(Mode::from_bits(0o644)))
            .map_err(|e| format!("cannot write {}: {e}", at.display()))?;
        run(Some(work), &["add", "--", path], None)?;
    }
    // Nothing staged is nothing to publish: the commit at the tip stands.
    if run(Some(work), &["diff", "--cached", "--quiet"], None).is_err() {
        run(Some(work), &["commit", "--quiet", "-m", message], None)?;
        run(
            Some(work),
            &[
                "push",
                "--quiet",
                "origin",
                &format!("HEAD:refs/heads/{branch}"),
            ],
            None,
        )?;
    }
    text(work, &["rev-parse", "HEAD"])
}
