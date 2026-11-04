use std::path::Path;
use std::sync::Arc;

use file_system::{Fs, Rooted};

use session_ports::ProjectStores;

use super::Stores;

#[test]
fn test_a_project_s_database_is_ctxloom_db_in_its_directory() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "/home/u");
    assert_eq!(
        stores.path("jolly-mute-magma"),
        Path::new("/home/u/.ctxloom/projects/jolly-mute-magma/ctxloom.db")
    );
}

#[tokio::test]
async fn test_open_makes_the_directory_and_the_database_under_the_home() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let stores = Stores::new(fs.clone(), "/home/u");
    stores.open("jolly-mute-magma").await.unwrap();
    let db = Path::new("/home/u/.ctxloom/projects/jolly-mute-magma/ctxloom.db");
    assert!(fs.exists(db).unwrap());
    let beside = Path::new("/home/u/.ctxloom/projects/jolly-mute-magma/state.db");
    assert!(!fs.exists(beside).unwrap(), "taskloom's store is not ours");
}

// ------------------------------------------------------- the ProjectStores port

fn project(id: &str) -> ports::Project {
    ports::Project {
        id: id.to_string(),
        root: None,
        store: std::path::PathBuf::from(format!("/home/u/.ctxloom/projects/{id}/state.db")),
    }
}

// Reading the record touches nothing (session.feature): a project with no
// database has none, and asking makes none.
#[tokio::test]
async fn test_open_existing_finds_no_store_and_makes_none() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut stores = super::SqliteStores::new(fs.clone(), "/home/u");
    assert!(
        ProjectStores::open_existing(&mut stores, &project("jolly-mute-magma"))
            .await
            .unwrap()
            .is_none()
    );
    let dir = Path::new("/home/u/.ctxloom/projects/jolly-mute-magma");
    assert!(!fs.exists(dir).unwrap(), "not even its directory");
}

#[tokio::test]
async fn test_open_makes_the_store_and_a_later_open_existing_finds_it() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut stores = super::SqliteStores::new(fs.clone(), "/home/u");
    ProjectStores::open(&mut stores, &project("jolly-mute-magma"))
        .await
        .unwrap();
    assert!(fs
        .exists(Path::new(
            "/home/u/.ctxloom/projects/jolly-mute-magma/ctxloom.db"
        ))
        .unwrap());
    // Another handle, as another process's read: the file is there.
    let mut reader = super::SqliteStores::new(fs, "/home/u");
    assert!(
        ProjectStores::open_existing(&mut reader, &project("jolly-mute-magma"))
            .await
            .unwrap()
            .is_some()
    );
}

// Each project has a store of its own, and asking for another does not
// confuse them.
#[tokio::test]
async fn test_each_project_has_its_own_store() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut stores = super::SqliteStores::new(fs, "/home/u");
    ProjectStores::open(&mut stores, &project("jolly-mute-magma"))
        .await
        .unwrap();
    assert!(
        ProjectStores::open_existing(&mut stores, &project("brisk-tall-oak"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        ProjectStores::open_existing(&mut stores, &project("jolly-mute-magma"))
            .await
            .unwrap()
            .is_some()
    );
}

// A store held to write serves a write again without opening afresh, and a
// read-only one is replaced by one opened to write.
#[tokio::test]
async fn test_a_read_only_store_is_replaced_when_a_write_asks() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut first = super::SqliteStores::new(fs.clone(), "/home/u");
    ProjectStores::open(&mut first, &project("jolly-mute-magma"))
        .await
        .unwrap();
    let mut stores = super::SqliteStores::new(fs, "/home/u");
    assert!(
        ProjectStores::open_existing(&mut stores, &project("jolly-mute-magma"))
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        ProjectStores::open(&mut stores, &project("jolly-mute-magma"))
            .await
            .is_ok()
    );
}
