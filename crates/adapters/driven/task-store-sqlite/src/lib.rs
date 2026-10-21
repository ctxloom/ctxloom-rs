//! A project's task store: annals-store-sql's SQLite store at the path the
//! project names (`~/.ctxloom/projects/<project-id>/state.db`), opened on
//! first use and held after, behind the [`TaskStores`] port.

mod error;

use std::path::{Path, PathBuf};

use annals::next::StoreError;
use annals_diagnostic::Report;
use annals_store_sql::sqlite::SqliteStore;
use ports::{Project, TaskStores};

pub use error::{diagnostic_samples, StoreOpenError};

/// The SQLite store of the project last asked for, held until another is.
#[derive(Default)]
pub struct SqliteStores {
    held: Option<(PathBuf, SqliteStore)>,
}

impl TaskStores for SqliteStores {
    type Store = SqliteStore;

    async fn open(&mut self, project: &Project) -> Result<&mut SqliteStore, StoreError> {
        let held = self.held.take().filter(|(path, _)| *path == project.store);
        let store = match held {
            Some((_, store)) => store,
            None => {
                create_parent(&project.store).map_err(unavailable)?;
                open_writable(project.store.clone()).await?
            }
        };
        let (_, store) = self.held.insert((project.store.clone(), store));
        Ok(store)
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

/// [`SqliteStore::open`], run to its end on a blocking thread.
///
/// The compiler cannot prove its future `Send`: sqlx's migrator
/// is generic over `Acquire<'a>`, and an `async fn` holding it fails the
/// higher-ranked check ("implementation of `Acquire` is not general
/// enough"). The port's futures must be `Send`, so the open is driven by
/// `block_on` on a blocking thread, which needs only the path to be `Send`.
/// Once annals-store-sql's `open` is provably `Send`, this is a plain await.
#[expect(
    clippy::result_large_err,
    reason = "the blocking closure passes SqliteStore::open's own error through"
)]
async fn open_writable(path: PathBuf) -> Result<SqliteStore, StoreError> {
    let shown = path.display().to_string();
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || runtime.block_on(SqliteStore::open(&path)))
        .await
        .map_err(|e| {
            unavailable(StoreOpenError::Interrupted {
                path: shown,
                detail: e.to_string(),
            })
        })?
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
