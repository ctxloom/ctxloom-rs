//! The `Remotes` port over the git command line (the plan's REC: the git CLI
//! behind the port first, not gix).
//!
//! A repository is cloned once into the cache, `<cache root>/<host>/<path>`
//! (every spelling of one repository caching in one directory), with its full
//! history. Every read is from commits and remote-tracking refs, never from a
//! checked-out working tree: a clone is made without a checkout, and a stale
//! one that someone reset must not leak into what is served.
//!
//! git is run with the repository-location variables of its environment
//! (`GIT_DIR` and its siblings, `git -c`'s exports) removed, so the working
//! directory and the repository's own configuration alone choose the
//! repository; and non-interactive: a private, renamed or mistyped address
//! fails fast rather than waiting on a prompt nobody can answer. Credentials
//! are git's own: its credential helpers, ssh keys and agent, and the
//! committer's identity from the user's configuration.

mod command;
mod publish;
mod read;

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use deps::address::cache_segments;
use deps_ports::{Remotes, RepoTree, Why};
use file_system::Fs;

/// Where bundles are in a repository.
pub const BUNDLES: &str = ".ctxloom/content/bundles/v2";

/// The repositories content comes from, over git.
pub struct GitRemotes {
    fs: Arc<dyn Fs>,
    cache: PathBuf,
}

impl GitRemotes {
    /// Remotes cloned below `cache` (`<app dir>/cache/repos`), on `fs`.
    pub fn new(fs: Arc<dyn Fs>, cache: PathBuf) -> GitRemotes {
        GitRemotes { fs, cache }
    }

    /// Where the clone of `url` is cached; why it has no place if its address
    /// names no repository.
    fn clone_dir(&self, url: &str) -> Result<PathBuf, Why> {
        let segments = cache_segments(url)
            .ok_or_else(|| format!("refusing to cache {url:?}: it names no repository"))?;
        Ok(segments
            .iter()
            .fold(self.cache.clone(), |dir, s| dir.join(s)))
    }

    fn is_clone(&self, dir: &Path) -> bool {
        self.fs
            .metadata(&dir.join(".git"))
            .is_ok_and(|meta| meta.kind == file_system::Kind::Dir)
    }

    /// The clone of `url`, which must be there.
    fn cloned(&self, url: &str) -> Result<PathBuf, Why> {
        let dir = self.clone_dir(url)?;
        if self.is_clone(&dir) {
            Ok(dir)
        } else {
            Err(format!("{url} is not cloned yet"))
        }
    }

    /// A clone of `url` into `dir`, without a checkout: the objects and the
    /// remote-tracking refs are all that is read.
    fn clone_into(&self, url: &str, dir: &Path, checkout: bool) -> Result<(), Why> {
        // A directory is cleared first: it must be a place of the cache, never
        // the cache itself nor anything outside it.
        let climbs = dir
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir));
        if climbs || dir == self.cache || !dir.starts_with(&self.cache) {
            return Err(format!(
                "refusing to clone into {}: it is not a place of the cache",
                dir.display()
            ));
        }
        if self.fs.exists(dir).unwrap_or(false) {
            self.fs
                .remove_dir_all(dir)
                .map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
        }
        if let Some(parent) = dir.parent() {
            self.fs
                .create_dir_all(parent, os::Mode::from_bits(0o755))
                .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
        }
        let target = dir.to_string_lossy();
        let mut args = vec!["clone"];
        if !checkout {
            args.push("--no-checkout");
        }
        args.extend(["--", url, &target]);
        let outcome = command::run(None, &args, None);
        if outcome.is_err() {
            // A partial clone is not a clone; the next call clones afresh.
            let _ = self.fs.remove_dir_all(dir);
        }
        outcome.map(drop)
    }
}

impl Remotes for GitRemotes {
    fn local_repository(&self, path: &str) -> Result<String, Why> {
        let dir = Path::new(path);
        command::run(Some(dir), &["rev-parse", "--git-dir"], None).map_err(|_| {
            format!("{path:?} holds no git repository: name one, or write its file:// URL")
        })?;
        let absolute = self
            .fs
            .canonicalize(dir)
            .map_err(|e| format!("cannot resolve {path:?}: {e}"))?;
        Ok(format!("file://{}", absolute.display()))
    }

    fn ensure(&self, url: &str) -> Result<(), Why> {
        let dir = self.clone_dir(url)?;
        if self.is_clone(&dir) {
            return Ok(());
        }
        self.clone_into(url, &dir, false)
    }

    fn refresh(&self, url: &str) -> Result<(), Why> {
        let dir = self.clone_dir(url)?;
        if !self.is_clone(&dir) {
            return self.clone_into(url, &dir, false);
        }
        command::run(
            Some(&dir),
            &["fetch", "--prune", "--force", "--tags", "origin"],
            None,
        )?;
        // The default branch may have moved; a repository with no branch yet
        // has none to point at, and that is not a failure.
        let _ = command::run(
            Some(&dir),
            &["remote", "set-head", "origin", "--auto"],
            None,
        );
        Ok(())
    }

    fn tags(&self, url: &str) -> Result<Vec<String>, Why> {
        read::tags(&self.cloned(url)?)
    }

    fn resolve_ref(&self, url: &str, name: &str) -> Result<String, Why> {
        read::resolve_ref(&self.cloned(url)?, name)
    }

    fn resolve_tag(&self, url: &str, name: &str) -> Result<String, Why> {
        read::resolve_tag(&self.cloned(url)?, name)
    }

    fn default_branch(&self, url: &str) -> Result<String, Why> {
        read::default_branch(&self.cloned(url)?)
    }

    fn read_bundle(&self, url: &str, sha: &str, bundle: &str) -> Result<Option<RepoTree>, Why> {
        read::bundle(&self.cloned(url)?, sha, bundle)
    }

    fn list_bundles(&self, url: &str) -> Result<Vec<String>, Why> {
        read::bundles(&self.cloned(url)?)
    }

    fn has_bundle(&self, url: &str, bundle: &str) -> Result<bool, Why> {
        read::has_bundle(&self.cloned(url)?, bundle)
    }

    fn publish(
        &self,
        url: &str,
        files: &BTreeMap<String, Vec<u8>>,
        message: &str,
    ) -> Result<String, Why> {
        publish::publish(self.fs.as_ref(), &self.cache, url, files, message)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
