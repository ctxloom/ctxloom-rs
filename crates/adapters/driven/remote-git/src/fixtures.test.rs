//! The tests' own files: real paths on the host, reached through the
//! file-system seam (the adapter runs a real git, which names real paths),
//! under a root the test owns and removes.

use std::path::Path;
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

/// Whether the host runs shell scripts: a test that writes one asks.
pub(crate) fn runs_scripts() -> bool {
    os::host().flavor() == Flavor::Unix
}

pub(crate) fn mkdirs(path: impl AsRef<Path>) {
    fs().create_dir_all(path.as_ref(), Mode::from_bits(0o777))
        .unwrap();
}

pub(crate) fn put(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    fs().write_atomic(path.as_ref(), bytes.as_ref(), Perms::Exact(Mode::FILE))
        .unwrap();
}

/// A file the host would run.
pub(crate) fn put_exec(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    fs().write_atomic(
        path.as_ref(),
        bytes.as_ref(),
        Perms::Exact(Mode::from_bits(0o755)),
    )
    .unwrap();
}

pub(crate) fn slurp(path: impl AsRef<Path>) -> String {
    fs().read_to_string(path.as_ref()).unwrap()
}

pub(crate) fn exists(path: impl AsRef<Path>) -> bool {
    fs().exists(path.as_ref()).unwrap()
}

pub(crate) fn is_dir(path: impl AsRef<Path>) -> bool {
    fs().metadata(path.as_ref())
        .is_ok_and(|meta| meta.kind == Kind::Dir)
}

pub(crate) fn remove(path: impl AsRef<Path>) {
    fs().remove_file(path.as_ref()).unwrap();
}

pub(crate) fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    fs().rename(from.as_ref(), to.as_ref()).unwrap();
}

pub(crate) fn link(target: impl AsRef<Path>, link: impl AsRef<Path>) {
    fs().symlink_dir(target.as_ref(), link.as_ref()).unwrap();
}

/// The names in a directory.
pub(crate) fn names(path: impl AsRef<Path>) -> Vec<String> {
    fs().read_dir(path.as_ref())
        .unwrap()
        .into_iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect()
}

pub(crate) fn canonical(path: impl AsRef<Path>) -> std::path::PathBuf {
    fs().canonicalize(path.as_ref()).unwrap()
}
