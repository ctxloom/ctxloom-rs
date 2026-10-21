//! Opening a project's store over a real filesystem, on a tokio runtime as
//! the binary runs it.

use std::future::Future;
use std::path::{Path, PathBuf};

use annals::next::v1::Cover;
use annals::next::AsyncEventStore;
use annals_diagnostic::Diagnostic;
use ports::{Project, TaskStores};

use super::SqliteStores;

fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("a runtime")
        .block_on(future)
}

fn project(id: &str, store: PathBuf) -> Project {
    Project {
        id: id.to_string(),
        root: "/work/tree".to_string(),
        store,
    }
}

fn some_stream() -> Cover {
    Cover {
        domain: "tasks".to_string(),
        root: None,
        edition: None,
    }
}

/// Whether `store` answers a read: it is a working, migrated store.
async fn answers(stores: &mut SqliteStores, project: &Project) -> bool {
    let store = stores.open(project).await.expect("opens");
    store.read_from(&some_stream(), 0).await.is_ok()
}

fn store_in(dir: &Path, id: &str) -> PathBuf {
    dir.join(".ctxloom/projects").join(id).join("state.db")
}

#[test]
fn test_opening_creates_the_directory_and_a_migrated_database() {
    let home = tempfile::tempdir().unwrap();
    let path = store_in(home.path(), "jolly-mute-magma");
    let mut stores = SqliteStores::default();
    assert!(block_on(answers(
        &mut stores,
        &project("jolly-mute-magma", path.clone())
    )));
    assert!(path.exists(), "the database is on disk");
}

#[test]
fn test_the_store_is_opened_once_and_held_for_its_project() {
    let home = tempfile::tempdir().unwrap();
    let path = store_in(home.path(), "jolly-mute-magma");
    let project = project("jolly-mute-magma", path.clone());
    let mut stores = SqliteStores::default();
    block_on(async {
        assert!(answers(&mut stores, &project).await);
        std::fs::remove_file(&path).unwrap();
        assert!(answers(&mut stores, &project).await);
    });
    assert!(!path.exists(), "the held store, not a new file");
}

#[test]
fn test_another_project_opens_its_own_store() {
    let home = tempfile::tempdir().unwrap();
    let (first, second) = (
        store_in(home.path(), "jolly-mute-magma"),
        store_in(home.path(), "high-tight-gulf"),
    );
    let mut stores = SqliteStores::default();
    block_on(async {
        assert!(answers(&mut stores, &project("jolly-mute-magma", first.clone())).await);
        assert!(answers(&mut stores, &project("high-tight-gulf", second.clone())).await);
    });
    assert!(first.exists() && second.exists());
}

#[test]
fn test_a_directory_that_cannot_be_made_fails_naming_it() {
    let home = tempfile::tempdir().unwrap();
    let blocker = home.path().join("projects");
    std::fs::write(&blocker, "a file where the directory goes").unwrap();
    let path = blocker.join("jolly-mute-magma").join("state.db");
    let mut stores = SqliteStores::default();
    let error = block_on(stores.open(&project("jolly-mute-magma", path)))
        .map(drop)
        .unwrap_err();
    let annals::next::StoreError::Unavailable(report) = &error else {
        panic!("unavailable: {error:?}");
    };
    assert_eq!(report.code(), "task-store-sqlite-create-dir");
    assert!(
        report.to_string().contains(&blocker.display().to_string()),
        "{report}"
    );
}

#[test]
fn test_a_database_that_does_not_open_fails_with_the_stores_error() {
    let home = tempfile::tempdir().unwrap();
    // A directory where the database file goes.
    let path = home.path().join("state.db");
    std::fs::create_dir(&path).unwrap();
    let mut stores = SqliteStores::default();
    let error = block_on(stores.open(&project("jolly-mute-magma", path)))
        .map(drop)
        .unwrap_err();
    let annals::next::StoreError::Unavailable(report) = &error else {
        panic!("unavailable: {error:?}");
    };
    assert_eq!(report.code(), "annals-store-sql-unavailable");
}
