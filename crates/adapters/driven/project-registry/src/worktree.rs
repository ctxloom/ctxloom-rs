//! Linked worktrees, found as git finds them: a worktree's `.git` is a file
//! naming its git directory (`gitdir: <path>`), which holds a `commondir`
//! file naming the repository's common directory. The primary checkout is
//! the directory that common directory, `.git`, is in.

use std::fs;
use std::path::{Path, PathBuf};

use crate::path::canonical;

/// The primary checkout of the linked worktree at `root`, canonical; none
/// when `root` is not a linked worktree (its `.git` is a directory, or
/// absent, or names a git directory with no `commondir`, as a submodule's
/// does), or when its repository is bare and has no primary checkout.
pub fn primary_checkout(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if !dot_git.is_file() {
        return None;
    }
    let text = fs::read_to_string(&dot_git).ok()?;
    let git_dir = root.join(text.trim().strip_prefix("gitdir:")?.trim());
    let common = fs::read_to_string(git_dir.join("commondir")).ok()?;
    let common = canonical(&git_dir.join(common.trim()));
    if common.file_name()? != ".git" {
        return None;
    }
    let primary = common.parent()?.to_path_buf();
    (primary != root).then_some(primary)
}

#[cfg(test)]
#[path = "worktree.test.rs"]
mod tests;
