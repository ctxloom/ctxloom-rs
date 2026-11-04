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

use ctxloom_ports::{ConfigFiles, Failure, LayerText, Source};
use file_system::{Fs, Kind};

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

    /// The file's text, or none where it is not there.
    fn layer(&self, path: &Path) -> Result<Option<LayerText>, Failure> {
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

    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        let home = self.home.join(APP_DIR_NAME).join(CONFIG_FILE_NAME);
        let mut paths = vec![home.clone()];
        let project = self.app_dir.path.join(CONFIG_FILE_NAME);
        if self.app_dir.source == Source::Project && project != home {
            paths.push(project);
        }
        let mut layers = Vec::new();
        for path in paths {
            layers.extend(self.layer(&path)?);
        }
        Ok(layers)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
