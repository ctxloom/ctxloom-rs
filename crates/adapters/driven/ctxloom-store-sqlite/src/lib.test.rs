use std::path::Path;
use std::sync::Arc;

use file_system::{Fs, Rooted};

use session_ports::ProjectStores;

use super::Stores;

#[test]
fn test_a_project_s_database_is_ctxloom_db_in_its_directory() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "/home/u");
    assert_eq!(
        stores.path("jolly-mute-magma").unwrap(),
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

use super::{OpenError, StoreOpenError};
use angzarr_diagnostic::Diagnostic;
use angzarr_embedded::angzarr_ports::Arg as PortArg;
use angzarr_embedded::StoreError;
use os::Mode;

#[test]
fn test_the_path_is_the_home_the_app_dir_projects_the_id_and_the_file_name() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "/h");
    let path = stores.path("p").unwrap();
    let parts: Vec<_> = path.iter().map(|p| p.to_str().unwrap()).collect();
    assert_eq!(parts, ["/", "h", ".ctxloom", "projects", "p", "ctxloom.db"]);
    // asking where a database is makes nothing
    assert_eq!(stores.path("p").unwrap(), path);
}

/// Defect (1), fixed: an id that is not one plain name is refused, with the
/// shared crate's coded diagnostic, rather than joined verbatim (it could
/// leave the projects directory).
#[test]
fn test_an_unsafe_project_id_is_refused_with_a_coded_diagnostic() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "/h");
    let cases = [
        ("", "project-dir-id-empty"),
        ("/etc", "project-dir-id-absolute"),
        ("../x", "project-dir-id-separator"),
        ("a/b", "project-dir-id-separator"),
        ("a\\b", "project-dir-id-separator"),
        ("..", "project-dir-id-relative"),
        ("a\0b", "project-dir-id-nul"),
    ];
    for (id, code) in cases {
        let error = stores.path(id).expect_err(id);
        assert_eq!(error.code(), code, "{id:?}");
        assert!(matches!(error, StoreOpenError::UnsafeProject(_)), "{id:?}");
        let shown = |a: Vec<(&str, angzarr_diagnostic::Arg)>| format!("{a:?}");
        assert_eq!(
            shown(error.args()),
            if id.is_empty() {
                "[]".to_string()
            } else {
                format!("[(\"id\", Str({id:?}))]")
            },
            "{id:?}"
        );
    }
}

#[tokio::test]
async fn test_an_unsafe_project_id_opens_nothing_and_makes_nothing() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let stores = Stores::new(fs.clone(), "/h");
    let error = stores.open("../x").await.err().expect("refused");
    assert_eq!(error.code(), "project-dir-id-separator");
    let error = stores.open_existing("../x").await.err().expect("refused");
    let StoreError::Unavailable(cause) = error else {
        panic!("not Unavailable: {error:?}");
    };
    assert_eq!(cause.code, "project-dir-id-separator");
    assert_eq!(cause.args, [("id", PortArg::Str("../x".to_string()))]);
    assert!(!fs.exists(Path::new("/h")).unwrap(), "not even the home");
}

#[tokio::test]
async fn test_the_port_refuses_an_unsafe_project_id_too() {
    let fs = Arc::new(Rooted::temp().unwrap());
    let mut stores = super::SqliteStores::new(fs.clone(), "/h");
    assert!(ProjectStores::open(&mut stores, &project("a/b"))
        .await
        .is_err());
    assert!(ProjectStores::open_existing(&mut stores, &project("a/b"))
        .await
        .is_err());
    assert!(!fs.exists(Path::new("/h")).unwrap());
}

#[test]
fn test_the_path_is_relative_to_a_relative_home() {
    let stores = Stores::new(Arc::new(Rooted::temp().unwrap()), "rel/home");
    assert_eq!(
        stores.path("p").unwrap(),
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
    let Err(StoreOpenError::Open(error @ OpenError::CreateDir { .. })) = stores.open("p").await
    else {
        panic!("the directory cannot be made");
    };
    let OpenError::CreateDir { path, .. } = &error else {
        unreachable!()
    };
    assert_eq!(path, "/home/u/.ctxloom/projects/p");
    assert_eq!(error.code(), "sqlite-store-opener-create-dir");
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

// ------------------------------------------------ a store from before the embedded store
//
// D7 (the extraction plan): ctxloom-rs inherits the opener's refusal. Before
// the opener was shared, ctxloom opened any file at `ctxloom.db`, migrating
// an earlier event store's in place; now it is refused, naming the backup to
// move it to, and left exactly as it was.

/// An earlier event store's file: one stream of two events under its rules,
/// its ledger only its first migration. Frozen in testdata.
const PRE_EMBEDDED: &[u8] = include_bytes!("../testdata/pre-embedded.db");

const DB_DIR: &str = "/home/u/.ctxloom/projects/jolly-mute-magma";

fn earlier_store(fs: &Rooted) -> Vec<u8> {
    fs.create_dir_all(Path::new(DB_DIR), Mode::from_bits(0o755))
        .unwrap();
    let db = Path::new(DB_DIR).join("ctxloom.db");
    fs.write_atomic(
        &db,
        PRE_EMBEDDED,
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    fs.read(&db).unwrap()
}

fn refusal_args(error: &dyn Diagnostic) -> Vec<(String, String)> {
    assert_eq!(error.code(), "sqlite-store-opener-predates-embedded");
    error
        .args()
        .into_iter()
        .map(|(name, arg)| match arg {
            angzarr_diagnostic::Arg::Str(text) => (name.to_string(), text),
            other => panic!("{name}: {other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn test_a_store_that_predates_the_embedded_store_is_refused_to_write_and_to_read_and_left_as_it_was(
) {
    let fs = Arc::new(Rooted::temp().unwrap());
    let before = earlier_store(&fs);
    let stores = Stores::new(fs.clone(), "/home/u");
    let expected = vec![
        ("path".to_string(), format!("{DB_DIR}/ctxloom.db")),
        (
            "backup".to_string(),
            format!("{DB_DIR}/ctxloom.db.pre-embedded"),
        ),
    ];
    let error = stores
        .open("jolly-mute-magma")
        .await
        .err()
        .expect("refused");
    assert_eq!(refusal_args(&error), expected, "to write");
    let error = stores
        .open_existing("jolly-mute-magma")
        .await
        .err()
        .expect("refused");
    let StoreError::Unavailable(cause) = error else {
        panic!("not Unavailable: {error:?}");
    };
    assert_eq!(cause.code, "sqlite-store-opener-predates-embedded");
    assert_eq!(
        cause.args,
        [
            ("path", PortArg::Str(expected[0].1.clone())),
            ("backup", PortArg::Str(expected[1].1.clone())),
        ],
        "to read"
    );
    assert_eq!(
        fs.read(&Path::new(DB_DIR).join("ctxloom.db")).unwrap(),
        before,
        "the file as it was: not migrated"
    );
}

#[tokio::test]
async fn test_the_port_refuses_a_store_that_predates_the_embedded_store_and_holds_nothing() {
    let fs = Arc::new(Rooted::temp().unwrap());
    earlier_store(&fs);
    let mut stores = super::SqliteStores::new(fs.clone(), "/home/u");
    let wanted = project("jolly-mute-magma");
    let error = ProjectStores::open(&mut stores, &wanted)
        .await
        .err()
        .expect("refused to write");
    let StoreError::Unavailable(cause) = &error else {
        panic!("not Unavailable: {error:?}");
    };
    assert_eq!(cause.code, "sqlite-store-opener-predates-embedded");
    let error = ProjectStores::open_existing(&mut stores, &wanted)
        .await
        .err()
        .expect("refused to read");
    let StoreError::Unavailable(cause) = &error else {
        panic!("not Unavailable: {error:?}");
    };
    assert_eq!(cause.code, "sqlite-store-opener-predates-embedded");
}
