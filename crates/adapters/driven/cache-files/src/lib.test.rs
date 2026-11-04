//! The cache's files: measured, removed, and kept inside the app dir.

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::ProjectCache;
use file_system::{CwdError, Entry, Fs, Meta, Open, Perms, Rooted};
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

#[test]
fn the_app_dir_is_the_one_the_cache_was_made_over() {
    let (_, cache) = cache();
    assert_eq!(cache.app_dir(), APP);
}

#[test]
fn a_file_is_removed_as_a_file_and_measured_by_its_bytes() {
    let (fs, cache) = cache();
    let file = write(&fs, "cache/one.md", "12345");
    assert_eq!(cache.measure("cache/one.md").unwrap(), Some(5));
    cache.remove("cache/one.md").unwrap();
    assert!(!fs.exists(&file).unwrap());
}

fn chmod(fs: &Rooted, rel: &str, mode: u32) {
    fs.set_permissions(&Path::new(APP).join(rel), Mode::from_bits(mode))
        .unwrap();
}

#[test]
fn a_failure_that_is_not_absence_is_reported_not_swallowed() {
    let (fs, cache) = cache();
    write(&fs, "cache/locked/inner/f", "x");
    write(&fs, "cache/readonly/f", "x");
    // Listable but not searchable: the entries are there, their metadata is
    // not reachable.
    chmod(&fs, "cache/locked", 0o600);
    let measured = cache.measure("cache/locked/inner");
    let totalled = cache.measure("cache/locked");
    // Searchable but not writable: nothing in it can be removed.
    chmod(&fs, "cache/readonly", 0o500);
    let removed = cache.remove("cache/readonly/f");
    chmod(&fs, "cache/locked", 0o700);
    chmod(&fs, "cache/readonly", 0o700);

    assert_eq!(measured.unwrap_err().code(), "cache-files-io");
    assert_eq!(totalled.unwrap_err().code(), "cache-files-io");
    assert_eq!(removed.unwrap_err().code(), "cache-files-io");
    assert!(fs.exists(&Path::new(APP).join("cache/readonly/f")).unwrap());
}

/// A file system that lists one name more than is there: what a tree looks
/// like when something else reaps an entry between the listing and the look.
#[derive(Debug)]
struct Ghosting {
    inner: Arc<Rooted>,
    ghost: &'static str,
}

impl Fs for Ghosting {
    fn cwd(&self) -> Result<&Path, CwdError> {
        self.inner.cwd()
    }
    fn os(&self) -> &dyn os::Os {
        self.inner.os()
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.inner.read(path)
    }
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.inner.read_to_string(path)
    }
    fn write_atomic(&self, path: &Path, bytes: &[u8], perms: Perms) -> io::Result<()> {
        self.inner.write_atomic(path, bytes, perms)
    }
    fn open(&self, path: &Path, how: Open) -> io::Result<File> {
        self.inner.open(path, how)
    }
    fn create_dir_all(&self, path: &Path, mode: Mode) -> io::Result<()> {
        self.inner.create_dir_all(path, mode)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_file(path)
    }
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_dir_all(path)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.inner.rename(from, to)
    }
    fn set_permissions(&self, path: &Path, mode: Mode) -> io::Result<()> {
        self.inner.set_permissions(path, mode)
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<Entry>> {
        let mut entries = self.inner.read_dir(path)?;
        entries.push(Entry {
            name: OsString::from(self.ghost),
            kind: file_system::Kind::File,
        });
        Ok(entries)
    }
    fn metadata(&self, path: &Path) -> io::Result<Meta> {
        self.inner.metadata(path)
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<Meta> {
        self.inner.symlink_metadata(path)
    }
    fn exists(&self, path: &Path) -> io::Result<bool> {
        self.inner.exists(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.inner.canonicalize(path)
    }
    fn symlink_dir(&self, target: &Path, link: &Path) -> io::Result<()> {
        self.inner.symlink_dir(target, link)
    }
    fn read_link(&self, link: &Path) -> io::Result<PathBuf> {
        self.inner.read_link(link)
    }
    fn make_temp_dir(&self, parent: &Path, prefix: &str) -> io::Result<PathBuf> {
        self.inner.make_temp_dir(parent, prefix)
    }
    fn real_path(&self, path: &Path) -> io::Result<PathBuf> {
        self.inner.real_path(path)
    }
    fn same_file(&self, file: &File, path: &Path) -> io::Result<bool> {
        self.inner.same_file(file, path)
    }
}

#[test]
fn an_entry_reaped_between_the_listing_and_the_look_counts_for_no_bytes() {
    let (fs, _) = cache();
    write(&fs, "cache/context/a.md", "12345");
    let ghosting = CacheFiles::new(
        Arc::new(Ghosting {
            inner: fs,
            ghost: "reaped.md",
        }),
        Path::new(APP),
    );
    assert_eq!(ghosting.measure("cache/context").unwrap(), Some(5));
}
