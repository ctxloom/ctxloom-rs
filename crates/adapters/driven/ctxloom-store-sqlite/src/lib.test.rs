use std::path::Path;
use std::sync::Arc;

use file_system::{Fs, Rooted};

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
