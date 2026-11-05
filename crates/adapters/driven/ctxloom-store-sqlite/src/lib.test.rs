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

// ---------------------------------------------------------------- characterization
//
// Pinned before the store opener may move (A0): where a project's database
// is, what asking makes and what it does not.

use super::StoreOpenError;
use angzarr_diagnostic::Diagnostic;
use os::Mode;

#[test]
fn test_the_path_is_the_home_the_app_dir_projects_the_id_and_the_file_name() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "/h");
    let path = stores.path("p");
    let parts: Vec<_> = path.iter().map(|p| p.to_str().unwrap()).collect();
    assert_eq!(parts, ["/", "h", ".ctxloom", "projects", "p", "ctxloom.db"]);
    // asking where a database is makes nothing
    assert_eq!(stores.path("p"), path);
}

#[test]
fn test_the_path_is_the_id_joined_verbatim_with_no_check_of_it() {
    // Candidate bug, pinned as it is: an id with a separator or dots is not
    // refused or flattened, so it can leave the projects directory.
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "/h");
    assert_eq!(
        stores.path("../x"),
        Path::new("/h/.ctxloom/projects/../x/ctxloom.db")
    );
    assert_eq!(
        stores.path("a/b"),
        Path::new("/h/.ctxloom/projects/a/b/ctxloom.db")
    );
}

#[test]
fn test_the_path_is_relative_to_a_relative_home() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "rel/home");
    assert_eq!(
        stores.path("p"),
        Path::new("rel/home/.ctxloom/projects/p/ctxloom.db")
    );
}

#[tokio::test]
async fn test_open_twice_is_the_same_database_and_finds_it_the_second_time() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let stores = Stores::new(fs.clone(), "/home/u");
    stores.open("p").await.unwrap();
    stores.open("p").await.unwrap();
    assert!(stores.open_existing("p").await.unwrap().is_some());
}

#[tokio::test]
async fn test_open_existing_with_the_directory_but_no_database_finds_none_and_makes_none() {
    let fs = Arc::new(Rooted::temp().unwrap());
    fs.create_dir_all(
        Path::new("/home/u/.ctxloom/projects/p"),
        Mode::from_bits(0o755),
    )
    .unwrap();
    let stores = Stores::new(fs.clone(), "/home/u");
    assert!(stores.open_existing("p").await.unwrap().is_none());
    assert!(!fs
        .exists(Path::new("/home/u/.ctxloom/projects/p/ctxloom.db"))
        .unwrap());
}

#[tokio::test]
async fn test_a_home_whose_app_dir_is_a_file_cannot_hold_a_store() {
    let fs = Arc::new(Rooted::temp().unwrap());
    fs.create_dir_all(Path::new("/home/u"), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        Path::new("/home/u/.ctxloom"),
        b"a file",
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    let stores = Stores::new(fs, "/home/u");
    let Err(StoreOpenError::CreateDir { path, .. }) = stores.open("p").await else {
        panic!("the directory cannot be made");
    };
    assert_eq!(path, "/home/u/.ctxloom/projects/p");
    assert_eq!(
        StoreOpenError::CreateDir {
            path,
            detail: String::new()
        }
        .code(),
        "ctxloom-store-sqlite-create-dir"
    );
}

#[tokio::test]
async fn test_a_store_held_to_write_is_not_replaced_by_a_read_and_each_id_is_apart() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut stores = super::SqliteStores::new(fs.clone(), "/home/u");
    ProjectStores::open(&mut stores, &project("a"))
        .await
        .unwrap();
    // a read of the held project is served by the held store: nothing made
    ProjectStores::open_existing(&mut stores, &project("a"))
        .await
        .unwrap()
        .unwrap();
    // a read of another project that has no file makes none, and drops the hold
    assert!(ProjectStores::open_existing(&mut stores, &project("b"))
        .await
        .unwrap()
        .is_none());
    assert!(!fs.exists(Path::new("/home/u/.ctxloom/projects/b")).unwrap());
    // ...yet a's file is still there for the next read
    assert!(ProjectStores::open_existing(&mut stores, &project("a"))
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn test_the_project_port_places_the_store_by_id_not_by_the_projects_store_field() {
    // `Project.store` (taskloom's state.db) is ignored: ours is ctxloom.db
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut stores = super::SqliteStores::new(fs.clone(), "/home/u");
    let mut elsewhere = project("p");
    elsewhere.store = std::path::PathBuf::from("/elsewhere/state.db");
    ProjectStores::open(&mut stores, &elsewhere).await.unwrap();
    assert!(fs
        .exists(Path::new("/home/u/.ctxloom/projects/p/ctxloom.db"))
        .unwrap());
    assert!(!fs.exists(Path::new("/elsewhere")).unwrap());
}
