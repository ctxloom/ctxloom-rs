//! Canonical paths, as Go's `cleanPath` gives them.

use std::path::{Path, PathBuf};

use super::{canonical, clean};

#[test]
fn test_a_clean_is_gos_filepath_clean() {
    for (path, cleaned) in [
        ("/a/b/c", "/a/b/c"),
        ("/a/b/c/", "/a/b/c"),
        ("/a//b/./c", "/a/b/c"),
        ("/a/b/../c", "/a/c"),
        ("/a/b/../../..", "/"),
        ("/..", "/"),
        ("a/./b/", "a/b"),
        ("a/../..", ".."),
        ("../a/..", ".."),
        ("../../a", "../../a"),
        ("./a", "a"),
        (".", "."),
        ("", "."),
    ] {
        assert_eq!(clean(Path::new(path)), Path::new(cleaned), "{path:?}");
    }
}

#[test]
fn test_a_canonical_path_resolves_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(dir.path()).unwrap().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = real.with_file_name("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(canonical(&link.join("")), real);
    assert_eq!(canonical(&link.join("../link/.")), real);
}

#[test]
fn test_a_path_that_does_not_resolve_is_cleaned() {
    assert_eq!(
        canonical(Path::new("/no/such/../place/")),
        PathBuf::from("/no/place")
    );
}
