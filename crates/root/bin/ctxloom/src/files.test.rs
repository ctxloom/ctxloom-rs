//! The tests' own files: real paths on the host, reached through the
//! file-system seam (the tests run the real binary, which names real paths),
//! under a root the test owns and removes.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use file_system::{Fs, Kind, Open, Perms, Rooted};
use os::Mode;

/// The host's own filesystem (no root), for real paths.
pub(crate) fn fs() -> Arc<dyn Fs> {
    let os = os::host();
    let top = os.flavor().top();
    Arc::new(file_system::Os::new(os, Path::new(top)).unwrap())
}

/// A temporary root, removed when dropped.
pub(crate) fn root() -> Rooted {
    Rooted::temp().unwrap()
}

pub(crate) fn mkdirs(path: impl AsRef<Path>) {
    fs().create_dir_all(path.as_ref(), Mode::from_bits(0o777))
        .expect("a directory");
}

/// A file with this content, its directory made.
pub(crate) fn put(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    put_mode(path, bytes, 0o644);
}

/// A file with this content and mode, its directory made.
pub(crate) fn put_mode(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>, mode: u32) {
    if let Some(parent) = path.as_ref().parent() {
        mkdirs(parent);
    }
    fs().write_atomic(
        path.as_ref(),
        bytes.as_ref(),
        Perms::Exact(Mode::from_bits(mode)),
    )
    .expect("a file");
}

/// A program that runs `text`, a script for `sh`: written beside the program
/// and linked to a shim, so no file the test wrote is exec'd (ETXTBSY). The
/// only way a test makes a file to run; its directory made.
pub(crate) fn put_program(path: impl AsRef<Path>, text: &str) {
    if let Some(parent) = path.as_ref().parent() {
        mkdirs(parent);
    }
    fixture_program::install_program(&*fs(), path.as_ref(), text).expect("a program");
}

pub(crate) fn set_mode(path: impl AsRef<Path>, mode: u32) {
    fs().set_permissions(path.as_ref(), Mode::from_bits(mode))
        .expect("a mode");
}

/// The permission bits; none where the host has none.
pub(crate) fn mode_of(path: impl AsRef<Path>) -> Option<u32> {
    let meta = fs().metadata(path.as_ref()).ok()?;
    meta.mode.map(|m| m.bits())
}

pub(crate) fn read(path: impl AsRef<Path>) -> std::io::Result<String> {
    fs().read_to_string(path.as_ref())
}

pub(crate) fn read_bytes(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    fs().read(path.as_ref())
}

pub(crate) fn exists(path: impl AsRef<Path>) -> bool {
    fs().exists(path.as_ref()).unwrap_or(false)
}

/// Whether a directory is there, links followed.
pub(crate) fn is_dir(path: impl AsRef<Path>) -> bool {
    fs().metadata(path.as_ref())
        .is_ok_and(|meta| meta.kind == Kind::Dir)
}

pub(crate) fn remove(path: impl AsRef<Path>) {
    fs().remove_file(path.as_ref()).expect("a file to remove");
}

/// A directory and everything below it.
pub(crate) fn remove_tree(path: impl AsRef<Path>) {
    fs().remove_dir_all(path.as_ref())
        .expect("a directory to remove");
}

pub(crate) fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    fs().rename(from.as_ref(), to.as_ref()).expect("a rename");
}

pub(crate) fn canonical(path: impl AsRef<Path>) -> PathBuf {
    fs().canonicalize(path.as_ref()).expect("a real path")
}

/// The paths directly in a directory; none where it cannot be read.
pub(crate) fn children(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let Ok(entries) = fs().read_dir(path.as_ref()) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .map(|e| path.as_ref().join(e.name))
        .collect()
}

/// A file opened for reading.
pub(crate) fn open_read(path: impl AsRef<Path>) -> std::io::Result<File> {
    fs().open(path.as_ref(), Open::Read)
}

/// A copy of a file, its mode kept.
pub(crate) fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let fs = fs();
    let bytes = fs.read(from.as_ref()).expect("a file");
    let mode = fs
        .metadata(from.as_ref())
        .ok()
        .and_then(|meta| meta.mode)
        .unwrap_or(Mode::FILE);
    fs.write_atomic(to.as_ref(), &bytes, Perms::Exact(mode))
        .expect("a file copy");
}

/// The directories of a `PATH`, as the host reads one.
pub(crate) fn search_path(path: &std::ffi::OsStr) -> Vec<PathBuf> {
    os::host().flavor().split_search_path_os(path)
}
