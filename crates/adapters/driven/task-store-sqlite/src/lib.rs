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
//! It answers a watch annals-store-sql's change token
//! ([`WatchedStores`]), paired with which opening of the store answered
//! it, since tokens compare only from one connection.
//!
//! [`StoreCheck`] checks a project's database whole (SQLite's `PRAGMA
//! integrity_check`, through annals-store-sql), opened afresh and
//! read-only: damage is reported as the store's diagnostic, never
//! repaired, and no database is no damage.

mod error;

use std::path::{Path, PathBuf};

use annals::next::{AsyncWatch, StoreError};
use annals_diagnostic::Report;
use annals_store_sql::sqlite::SqliteStore;
use annals_store_sql::Integrity;
use ports::{Checked, HeldToken, Project, StoreCheck, TaskStores, WatchedStores};

pub use error::{diagnostic_samples, StoreOpenError, EN_US};

/// The SQLite store of the project last asked for, held until another is.
#[derive(Default)]
pub struct SqliteStores {
    held: Option<Held>,
    /// How many stores have been opened: the last one's handle.
    opened: u64,
}

/// A store held open, and for what.
struct Held {
    path: PathBuf,
    /// Opened to write (created and migrated if need be), not read-only.
    writable: bool,
    /// Which opening this is, for its tokens ([`HeldToken::handle`]).
    handle: u64,
    store: SqliteStore,
}

impl SqliteStores {
    /// `store`, just opened, held for `path` under a handle of its own.
    fn hold(&mut self, path: &Path, writable: bool, store: SqliteStore) -> Held {
        self.opened += 1;
        Held {
            path: path.to_path_buf(),
            writable,
            handle: self.opened,
            store,
        }
    }

    /// The store held for `project`, or the one at its path opened
    /// read-only and held; `None` if there is none.
    async fn hold_existing(&mut self, project: &Project) -> Result<Option<&mut Held>, StoreError> {
        let held = self.held.take().filter(|held| held.path == project.store);
        let held = match held {
            Some(held) => held,
            None => match SqliteStore::open_existing(&project.store).await? {
                Some(store) => self.hold(&project.store, false, store),
                None => return Ok(None),
            },
        };
        Ok(Some(self.held.insert(held)))
    }
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
                let store = SqliteStore::open(&project.store).await?;
                self.hold(&project.store, true, store)
            }
        };
        Ok(&mut self.held.insert(held).store)
    }

    async fn open_existing(
        &mut self,
        project: &Project,
    ) -> Result<Option<&mut SqliteStore>, StoreError> {
        Ok(self
            .hold_existing(project)
            .await?
            .map(|held| &mut held.store))
    }
}

impl WatchedStores for SqliteStores {
    /// The held store's `PRAGMA data_version` and `total_changes()`
    /// (annals-store-sql's token), under the handle of its opening. Where
    /// there is no database, finding so is one look at the filesystem, so
    /// a watch of a project not yet written polls for its store cheaply.
    async fn change_token(&mut self, project: &Project) -> Result<Option<HeldToken>, StoreError> {
        let Some(held) = self.hold_existing(project).await? else {
            return Ok(None);
        };
        Ok(Some(HeldToken {
            handle: held.handle,
            change: held.store.change_token().await?,
        }))
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
