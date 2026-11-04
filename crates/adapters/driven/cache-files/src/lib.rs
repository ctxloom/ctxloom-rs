//! The `ProjectCache` port over the file-system seam (S12): the files below
//! the project's `.ctxloom`, measured and removed by their path below it.
//! Nothing outside the app dir is reachable: a path that climbs out of it is
//! refused.

mod error;

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::{Failure, ProjectCache};
use file_system::{Fs, Kind};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{CacheError, EN_US};

/// The cache of the project whose app dir is `app_dir`.
pub struct CacheFiles {
    fs: Arc<dyn Fs>,
    app_dir: PathBuf,
}

impl CacheFiles {
    pub fn new(fs: Arc<dyn Fs>, app_dir: &Path) -> CacheFiles {
        CacheFiles {
            fs,
            app_dir: app_dir.to_path_buf(),
        }
    }

    /// `rel` below the app dir; a path that is absolute or climbs out is
    /// refused.
    fn resolve(&self, rel: &str) -> Result<PathBuf, Failure> {
        let inside = Path::new(rel)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if !inside {
            return Err(Box::new(CacheError::Outside {
                rel: rel.to_string(),
            }));
        }
        Ok(self.app_dir.join(rel))
    }

    fn total(&self, path: &Path) -> Result<u64, Failure> {
        let meta = match self.fs.symlink_metadata(path) {
            Ok(meta) => meta,
            // Something else reaped it first: its bytes are already reclaimed.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(io_failure("measure", path, &e)),
        };
        if meta.kind != Kind::Dir {
            return Ok(meta.len);
        }
        let entries = self
            .fs
            .read_dir(path)
            .map_err(|e| io_failure("list", path, &e))?;
        let mut sum = 0;
        for entry in entries {
            sum += self.total(&path.join(&entry.name))?;
        }
        Ok(sum)
    }
}

fn io_failure(op: &str, path: &Path, error: &io::Error) -> Failure {
    Box::new(CacheError::Io {
        op: op.to_string(),
        path: path.display().to_string(),
        ext_err_txt: error.to_string(),
    })
}

impl ProjectCache for CacheFiles {
    fn app_dir(&self) -> String {
        self.app_dir.display().to_string()
    }

    fn measure(&self, rel: &str) -> Result<Option<u64>, Failure> {
        let path = self.resolve(rel)?;
        match self.fs.symlink_metadata(&path) {
            Ok(_) => self.total(&path).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_failure("measure", &path, &e)),
        }
    }

    fn remove(&self, rel: &str) -> Result<(), Failure> {
        let path = self.resolve(rel)?;
        let removed = match self.fs.symlink_metadata(&path) {
            Ok(meta) if meta.kind == Kind::Dir => self.fs.remove_dir_all(&path),
            Ok(_) => self.fs.remove_file(&path),
            Err(e) => Err(e),
        };
        match removed {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_failure("remove", &path, &e)),
        }
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
