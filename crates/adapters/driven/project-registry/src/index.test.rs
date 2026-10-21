//! The registry file: Go's schema read and written, unknown fields kept.

use std::fs;

use super::{load, lock, path, save, Index, SCHEMA_VERSION};
use crate::RegistryError;

const GO_WRITTEN: &str = "\
schema_version: 1
projects:
  - project_id: jolly-mute-magma
    path: /tmp/ctxloom-smoke
    created_at: 2026-06-10T02:35:20.837009562Z
    last_seen_at: 2026-06-10T02:35:20.837009562Z
    note: kept
  - project_id: high-tight-gulf
    path: /work/main
    created_at: 2026-06-10T02:42:33.650686047Z
later: field
";

#[test]
fn test_the_registry_is_under_the_homes_ctxloom_projects() {
    assert_eq!(
        path("/home/u".as_ref()),
        std::path::Path::new("/home/u/.ctxloom/projects/index.yaml")
    );
}

#[test]
fn test_a_registry_go_wrote_is_read_with_each_entry() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("index.yaml");
    fs::write(&file, GO_WRITTEN).unwrap();
    let index = load(&file).unwrap();
    assert_eq!(index.schema_version, 1);
    assert_eq!(index.id_for("/work/main"), Some("high-tight-gulf"));
    assert_eq!(index.id_for("/tmp/ctxloom-smoke"), Some("jolly-mute-magma"));
    assert_eq!(index.id_for("/elsewhere"), None);
    assert!(index.holds("high-tight-gulf"));
    assert!(!index.holds("other"));
    assert_eq!(index.projects[1].last_seen_at, "", "absent: empty");
}

#[test]
fn test_saving_keeps_what_was_read_and_what_this_version_does_not_know() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("index.yaml");
    fs::write(&file, GO_WRITTEN).unwrap();
    let mut index = load(&file).unwrap();
    index.register("calm-otter-fox", "/new");
    save(&file, &index).unwrap();
    let again = load(&file).unwrap();
    assert_eq!(again.projects[..2], index.projects[..2]);
    assert_eq!(again.projects[0].other["note"], "kept");
    assert_eq!(again.other["later"], "field");
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        !text.contains("last_seen_at: ''"),
        "an absent time stays absent: {text}"
    );
}

#[test]
fn test_a_registered_entry_carries_its_id_path_and_when() {
    let mut index = Index::default();
    index.register("calm-otter-fox", "/new");
    let entry = &index.projects[0];
    assert_eq!(entry.project_id, "calm-otter-fox");
    assert_eq!(entry.path, "/new");
    assert_eq!(entry.created_at, entry.last_seen_at);
    let at = time::OffsetDateTime::parse(
        &entry.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .expect("RFC 3339");
    assert!((time::OffsetDateTime::now_utc() - at).whole_seconds().abs() < 60);
}

#[test]
fn test_a_new_registry_is_written_at_this_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("deeper").join("index.yaml");
    save(&file, &Index::default()).unwrap();
    assert_eq!(load(&file).unwrap().schema_version, SCHEMA_VERSION);
}

#[test]
fn test_a_missing_or_blank_registry_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("index.yaml");
    assert_eq!(load(&file).unwrap(), Index::default());
    fs::write(&file, "\n  \n").unwrap();
    assert_eq!(load(&file).unwrap(), Index::default());
}

#[test]
fn test_a_registry_that_is_not_yaml_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("index.yaml");
    fs::write(&file, "projects: [unterminated").unwrap();
    assert!(matches!(load(&file), Err(RegistryError::Unreadable { .. })));
}

#[test]
fn test_a_registry_that_cannot_be_read_fails() {
    let dir = tempfile::tempdir().unwrap();
    // A directory where the file should be.
    assert!(matches!(load(dir.path()), Err(RegistryError::Io { .. })));
}

#[test]
fn test_the_lock_is_the_file_go_locks_and_excludes_another_holder() {
    use fs4::FileExt;

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sub").join("index.yaml");
    let held = lock(&file).unwrap();
    let lock_file = dir.path().join("sub").join("index.yaml.lock");
    let other = fs::File::open(&lock_file).expect("the lock file exists");
    assert!(
        FileExt::try_lock(&other).is_err(),
        "held exclusively while the guard lives"
    );
    drop(held);
    assert!(FileExt::try_lock(&other).is_ok(), "released on drop");
}
