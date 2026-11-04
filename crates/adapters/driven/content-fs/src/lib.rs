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

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::{Change, ContentRepo, Failure, Put, TreeFiles, UserFiles};
use file_system::{Fs, Kind, Open, Perms};
use os::Mode;

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

    fn apply(&self, name: &str, changes: &[Change]) -> Result<(), Failure> {
        let top = self.bundle_path(name)?;
        for change in changes {
            match change {
                Change::Write(put) => self.check_path(&top, &put.path, false)?,
                Change::Remove { path } => self.check_path(&top, path, false)?,
                Change::RemoveDir { path } => self.check_path(&top, path, true)?,
            }
        }
        for change in changes {
            match change {
                Change::Write(put) => self.write_put(&top, put)?,
                Change::Remove { path } => {
                    let target = join_rel(&top, path);
                    ignore_missing(self.fs.remove_file(&target), &target)?;
                }
                Change::RemoveDir { path } => {
                    let target = join_rel(&top, path);
                    ignore_missing(self.fs.remove_dir_all(&target), &target)?;
                }
            }
        }
        Ok(())
    }

    fn claim(&self, name: &str) -> Result<bool, Failure> {
        let top = self.bundle_path(name)?;
        let exists = self.fs.exists(&top).map_err(|e| list_failure(&top, &e))?;
        if exists {
            return Ok(false);
        }
        make_dirs(&*self.fs, &top)?;
        // The claim is one exclusive creation: of the manifest, which the
        // bundle then is. Two creators racing here have one winner.
        let manifest = top.join(MANIFEST);
        match self
            .fs
            .open(&manifest, Open::CreateNew(Mode::from_bits(0o644)))
        {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(write_failure(&manifest, &e)),
        }
    }

    fn remove_bundle(&self, name: &str) -> Result<(), Failure> {
        let top = self.bundle_path(name)?;
        ignore_missing(self.fs.remove_dir_all(&top), &top)
    }

    fn bundle_dir(&self, name: &str) -> String {
        self.root.join(name).display().to_string()
    }
}

/// `rel` (slash separated) below `top`.
fn join_rel(top: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(top.to_path_buf(), |path, part| path.join(part))
}

fn write_failure(path: &Path, error: &io::Error) -> Failure {
    Box::new(ContentError::WriteFile {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn make_dirs(fs: &dyn Fs, dir: &Path) -> Result<(), Failure> {
    fs.create_dir_all(dir, Mode::from_bits(0o755)).map_err(|e| {
        Box::new(ContentError::MakeDir {
            path: dir.display().to_string(),
            detail: e.to_string(),
        }) as Failure
    })
}

/// A removal that finds nothing to remove is done.
fn ignore_missing(result: io::Result<()>, path: &Path) -> Result<(), Failure> {
    match result {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(Box::new(ContentError::RemovePath {
            path: path.display().to_string(),
            detail: e.to_string(),
        })),
        _ => Ok(()),
    }
}

impl ContentFs {
    /// The bundle's directory, for a name that does not climb out of the root.
    fn bundle_path(&self, name: &str) -> Result<PathBuf, Failure> {
        if !ctxloom_ports::is_plain_relative(name) {
            return Err(Box::new(ContentError::BadName {
                name: name.to_string(),
            }));
        }
        Ok(join_rel(&self.root, name))
    }

    /// A path inside the bundle: relative, no empty, `.` or `..` segment, no
    /// NUL, and no link on the way (the final part counts when `whole`, a
    /// directory about to be removed).
    fn check_path(&self, top: &Path, rel: &str, whole: bool) -> Result<(), Failure> {
        let bad = || -> Failure {
            Box::new(ContentError::BadPath {
                path: rel.to_string(),
            })
        };
        if !ctxloom_ports::is_plain_relative(rel) || rel.contains('\0') {
            return Err(bad());
        }
        let parts: Vec<&str> = rel.split('/').collect();
        let last = if whole { parts.len() } else { parts.len() - 1 };
        let mut here = top.to_path_buf();
        for part in &parts[..last] {
            here.push(part);
            match self.fs.symlink_metadata(&here) {
                Ok(meta) if meta.kind == Kind::Symlink => return Err(bad()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => break,
                Err(e) => return Err(list_failure(&here, &e)),
                Ok(_) => {}
            }
        }
        Ok(())
    }

    fn write_put(&self, top: &Path, put: &Put) -> Result<(), Failure> {
        let target = join_rel(top, &put.path);
        if let Some(parent) = target.parent() {
            make_dirs(&*self.fs, parent)?;
        }
        let mode = Mode::from_bits(if put.executable { 0o755 } else { 0o644 });
        self.fs
            .write_atomic(&target, &put.bytes, Perms::Exact(mode))
            .map_err(|e| write_failure(&target, &e))
    }
}

/// The user's own files, on a filesystem.
pub struct UserFilesFs {
    fs: Arc<dyn Fs>,
}

impl UserFilesFs {
    pub fn new(fs: Arc<dyn Fs>) -> UserFilesFs {
        UserFilesFs { fs }
    }
}

impl UserFilesFs {
    fn walk(&self, top: &Path, dir: &Path, out: &mut BTreeMap<String, Put>) -> Result<(), Failure> {
        for entry in self.fs.read_dir(dir).map_err(|e| list_failure(dir, &e))? {
            let path = dir.join(&entry.name);
            match entry.kind {
                Kind::Dir => self.walk(top, &path, out)?,
                Kind::File => {
                    let bytes = self.fs.read(&path).map_err(|e| read_failure(&path, &e))?;
                    let meta = self
                        .fs
                        .metadata(&path)
                        .map_err(|e| list_failure(&path, &e))?;
                    let rel = relative(top, &path);
                    out.insert(
                        rel.clone(),
                        Put {
                            path: rel,
                            bytes,
                            executable: meta.executable,
                        },
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// `path` below `top`, slash separated.
fn relative(top: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(top).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join("/")
}

fn read_failure(path: &Path, error: &io::Error) -> Failure {
    Box::new(ContentError::ReadFile {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

impl UserFiles for UserFilesFs {
    fn exists(&self, path: &str) -> Result<bool, Failure> {
        self.fs
            .exists(Path::new(path))
            .map_err(|e| list_failure(Path::new(path), &e))
    }

    fn is_dir(&self, path: &str) -> Result<bool, Failure> {
        match self.fs.metadata(Path::new(path)) {
            Ok(meta) => Ok(meta.kind == Kind::Dir),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(list_failure(Path::new(path), &e)),
        }
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, Failure> {
        self.fs
            .read(Path::new(path))
            .map_err(|e| read_failure(Path::new(path), &e))
    }

    fn read_tree(&self, dir: &str) -> Result<Vec<Put>, Failure> {
        let top = Path::new(dir);
        let mut found = BTreeMap::new();
        self.walk(top, top, &mut found)?;
        Ok(found.into_values().collect())
    }

    fn write_file(&self, path: &str, bytes: &[u8], overwrite: bool) -> Result<bool, Failure> {
        let target = Path::new(path);
        if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
            make_dirs(&*self.fs, parent)?;
        }
        if overwrite {
            self.fs
                .write_atomic(target, bytes, Perms::Exact(Mode::from_bits(0o644)))
                .map_err(|e| write_failure(target, &e))?;
            return Ok(true);
        }
        match self
            .fs
            .open(target, Open::CreateNew(Mode::from_bits(0o644)))
        {
            Ok(mut file) => {
                use std::io::Write;
                file.write_all(bytes)
                    .map_err(|e| write_failure(target, &e))?;
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(write_failure(target, &e)),
        }
    }

    fn write_tree(&self, dir: &str, files: &[Put]) -> Result<(), Failure> {
        let top = Path::new(dir);
        for put in files {
            let target = join_rel(top, &put.path);
            if let Some(parent) = target.parent() {
                make_dirs(&*self.fs, parent)?;
            }
            let mode = Mode::from_bits(if put.executable { 0o755 } else { 0o644 });
            self.fs
                .write_atomic(&target, &put.bytes, Perms::Exact(mode))
                .map_err(|e| write_failure(&target, &e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
