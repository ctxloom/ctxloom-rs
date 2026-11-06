//! The tests' own files: real paths on the host, reached through the
//! file-system seam, under a root the test owns and removes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use file_system::{Fs, Kind, Perms, Rooted};
use os::{Flavor, Mode};

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

/// Whether the host has unix permission bits and shell scripts.
pub(crate) fn unix() -> bool {
    os::host().flavor() == Flavor::Unix
}

pub(crate) fn mkdirs(path: impl AsRef<Path>) {
    fs().create_dir_all(path.as_ref(), Mode::from_bits(0o777))
        .unwrap();
}

/// A file with this mode.
pub(crate) fn put_mode(path: impl AsRef<Path>, text: &str, mode: u32) {
    fs().write_atomic(
        path.as_ref(),
        text.as_bytes(),
        Perms::Exact(Mode::from_bits(mode)),
    )
    .unwrap();
}

/// A program that runs `body`: `path` is linked to `scripts/interpret.sh`
/// with the script beside it, so the written file is read by `sh` and never
/// exec'd (see that file for why).
pub(crate) fn put_program(path: impl AsRef<Path>, body: &str) {
    let mut beside = path.as_ref().as_os_str().to_owned();
    beside.push(".body");
    fs().write_atomic(
        Path::new(&beside),
        body.as_bytes(),
        Perms::Exact(Mode::FILE),
    )
    .unwrap();
    let shim = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../scripts/interpret.sh");
    fs().symlink_dir(&shim, path.as_ref()).unwrap();
}

pub(crate) fn exists(path: impl AsRef<Path>) -> bool {
    fs().exists(path.as_ref()).unwrap()
}

pub(crate) fn remove_dir(path: impl AsRef<Path>) {
    fs().remove_dir_all(path.as_ref()).unwrap();
}

pub(crate) fn canonical(path: impl AsRef<Path>) -> PathBuf {
    fs().canonicalize(path.as_ref()).unwrap()
}

/// The entries of a directory, with whether each is a directory.
pub(crate) fn entries(path: impl AsRef<Path>) -> Vec<(String, bool)> {
    fs().read_dir(path.as_ref())
        .unwrap()
        .into_iter()
        .map(|e| (e.name.to_string_lossy().into_owned(), e.kind == Kind::Dir))
        .collect()
}
