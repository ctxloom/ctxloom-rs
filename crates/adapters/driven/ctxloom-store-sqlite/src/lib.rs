//! A project's ctxloom store (D26): angzarr-store-sqlite's [`SqliteStore`]
//! at `<home>/.ctxloom/projects/<project-id>/ctxloom.db`, beside taskloom's
//! `state.db` and never the same file. All file I/O goes through the
//! `file-system` seam, so a test under a temporary home touches nothing
//! else.
//!
//! - [`Stores::open`] opens the project's store to write, making its
//!   directory and the file (migrated if need be).
//! - Errors are this crate's, with the store's own text as `ext_err_txt`.

mod error;

use std::path::PathBuf;
use std::sync::Arc;

use angzarr_embedded::angzarr_store_sqlite::SqliteStore;
use file_system::Fs;
use os::Mode;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{StoreOpenError, EN_US};

/// The ctxloom stores under one home.
pub struct Stores {
    fs: Arc<dyn Fs>,
    home: PathBuf,
}

impl Stores {
    /// The stores under `home`, on `fs`; none opened yet.
    pub fn new(fs: Arc<dyn Fs>, home: impl Into<PathBuf>) -> Stores {
        Stores {
            fs,
            home: home.into(),
        }
    }

    /// Where `project`'s database is: `<home>/.ctxloom/projects/<project>/ctxloom.db`.
    pub fn path(&self, project: &str) -> PathBuf {
        self.home
            .join(".ctxloom")
            .join("projects")
            .join(project)
            .join("ctxloom.db")
    }

    /// `project`'s store, opened to write: its directory and file made and
    /// migrated if need be.
    pub async fn open(&self, project: &str) -> Result<SqliteStore, StoreOpenError> {
        let path = self.path(project);
        let shown = path.display().to_string();
        if let Some(dir) = path.parent() {
            // As std's create_dir_all makes them: 0777 before the umask.
            self.fs
                .create_dir_all(dir, Mode::from_bits(0o777))
                .map_err(|e| StoreOpenError::CreateDir {
                    path: dir.display().to_string(),
                    detail: e.to_string(),
                })?;
        }
        let real = self
            .fs
            .real_path(&path)
            .map_err(|e| StoreOpenError::Unplaced {
                path: shown.clone(),
                detail: e.to_string(),
            })?;
        SqliteStore::open(&real)
            .await
            .map_err(|e| StoreOpenError::Open {
                path: shown,
                detail: e.to_string(),
            })
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
