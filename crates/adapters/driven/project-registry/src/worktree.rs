//! Linked worktrees, found as git finds them: a worktree's `.git` is a file
//! naming its git directory (`gitdir: <path>`), which holds a `commondir`
//! file naming the repository's common directory. The primary checkout is
//! the directory that common directory, `.git`, is in.

use std::fs;
use std::path::{Path, PathBuf};

use crate::path::canonical;
use crate::RegistryError;

/// The primary checkout of the linked worktree at `root`, canonical; none
/// when `root` is not a linked worktree (its `.git` is a directory, or
/// absent, or names a git directory with no `commondir`, as a submodule's
/// does), or when its repository is bare and has no primary checkout. A
/// `.git` naming a git directory that is gone (the primary checkout was
/// deleted or moved) is an error, as in Go: resolving the worktree on its
/// own would mint a project nobody finds again.
pub fn primary_checkout(root: &Path) -> Result<Option<PathBuf>, RegistryError> {
    let dot_git = root.join(".git");
    if !dot_git.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&dot_git).map_err(|e| RegistryError::io(&dot_git, e))?;
    let Some(named) = text.trim().strip_prefix("gitdir:") else {
        return Ok(None);
    };
    let git_dir = root.join(named.trim());
    if !git_dir.is_dir() {
        return Err(RegistryError::StaleWorktree {
            root: root.display().to_string(),
            git_dir: git_dir.display().to_string(),
        });
    }
    Ok(primary_of(&git_dir).filter(|primary| primary != root))
}

/// The checkout the repository of the git directory `git_dir` is kept in,
/// through its `commondir`.
fn primary_of(git_dir: &Path) -> Option<PathBuf> {
    let common = fs::read_to_string(git_dir.join("commondir")).ok()?;
    let common = canonical(&git_dir.join(common.trim()));
    if common.file_name()? != ".git" {
        return None;
    }
    Some(common.parent()?.to_path_buf())
}

#[cfg(test)]
#[path = "worktree.test.rs"]
mod tests;
