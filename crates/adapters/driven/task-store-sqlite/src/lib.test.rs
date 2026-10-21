//! Opening a project's store over a real filesystem, on a tokio runtime as
//! the binary runs it.

use std::future::Future;
use std::path::{Path, PathBuf};

use annals::next::v1::Cover;
use annals::next::AsyncEventStore;
use annals_diagnostic::Diagnostic;
use ports::{Project, TaskStores};

use annals_store_sql::sqlite::SqliteStore;

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

// ------------------------------------------------------------ reads

#[test]
fn test_reading_a_project_with_no_database_finds_none_and_creates_nothing() {
    let home = tempfile::tempdir().unwrap();
    let path = store_in(home.path(), "tpyo");
    let mut stores = SqliteStores::default();
    let found = block_on(stores.open_existing(&project("tpyo", path))).unwrap();
    assert!(found.is_none());
    assert!(
        !home.path().join(".ctxloom").exists(),
        "no directory, no file"
    );
    assert!(stores.held.is_none());
}

#[test]
fn test_reading_an_existing_database_opens_it_read_only() {
    let home = tempfile::tempdir().unwrap();
    let path = store_in(home.path(), "jolly-mute-magma");
    let project = project("jolly-mute-magma", path);
    block_on(async {
        SqliteStores::default().open(&project).await.unwrap();
        let mut stores = SqliteStores::default();
        let store = stores
            .open_existing(&project)
            .await
            .unwrap()
            .expect("found");
        assert!(store.read_from(&some_stream(), 0).await.is_ok());
        assert!(!stores.held.as_ref().unwrap().writable);
    });
}

#[test]
fn test_a_store_held_to_write_serves_a_read() {
    let home = tempfile::tempdir().unwrap();
    let path = store_in(home.path(), "jolly-mute-magma");
    let project = project("jolly-mute-magma", path.clone());
    let mut stores = SqliteStores::default();
    block_on(async {
        stores.open(&project).await.unwrap();
        std::fs::remove_file(&path).unwrap();
        let found = stores.open_existing(&project).await.unwrap();
        assert!(found.is_some(), "the held store, though the file is gone");
    });
    assert!(stores.held.as_ref().unwrap().writable);
}

#[test]
fn test_a_store_held_to_read_is_opened_again_to_write() {
    let home = tempfile::tempdir().unwrap();
    let path = store_in(home.path(), "jolly-mute-magma");
    let project = project("jolly-mute-magma", path);
    let mut stores = SqliteStores::default();
    block_on(async {
        SqliteStores::default().open(&project).await.unwrap();
        stores
            .open_existing(&project)
            .await
            .unwrap()
            .expect("found");
        stores.open(&project).await.unwrap();
    });
    assert!(stores.held.as_ref().unwrap().writable);
}

#[test]
fn test_reading_another_project_does_not_serve_the_held_one() {
    let home = tempfile::tempdir().unwrap();
    let mut stores = SqliteStores::default();
    block_on(async {
        let first = project(
            "jolly-mute-magma",
            store_in(home.path(), "jolly-mute-magma"),
        );
        stores.open(&first).await.unwrap();
        let other = project("tpyo", store_in(home.path(), "tpyo"));
        assert!(stores.open_existing(&other).await.unwrap().is_none());
    });
}

#[test]
fn test_reading_a_database_not_yet_migrated_migrates_it_as_a_write_would() {
    // A writer creates the file, then migrates it under its write lock; a
    // reader between the two finds it unmigrated (here, empty), and waits on
    // that lock for the migration rather than failing.
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("state.db");
    std::fs::write(&path, "").unwrap();
    let mut stores = SqliteStores::default();
    block_on(async {
        let store = stores
            .open_existing(&project("jolly-mute-magma", path))
            .await
            .unwrap()
            .expect("found");
        assert!(store.read_from(&some_stream(), 0).await.is_ok());
    });
    assert!(stores.held.as_ref().unwrap().writable);
}

#[test]
fn test_a_database_that_cannot_be_read_fails_a_read() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("state.db");
    std::fs::write(&path, "not a database").unwrap();
    let mut stores = SqliteStores::default();
    let error = block_on(stores.open_existing(&project("jolly-mute-magma", path)))
        .map(|found| found.is_some())
        .unwrap_err();
    assert!(
        matches!(error, annals::next::StoreError::Unavailable(_)),
        "{error:?}"
    );
}

#[test]
fn test_a_read_failing_for_another_reason_fails_with_the_read_only_opens_error() {
    // A directory where the database goes: the read-only open fails, and
    // so would a write open, but differently; the read's error is passed
    // through, and no write is tried.
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("state.db");
    std::fs::create_dir(&path).unwrap();
    let mut stores = SqliteStores::default();
    let error = block_on(stores.open_existing(&project("jolly-mute-magma", path.clone())))
        .map(|found| found.is_some())
        .unwrap_err();
    let read_only = block_on(SqliteStore::open_existing(&path))
        .map(|found| found.is_some())
        .unwrap_err();
    assert_eq!(error.to_string(), read_only.to_string());
}
