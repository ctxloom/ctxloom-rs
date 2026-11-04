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
use angzarr_embedded::StoreError;
use file_system::Fs;
use os::Mode;
use ports::Project;
use session_ports::ProjectStores;

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

    /// `project`'s store, read-only: `None` where it has no database, and
    /// nothing is made.
    pub async fn open_existing(&self, project: &str) -> Result<Option<SqliteStore>, StoreError> {
        let path = self.path(project);
        let real = self.fs.real_path(&path).map_err(|e| {
            StoreError::Unavailable(
                StoreOpenError::Unplaced {
                    path: path.display().to_string(),
                    detail: e.to_string(),
                }
                .cause(),
            )
        })?;
        SqliteStore::open_existing(&real).await
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

/// The SQLite store of the project last asked for, held until another is,
/// behind the [`ProjectStores`] port.
///
/// [`SqliteStores::open`] opens it to write, making its directory and file
/// (migrated if need be); [`SqliteStores::open_existing`] opens it read-only
/// and never creates it, finding no store where there is no file. A store
/// held to write serves a read.
pub struct SqliteStores {
    stores: Stores,
    held: Option<Held>,
}

/// A store held open, and for whom.
struct Held {
    project: String,
    /// Opened to write (created and migrated if need be), not read-only.
    writable: bool,
    store: SqliteStore,
}

impl SqliteStores {
    /// The stores under `home`, on `fs`; none opened yet.
    pub fn new(fs: Arc<dyn Fs>, home: impl Into<PathBuf>) -> SqliteStores {
        SqliteStores {
            stores: Stores::new(fs, home),
            held: None,
        }
    }
}

impl ProjectStores for SqliteStores {
    type Store = SqliteStore;

    /// The project's store, opened to write: its directory and file made and
    /// migrated if need be. Held after.
    async fn open(&mut self, project: &Project) -> Result<&SqliteStore, StoreError> {
        let held = self
            .held
            .take()
            .filter(|h| h.project == project.id && h.writable);
        if let Some(held) = held {
            return Ok(&self.held.insert(held).store);
        }
        let store = self
            .stores
            .open(&project.id)
            .await
            .map_err(|e| StoreError::Unavailable(e.cause()))?;
        Ok(&self
            .held
            .insert(Held {
                project: project.id.clone(),
                writable: true,
                store,
            })
            .store)
    }

    /// The project's store, held or opened read-only; `None` if there is no
    /// file, and nothing is made.
    async fn open_existing(
        &mut self,
        project: &Project,
    ) -> Result<Option<&SqliteStore>, StoreError> {
        if let Some(held) = self.held.take().filter(|h| h.project == project.id) {
            return Ok(Some(&self.held.insert(held).store));
        }
        let Some(store) = self.stores.open_existing(&project.id).await? else {
            return Ok(None);
        };
        Ok(Some(
            &self
                .held
                .insert(Held {
                    project: project.id.clone(),
                    writable: false,
                    store,
                })
                .store,
        ))
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
