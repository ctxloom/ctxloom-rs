//! A project's ctxloom store (D26): angzarr-store-sqlite's [`SqliteStore`]
//! at `<home>/.ctxloom/projects/<project-id>/ctxloom.db`, beside taskloom's
//! `state.db` and never the same file. All file I/O goes through the
//! `file-system` seam, so a test under a temporary home touches nothing
//! else.
//!
//! - [`Stores::open`] opens the project's store to write, making its
//!   directory and the file (migrated if need be).
//! - The opening is `sqlite-store-opener`'s: this crate is the file name, the
//!   placement under the home and the [`ProjectStores`] port. A store from
//!   before the embedded store is refused ([`OpenError::PredatesEmbedded`]).
//! - Errors are the opener's and the project id's, each under its own code.

mod error;

use std::path::PathBuf;
use std::sync::Arc;

use angzarr_embedded::angzarr_store_sqlite::SqliteStore;
use angzarr_embedded::StoreError;
use file_system::Fs;
use ports::Project;
use session_ports::ProjectStores;
use sqlite_store_opener::Opener;

pub use error::StoreOpenError;
#[cfg(any(test, feature = "test-support"))]
pub use sqlite_store_opener::diagnostic_samples;
pub use sqlite_store_opener::{OpenError, EN_US};

/// The app's directory under a home.
const APP_DIR: &str = ".ctxloom";

/// The store's file name: beside taskloom's `state.db`, never the same file.
const FILE_NAME: &str = "ctxloom.db";

/// The ctxloom stores under one home.
pub struct Stores {
    opener: Opener,
    home: PathBuf,
}

impl Stores {
    /// The stores under `home`, on `fs`; none opened yet.
    pub fn new(fs: Arc<dyn Fs>, home: impl Into<PathBuf>) -> Stores {
        Stores {
            opener: Opener::new(fs, FILE_NAME),
            home: home.into(),
        }
    }

    /// Where `project`'s database is: `<home>/.ctxloom/projects/<project>/ctxloom.db`.
    ///
    /// Refused where `project` is not one plain name (empty, absolute, with a
    /// path separator or NUL, or `.` or `..`): the shared `project-dir`
    /// crate's check, so an id cannot place the database outside the
    /// projects directory.
    pub fn path(&self, project: &str) -> Result<PathBuf, StoreOpenError> {
        project_dir::project_dir(&self.home, APP_DIR, project)
            .map(|dir| self.opener.file_in(&dir))
            .map_err(StoreOpenError::UnsafeProject)
    }

    /// `project`'s store, read-only: `None` where it has no database, and
    /// nothing is made. A store from before the embedded store is refused.
    pub async fn open_existing(&self, project: &str) -> Result<Option<SqliteStore>, StoreError> {
        let path = self
            .path(project)
            .map_err(StoreOpenError::into_store_error)?;
        self.opener
            .open_existing(&path)
            .await
            .map_err(OpenError::into_store_error)
    }

    /// `project`'s store, opened to write: its directory and file made and
    /// migrated if need be. A store from before the embedded store is
    /// refused.
    pub async fn open(&self, project: &str) -> Result<SqliteStore, StoreOpenError> {
        let path = self.path(project)?;
        self.opener
            .open_for_write(&path)
            .await
            .map_err(StoreOpenError::Open)
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
            .map_err(StoreOpenError::into_store_error)?;
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
