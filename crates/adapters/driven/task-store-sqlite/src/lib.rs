//! A project's task store: annals-store-sql's SQLite store at the path the
//! project names (`~/.ctxloom/projects/<project-id>/state.db`), opened on
//! first use and held after, behind the [`TaskStores`] port. A write opens
//! it creating what is missing; a read opens it read-only and never
//! creates, migrates or writes, finding no store where there is no file. A
//! file a writer is still making is annals-store-sql's to wait for; one not
//! migrated is refused (`annals-store-sql-unmigrated`) until a write
//! migrates it.

mod error;

use std::path::{Path, PathBuf};

use annals::next::StoreError;
use annals_diagnostic::Report;
use annals_store_sql::sqlite::SqliteStore;
use ports::{Project, TaskStores};

pub use error::{diagnostic_samples, StoreOpenError, EN_US};

/// The SQLite store of the project last asked for, held until another is.
#[derive(Default)]
pub struct SqliteStores {
    held: Option<Held>,
}

/// A store held open, and for what.
struct Held {
    path: PathBuf,
    /// Opened to write (created and migrated if need be), not read-only.
    writable: bool,
    store: SqliteStore,
}

impl TaskStores for SqliteStores {
    type Store = SqliteStore;

    async fn open(&mut self, project: &Project) -> Result<&mut SqliteStore, StoreError> {
        let held = self
            .held
            .take()
            .filter(|held| held.path == project.store && held.writable);
        let held = match held {
            Some(held) => held,
            None => {
                create_parent(&project.store).map_err(unavailable)?;
                Held {
                    path: project.store.clone(),
                    writable: true,
                    store: SqliteStore::open(&project.store).await?,
                }
            }
        };
        Ok(&mut self.held.insert(held).store)
    }

    async fn open_existing(
        &mut self,
        project: &Project,
    ) -> Result<Option<&mut SqliteStore>, StoreError> {
        let held = self.held.take().filter(|held| held.path == project.store);
        let held = match held {
            Some(held) => held,
            None => match SqliteStore::open_existing(&project.store).await? {
                Some(store) => Held {
                    path: project.store.clone(),
                    writable: false,
                    store,
                },
                None => return Ok(None),
            },
        };
        Ok(Some(&mut self.held.insert(held).store))
    }
}

/// Make the directory the database at `path` goes in.
fn create_parent(path: &Path) -> Result<(), StoreOpenError> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(dir).map_err(|e| StoreOpenError::CreateDir {
        path: dir.display().to_string(),
        detail: e.to_string(),
    })
}

fn unavailable(error: StoreOpenError) -> StoreError {
    StoreError::Unavailable(Report::of(&error))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
