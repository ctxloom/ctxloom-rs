//! The cache's files: measured, removed, and kept inside the app dir.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::ProjectCache;
use file_system::{Fs, Rooted};
use os::Mode;

use super::CacheFiles;

const APP: &str = "/work/.ctxloom";

fn cache() -> (Arc<Rooted>, CacheFiles) {
    let fs = Arc::new(Rooted::temp().unwrap());
    (fs.clone(), CacheFiles::new(fs, Path::new(APP)))
}

fn write(fs: &Rooted, rel: &str, body: &str) -> PathBuf {
    let path = Path::new(APP).join(rel);
    fs.create_dir_all(path.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        &path,
        body.as_bytes(),
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    path
}

#[test]
fn a_tree_is_measured_by_the_bytes_of_its_files() {
    let (fs, cache) = cache();
    write(&fs, "cache/context/a.md", "12345");
    write(&fs, "cache/context/deep/b.md", "123");
    assert_eq!(cache.measure("cache/context").unwrap(), Some(8));
}

#[test]
fn nothing_there_measures_none_and_removes_without_failing() {
    let (_, cache) = cache();
    assert_eq!(cache.measure("cache/repos").unwrap(), None);
    cache.remove("cache/repos").unwrap();
}

#[test]
fn remove_takes_the_tree_and_only_it() {
    let (fs, cache) = cache();
    let kept = write(&fs, "cache/bundles/x", "x");
    write(&fs, "cache/context/a.md", "a");
    cache.remove("cache/context").unwrap();
    assert_eq!(cache.measure("cache/context").unwrap(), None);
    assert!(fs.exists(&kept).unwrap());
}

#[test]
fn a_path_that_leaves_the_app_dir_is_refused() {
    let (_, cache) = cache();
    for rel in ["../elsewhere", "/etc", "cache/../../x"] {
        let error = cache.remove(rel).unwrap_err();
        assert_eq!(error.code(), "cache-files-outside", "{rel}");
        assert!(cache.measure(rel).is_err(), "{rel}");
    }
}
