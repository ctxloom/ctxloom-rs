//! The `ContentRepo` port over the file-system seam (S1): the project's
//! bundles at `<app dir>/content/bundles/v2/<name>/`, read as files.
//!
//! - A directory holding a `bundle.yaml` is a bundle, however deep its name's
//!   path is (`team/rules`); everything below it is its items, so the walk
//!   stops there. A file is never a bundle.
//! - A tree is read by listing directories, never by globbing: a metadata
//!   sidecar is dot-prefixed (`.postgres.meta.yaml`), and a walk that skipped
//!   dotfiles would drop it from its item.
//! - Nothing is created: reading is a read.

mod error;

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::{ContentRepo, Failure, TreeFiles};
use file_system::{Fs, Kind};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{ContentError, EN_US};

/// Where bundles are below the app dir.
pub const BUNDLES_BELOW_APP_DIR: &str = "content/bundles/v2";

/// The envelope file whose presence makes a directory a bundle.
pub const MANIFEST: &str = "bundle.yaml";

/// The project's bundles, on a filesystem.
pub struct ContentFs {
    fs: Arc<dyn Fs>,
    root: PathBuf,
}

impl ContentFs {
    /// The bundles under `app_dir` (the `.ctxloom` directory), on `fs`.
    pub fn new(fs: Arc<dyn Fs>, app_dir: &Path) -> ContentFs {
        ContentFs {
            fs,
            root: app_dir.join(BUNDLES_BELOW_APP_DIR),
        }
    }

    /// What is at `path`, a link followed; `None` where nothing is.
    fn kind_of(&self, path: &Path) -> io::Result<Option<Kind>> {
        match self.fs.metadata(path) {
            Ok(meta) => Ok(Some(meta.kind)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The names of the bundles at or below `dir` (named `prefix`).
    fn walk(&self, dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), Failure> {
        let entries = self.fs.read_dir(dir).map_err(|e| list_failure(dir, &e))?;
        for entry in entries {
            let name = entry.name.to_string_lossy().into_owned();
            let path = dir.join(&entry.name);
            let kind = match entry.kind {
                Kind::Symlink => self.kind_of(&path).map_err(|e| list_failure(&path, &e))?,
                other => Some(other),
            };
            if kind != Some(Kind::Dir) {
                continue;
            }
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            match self.kind_of(&path.join(MANIFEST)) {
                Ok(Some(Kind::File)) => out.push(rel),
                Ok(_) => self.walk(&path, &rel, out)?,
                Err(e) => return Err(list_failure(&path, &e)),
            }
        }
        Ok(())
    }

    /// The files below `dir` with an execute bit, by their path relative to `top`.
    fn executables(
        &self,
        top: &Path,
        dir: &Path,
        out: &mut BTreeSet<String>,
    ) -> Result<(), Failure> {
        for entry in self.fs.read_dir(dir).map_err(|e| list_failure(dir, &e))? {
            let path = dir.join(&entry.name);
            let meta = self
                .fs
                .metadata(&path)
                .map_err(|e| list_failure(&path, &e))?;
            match meta.kind {
                Kind::Dir => self.executables(top, &path, out)?,
                Kind::File if meta.executable => {
                    let rel = path.strip_prefix(top).unwrap_or(&path);
                    let rel: Vec<String> = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect();
                    out.insert(rel.join("/"));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Every file below `dir`, by its path relative to `top`.
    fn collect(&self, top: &Path, dir: &Path, out: &mut TreeFiles) -> Result<(), Failure> {
        for entry in self.fs.read_dir(dir).map_err(|e| list_failure(dir, &e))? {
            let path = dir.join(&entry.name);
            let kind = match entry.kind {
                Kind::Symlink => self.kind_of(&path).map_err(|e| list_failure(&path, &e))?,
                other => Some(other),
            };
            match kind {
                Some(Kind::Dir) => self.collect(top, &path, out)?,
                Some(Kind::File) => {
                    let bytes = self.fs.read(&path).map_err(|e| {
                        Box::new(ContentError::ReadFile {
                            path: path.display().to_string(),
                            detail: e.to_string(),
                        }) as Failure
                    })?;
                    let rel = path.strip_prefix(top).unwrap_or(&path);
                    let rel: Vec<String> = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect();
                    out.insert(rel.join("/"), bytes);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn list_failure(path: &Path, error: &io::Error) -> Failure {
    Box::new(ContentError::ListDir {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

impl ContentRepo for ContentFs {
    fn has_root(&self) -> Result<bool, Failure> {
        match self.kind_of(&self.root) {
            Ok(kind) => Ok(kind == Some(Kind::Dir)),
            Err(e) => Err(Box::new(ContentError::ListRoot {
                path: self.root.display().to_string(),
                detail: e.to_string(),
            })),
        }
    }

    fn bundle_names(&self) -> Result<Vec<String>, Failure> {
        if !self.has_root()? {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        self.walk(&self.root, "", &mut names)?;
        names.sort();
        Ok(names)
    }

    fn read_tree(&self, name: &str) -> Result<TreeFiles, Failure> {
        let climbs = name.is_empty()
            || Path::new(name).is_absolute()
            || name.split('/').any(|part| part == ".." || part == ".");
        if climbs {
            return Err(Box::new(ContentError::BadName {
                name: name.to_string(),
            }));
        }
        let top = self.root.join(name);
        let mut files = TreeFiles::new();
        self.collect(&top, &top, &mut files)?;
        Ok(files)
    }

    fn manifest_path(&self, name: &str) -> String {
        self.root.join(name).join(MANIFEST).display().to_string()
    }

    fn executable_files(&self, name: &str) -> Result<BTreeSet<String>, Failure> {
        let top = self.root.join(name);
        let mut found = BTreeSet::new();
        self.executables(&top, &top, &mut found)?;
        Ok(found)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
