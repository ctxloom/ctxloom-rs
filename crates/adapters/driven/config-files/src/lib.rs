//! The `ConfigFiles` port over the file-system seam (S1): where the project's
//! `.ctxloom` is, and its layered `config.yaml` files (the home's, then the
//! project's).
//!
//! Finding the app dir (Go's `configload/appdir.go`):
//!
//! 1. `CTXLOOM_ROOT`, when it names a directory that exists, is the project's
//!    root, and `<root>/.ctxloom` its app dir whether it exists yet or not.
//!    One that does not is ignored (and said by the caller).
//! 2. Otherwise the nearest `.ctxloom` directory up from the working
//!    directory, stopping at the temporary directory: that is shared scratch
//!    space, never a project.
//! 3. Otherwise the home's `~/.ctxloom` (no project).
//!
//! Finding is a read: nothing is created.

mod error;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::{ConfigFiles, Failure, LayerText, Put, Scaffold, Scope, Source};
use file_system::{Fs, Kind, Perms};
use os::Mode;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{ConfigFileError, EN_US};

/// The name of the app directory.
pub const APP_DIR_NAME: &str = ".ctxloom";

/// The name of the config file in it.
pub const CONFIG_FILE_NAME: &str = "config.yaml";

/// What finding the app dir needs of the process.
#[derive(Clone, Debug)]
pub struct Surroundings {
    /// `CTXLOOM_ROOT`, as given (empty counts as unset).
    pub root_env: Option<String>,
    pub cwd: PathBuf,
    pub home: PathBuf,
    /// The OS temporary directory.
    pub temp_root: PathBuf,
}

/// An app dir found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AppDir {
    pub path: PathBuf,
    pub source: Source,
    /// `CTXLOOM_ROOT` named a directory that is not there.
    pub ignored_root: Option<String>,
}

/// Find the app dir.
pub fn find_app_dir(fs: &dyn Fs, around: &Surroundings) -> AppDir {
    let is_dir = |path: &Path| fs.metadata(path).is_ok_and(|m| m.kind == Kind::Dir);
    let mut ignored_root = None;
    if let Some(root) = around.root_env.as_deref().filter(|r| !r.is_empty()) {
        if is_dir(Path::new(root)) {
            return AppDir {
                path: Path::new(root).join(APP_DIR_NAME),
                source: Source::Project,
                ignored_root: None,
            };
        }
        ignored_root = Some(root.to_string());
    }
    for dir in around.cwd.ancestors() {
        if dir == around.temp_root {
            break;
        }
        let candidate = dir.join(APP_DIR_NAME);
        if is_dir(&candidate) {
            return AppDir {
                path: candidate,
                source: Source::Project,
                ignored_root,
            };
        }
    }
    AppDir {
        path: around.home.join(APP_DIR_NAME),
        source: Source::Home,
        ignored_root,
    }
}

/// The layered config files of one app dir.
pub struct ConfigFilesFs {
    fs: Arc<dyn Fs>,
    app_dir: AppDir,
    home: PathBuf,
}

impl ConfigFilesFs {
    /// The config files of `app_dir`, with the home's under them.
    pub fn new(fs: Arc<dyn Fs>, app_dir: AppDir, home: &Path) -> ConfigFilesFs {
        ConfigFilesFs {
            fs,
            app_dir,
            home: home.to_path_buf(),
        }
    }

    pub fn app_dir_found(&self) -> &AppDir {
        &self.app_dir
    }

    /// `scope`'s config file: the home's, or the project's (the home's again
    /// with no project).
    fn layer_file(&self, scope: Scope) -> PathBuf {
        match scope {
            Scope::Home => self.home.join(APP_DIR_NAME).join(CONFIG_FILE_NAME),
            Scope::Project => self.app_dir.path.join(CONFIG_FILE_NAME),
        }
    }

    /// The file's text, or none where it is not there.
    fn read_layer(&self, path: &Path) -> Result<Option<LayerText>, Failure> {
        let shown = path.display().to_string();
        match self.fs.read(path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => Ok(Some(LayerText { path: shown, text })),
                Err(_) => Err(Box::new(ConfigFileError::NotText { path: shown })),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Box::new(ConfigFileError::Read {
                path: shown,
                detail: e.to_string(),
            })),
        }
    }
}

impl ConfigFiles for ConfigFilesFs {
    fn source(&self) -> Source {
        self.app_dir.source
    }

    fn app_dir(&self) -> String {
        self.app_dir.path.display().to_string()
    }

    fn ignored_root(&self) -> Option<String> {
        self.app_dir.ignored_root.clone()
    }

    fn layer(&self, scope: Scope) -> Result<Option<LayerText>, Failure> {
        self.read_layer(&self.layer_file(scope))
    }

    fn layer_path(&self, scope: Scope) -> String {
        self.layer_file(scope).display().to_string()
    }

    fn write_layer(&self, scope: Scope, text: &str) -> Result<(), Failure> {
        let path = self.layer_file(scope);
        write_file(
            &*self.fs,
            &path,
            text.as_bytes(),
            Perms::KeepOr(Mode::from_bits(0o644)),
        )
    }

    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        let home = self.home.join(APP_DIR_NAME).join(CONFIG_FILE_NAME);
        let mut paths = vec![home.clone()];
        let project = self.app_dir.path.join(CONFIG_FILE_NAME);
        if self.app_dir.source == Source::Project && project != home {
            paths.push(project);
        }
        let mut layers = Vec::new();
        for path in paths {
            layers.extend(self.read_layer(&path)?);
        }
        Ok(layers)
    }
}

/// The `.ctxloom` of a new project: under the working directory.
pub struct ScaffoldFs {
    fs: Arc<dyn Fs>,
    app_dir: PathBuf,
}

impl ScaffoldFs {
    /// The scaffold of the project at `cwd`, on `fs`.
    pub fn new(fs: Arc<dyn Fs>, cwd: &Path) -> ScaffoldFs {
        ScaffoldFs {
            fs,
            app_dir: cwd.join(APP_DIR_NAME),
        }
    }
}

/// A file written whole, atomically, its directory made.
fn write_file(fs: &dyn Fs, path: &Path, bytes: &[u8], perms: Perms) -> Result<(), Failure> {
    let failure = |e: &io::Error| -> Failure {
        Box::new(ConfigFileError::Write {
            path: path.display().to_string(),
            detail: e.to_string(),
        })
    };
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent, Mode::from_bits(0o755))
            .map_err(|e| failure(&e))?;
    }
    fs.write_atomic(path, bytes, perms).map_err(|e| failure(&e))
}

impl ScaffoldFs {
    /// `rel` below the app dir, or why it is not a path below it.
    fn below(&self, rel: &str) -> Result<PathBuf, Failure> {
        if !ctxloom_ports::is_plain_relative(rel) || rel.contains('\0') {
            return Err(Box::new(ConfigFileError::Write {
                path: rel.to_string(),
                detail: "not a path below the project's .ctxloom".to_string(),
            }));
        }
        Ok(rel
            .split('/')
            .fold(self.app_dir.clone(), |p, part| p.join(part)))
    }
}

impl Scaffold for ScaffoldFs {
    fn app_dir(&self) -> String {
        self.app_dir.display().to_string()
    }

    fn exists(&self, rel: &str) -> Result<bool, Failure> {
        let path = self.below(rel)?;
        self.fs.exists(&path).map_err(|e| {
            Box::new(ConfigFileError::Read {
                path: path.display().to_string(),
                detail: e.to_string(),
            }) as Failure
        })
    }

    fn write(&self, files: &[Put]) -> Result<(), Failure> {
        let targets: Vec<PathBuf> = files
            .iter()
            .map(|put| self.below(&put.path))
            .collect::<Result<_, _>>()?;
        for (put, path) in files.iter().zip(&targets) {
            let mode = Mode::from_bits(if put.executable { 0o755 } else { 0o644 });
            write_file(&*self.fs, path, &put.bytes, Perms::Exact(mode))?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
