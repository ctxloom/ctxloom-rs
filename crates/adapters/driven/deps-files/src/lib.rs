//! The `DepsFiles` port over the file-system seam.
//!
//! - `<app dir>/remotes.yaml` and `<app dir>/lock.yaml`, written atomically.
//! - The trees installed from remotes, derived and rebuildable:
//!   `<app dir>/cache/bundles/<host and path>/<bundle>/<commit>/`, each marked
//!   with the identity it is the tree of, so the cache can be pruned by what
//!   the lock names. Installing replaces whatever was installed for the
//!   identity, in one rename.
//! - The bundles the project authors, at `<app dir>/content/bundles/v2/`.

mod installed;
mod local;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use deps_ports::{BundleSlot, CopyOutcome, DepsFiles, RepoTree, Why};
use file_system::{Fs, Kind, Perms};
use os::Mode;

pub use installed::IDENTITY_MARKER;

/// The directories and files this feature keeps in a project's app dir.
pub struct ProjectFiles {
    fs: Arc<dyn Fs>,
    app_dir: PathBuf,
}

impl ProjectFiles {
    /// The files of the project (or home) whose `.ctxloom` is `app_dir`.
    pub fn new(fs: Arc<dyn Fs>, app_dir: PathBuf) -> ProjectFiles {
        ProjectFiles { fs, app_dir }
    }

    fn read_text(&self, path: &Path) -> Result<Option<String>, Why> {
        match self.fs.read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn write_text(&self, path: &Path, text: &str) -> Result<(), Why> {
        self.fs
            .create_dir_all(&self.app_dir, Mode::from_bits(0o755))
            .map_err(|e| e.to_string())?;
        self.fs
            .write_atomic(path, text.as_bytes(), Perms::Exact(Mode::from_bits(0o644)))
            .map_err(|e| e.to_string())
    }

    fn registry_file(&self) -> PathBuf {
        self.app_dir.join("remotes.yaml")
    }

    fn lock_file(&self) -> PathBuf {
        self.app_dir.join("lock.yaml")
    }

    /// The kind of what is at `path`, a link followed; none where nothing is.
    pub(crate) fn kind_of(&self, path: &Path) -> Option<Kind> {
        self.fs.metadata(path).ok().map(|meta| meta.kind)
    }
}

impl DepsFiles for ProjectFiles {
    fn registry_path(&self) -> String {
        self.registry_file().display().to_string()
    }

    fn read_registry(&self) -> Result<Option<String>, Why> {
        self.read_text(&self.registry_file())
    }

    fn write_registry(&self, text: &str) -> Result<(), Why> {
        self.write_text(&self.registry_file(), text)
    }

    fn lock_path(&self) -> String {
        self.lock_file().display().to_string()
    }

    fn read_lock(&self) -> Result<Option<String>, Why> {
        self.read_text(&self.lock_file())
    }

    fn write_lock(&self, text: &str) -> Result<(), Why> {
        self.write_text(&self.lock_file(), text)
    }

    fn is_installed(&self, slot: &BundleSlot, sha: &str) -> bool {
        installed::is_installed(self, slot, sha)
    }

    fn install(&self, slot: &BundleSlot, sha: &str, tree: &RepoTree) -> Result<(), Why> {
        installed::install(self, slot, sha, tree)
    }

    fn installed_path(&self, slot: &BundleSlot, sha: &str) -> String {
        installed::path(self, slot, sha)
    }

    fn read_installed(&self, slot: &BundleSlot, sha: &str) -> Result<Option<RepoTree>, Why> {
        installed::read(self, slot, sha)
    }

    fn prune(&self, keep: &[(String, String)]) -> Result<Vec<String>, Why> {
        installed::prune(self, keep)
    }

    fn local_bundle(&self, name: &str) -> Result<Option<String>, Why> {
        local::manifest(self, name)
    }

    fn read_local_bundle(
        &self,
        name: &str,
    ) -> Result<std::collections::BTreeMap<String, Vec<u8>>, Why> {
        local::read(self, name)
    }

    fn remove_local_bundle(&self, name: &str) -> Result<(), Why> {
        local::remove(self, name)
    }

    fn is_directory(&self, path: &str) -> bool {
        self.kind_of(Path::new(path)) == Some(Kind::Dir)
    }

    fn exists(&self, path: &str) -> bool {
        self.fs.exists(Path::new(path)).unwrap_or(false)
    }

    fn copy_bundle(
        &self,
        dir: &str,
        name: &str,
        files: &std::collections::BTreeMap<String, Vec<u8>>,
        force: bool,
    ) -> Result<CopyOutcome, Why> {
        local::copy(self, dir, name, files, force)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
