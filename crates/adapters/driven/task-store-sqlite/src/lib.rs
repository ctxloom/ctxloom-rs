//! A project's task store: annals-store-sql's SQLite store at the path the
//! project names (`~/.ctxloom/projects/<project-id>/state.db`), opened on
//! first use and held after, behind the [`TaskStores`] port. A write opens
//! it creating what is missing; a read opens it read-only and never
//! creates, migrates or writes, finding no store where there is no file. A
//! file a writer is still making is annals-store-sql's to wait for; one not
//! migrated is refused (`annals-store-sql-unmigrated`) until a write
//! migrates it.
//!
//! The store purges a stream (a prune's), on its own or in a unit of work,
//! through annals-store-sql's purge, so it is a [`ports::PurgingStore`].
//!
//! [`StoreCheck`] checks a project's database whole (SQLite's `PRAGMA
//! integrity_check`, through annals-store-sql), opened afresh and
//! read-only: damage is reported as the store's diagnostic, never
//! repaired, and no database is no damage.

mod error;

use std::path::{Path, PathBuf};

use annals::next::StoreError;
use annals_diagnostic::Report;
use annals_store_sql::sqlite::SqliteStore;
use annals_store_sql::Integrity;
use ports::{Checked, Project, StoreCheck, TaskStores};

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

impl StoreCheck for SqliteStores {
    /// The database at `store`, opened afresh and read-only, never the
    /// store held: a check reads only what was last committed, waits on no
    /// writer, and writes nothing. It runs on a runtime of its own, on a
    /// thread of its own, since the port is synchronous and its caller may
    /// be on a runtime already (the binary's).
    fn check(&mut self, store: &Path) -> Result<Checked, Report> {
        std::thread::scope(|scope| {
            let checking = scope.spawn(|| {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .map_err(|e| {
                        Report::of(&StoreOpenError::NoRuntime {
                            detail: e.to_string(),
                        })
                    })?;
                runtime.block_on(check(store)).map_err(|e| Report::of(&e))
            });
            checking
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    }
}

/// The database at `path`, checked read-only; absent if there is none.
async fn check(path: &Path) -> Result<Checked, StoreError> {
    let Some(mut store) = SqliteStore::open_existing(path).await? else {
        return Ok(Checked::Absent);
    };
    Ok(match store.integrity_check().await? {
        Integrity::Intact => Checked::Intact,
        Integrity::Damaged(damage) => Checked::Damaged(Report::of(&damage)),
    })
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
