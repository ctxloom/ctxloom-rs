//! The ports the core needs from outside: the project's content tree and its
//! config layers (S1), and what the write side adds to them (S2): changes to a
//! bundle's tree, the user's own files (export and import), the config layers
//! written, an editor run, and a companion probed. The library read model holds
//! no path and reads no file; an adapter (`content-fs`, `config-files`,
//! `editor-process`, `companion-probe`) does, and hands the text and the bytes
//! over.

use std::collections::{BTreeMap, BTreeSet};

use angzarr_diagnostic::Diagnostic;

/// Why an adapter could not do what it was asked: its own diagnostic.
pub type Failure = Box<dyn Diagnostic>;

/// A bundle's files by path relative to the bundle (`bundle.yaml`,
/// `fragments/solid.md`, `skills/.code-reviewer.meta.yaml`), slash separated.
pub type TreeFiles = BTreeMap<String, Vec<u8>>;

/// A file to put somewhere: its path below the directory it lands in, slash
/// separated.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Put {
    pub path: String,
    pub bytes: Vec<u8>,
    /// Whether it carries the execute bits.
    pub executable: bool,
}

impl Put {
    /// A plain file.
    pub fn file(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Put {
        Put {
            path: path.into(),
            bytes: bytes.into(),
            executable: false,
        }
    }
}

/// One change to a bundle's tree.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Change {
    /// A file written whole (replacing the one there), its directories made.
    Write(Put),
    /// A file taken away; no error where it is not there.
    Remove { path: String },
    /// A directory taken away with everything in it; no error where it is not
    /// there.
    RemoveDir { path: String },
}

/// The project's authored bundles: `<app dir>/content/bundles/v2/<name>/`,
/// where a directory holding a `bundle.yaml` is a bundle (and everything
/// below it is its items), however deep its name's path is.
pub trait ContentRepo {
    /// Whether the bundles directory is there at all.
    fn has_root(&self) -> Result<bool, Failure>;

    /// The names of the bundles (their paths below the root, slash
    /// separated), sorted. None when the root is not there.
    fn bundle_names(&self) -> Result<Vec<String>, Failure>;

    /// The files of the bundle `name`'s tree.
    fn read_tree(&self, name: &str) -> Result<TreeFiles, Failure>;

    /// Where the bundle's `bundle.yaml` is, as the user is shown it.
    fn manifest_path(&self, name: &str) -> String;

    /// The files of the bundle's tree that carry an execute bit, by their
    /// path relative to the bundle: what a skill package's listing reports
    /// as its mode.
    fn executable_files(&self, name: &str) -> Result<BTreeSet<String>, Failure>;

    /// Apply `changes` to the tree of the bundle `name`, in order, each file
    /// written atomically and its directories made. A path that is absolute,
    /// climbs out of the bundle or goes through a link is refused before
    /// anything is changed.
    fn apply(&self, name: &str, changes: &[Change]) -> Result<(), Failure>;

    /// Claim the directory of a new bundle in one step (a race between two
    /// creators has one winner): true when this call made it, false when
    /// anything was there already.
    fn claim(&self, name: &str) -> Result<bool, Failure>;

    /// Take the bundle's whole tree away; no error where it is not there.
    fn remove_bundle(&self, name: &str) -> Result<(), Failure>;

    /// Where the bundle's directory is, as the user is shown it.
    fn bundle_dir(&self, name: &str) -> String;
}

/// The user's own files, outside the project's content: where a bundle is
/// exported to and imported from, a profile too, a skill's archive. Paths are
/// as the user typed them (relative to the working directory).
pub trait UserFiles {
    /// Whether anything is at `path`.
    fn exists(&self, path: &str) -> Result<bool, Failure>;

    /// Whether a directory is at `path`.
    fn is_dir(&self, path: &str) -> Result<bool, Failure>;

    /// The file's bytes.
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Failure>;

    /// Every file below the directory, by path below it, sorted, with its
    /// execute bit; links are not followed.
    fn read_tree(&self, dir: &str) -> Result<Vec<Put>, Failure>;

    /// The file written whole, its directories made. With `overwrite` false
    /// nothing is replaced: the answer is false and nothing is written when
    /// a file is there.
    fn write_file(&self, path: &str, bytes: &[u8], overwrite: bool) -> Result<bool, Failure>;

    /// The files written below `dir`, its directories made.
    fn write_tree(&self, dir: &str, files: &[Put]) -> Result<(), Failure>;
}

/// Where a program is on the user's `PATH`.
pub trait Executables {
    /// The path of the first file named `name` on the `PATH` that may be
    /// run, as it is shown; none where there is none.
    fn find(&self, name: &str) -> Option<String>;
}

/// The environment ctxloom was launched in, as far as credentials go: whether
/// a variable is exported with a value, and nothing more. The value is not on
/// offer, so no use case can print or store it.
pub trait ExportedVars {
    /// Whether `name` is exported with a non-empty value.
    fn is_exported(&self, name: &str) -> bool;
}

/// Where the configuration came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// A project's `.ctxloom` (named by `CTXLOOM_ROOT`, or found by walking
    /// up from the working directory).
    Project,
    /// No project: the home's `~/.ctxloom` alone.
    Home,
}

/// Which config file: the project's, or the home's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    Project,
    Home,
}

/// One config file's text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LayerText {
    /// The file's path, as the user is shown it.
    pub path: String,
    pub text: String,
}

/// The layered `config.yaml` files.
pub trait ConfigFiles {
    /// Where this invocation's configuration is from.
    fn source(&self) -> Source;

    /// The project's `.ctxloom` directory as shown (or the home's, with no
    /// project).
    fn app_dir(&self) -> String;

    /// The layers' files that are there, lowest first: the home's, then the
    /// project's (the home's alone with no project, and not twice when they
    /// are one file).
    fn layers(&self) -> Result<Vec<LayerText>, Failure>;

    /// The text of `scope`'s config file, if it is there.
    fn layer(&self, scope: Scope) -> Result<Option<LayerText>, Failure>;

    /// Where `scope`'s config file is, as the user is shown it.
    fn layer_path(&self, scope: Scope) -> String;

    /// `scope`'s config file written whole and atomically (its directory
    /// made); an existing file keeps its mode.
    fn write_layer(&self, scope: Scope, text: &str) -> Result<(), Failure>;

    /// A `CTXLOOM_ROOT` that named no directory, and so was ignored.
    fn ignored_root(&self) -> Option<String> {
        None
    }
}

/// Where a new project is made: the `.ctxloom` under the working directory
/// (not the one found by walking up, which may be another project's or the
/// home's).
pub trait Scaffold {
    /// That directory, as the user is shown it.
    fn app_dir(&self) -> String;

    /// Whether the file at `rel` below that directory is there.
    fn exists(&self, rel: &str) -> Result<bool, Failure>;

    /// The files written below that directory, each atomically, its
    /// directories made (mode 0644 files, 0755 directories).
    fn write(&self, files: &[Put]) -> Result<(), Failure>;
}

/// An editor to run: a program and the arguments that come before the file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EditorCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// The user's editor.
pub trait Editor {
    /// `content` handed to the editor in a file named `file_name`, and what
    /// the editor left in it. The file is removed afterwards.
    fn edit(
        &self,
        command: &EditorCommand,
        file_name: &str,
        content: &str,
    ) -> Result<String, Failure>;

    /// The editor opened on the file at `path`, which it edits in place.
    fn open(&self, command: &EditorCommand, path: &str) -> Result<(), Failure>;
}

/// Why a program is no companion.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Unanswered {
    /// It could not be run, failed, or took too long: what happened.
    Failed(String),
    /// It ran and printed no loadout.
    Silent,
}

/// Whether a program answers the companion probe.
pub trait LoadoutProbe {
    /// `<path> loadout --format yaml`, bounded in time: it must succeed and
    /// print something.
    fn answers(&self, path: &str) -> Result<(), Unanswered>;
}

/// The project's regenerable cache (`.ctxloom/cache`): measured and removed
/// by path below the app dir, nothing else (S12, `ctxloom clean`).
pub trait ProjectCache {
    /// The project's `.ctxloom` directory, as the user is shown it.
    fn app_dir(&self) -> String;

    /// The bytes of the files below `rel` (a file's own size); none where
    /// nothing is at `rel`. A file that vanishes mid-walk counts nothing.
    fn measure(&self, rel: &str) -> Result<Option<u64>, Failure>;

    /// Everything at `rel` removed; nothing there is not a failure.
    fn remove(&self, rel: &str) -> Result<(), Failure>;
}

/// Whether `name` is a slash-separated relative path of plain segments: not
/// empty, not absolute, and no empty, `.` or `..` segment, so it cannot climb
/// out of the directory it is joined to. An adapter adds what its own
/// filesystem also refuses (a backslash, a NUL).
pub fn is_plain_relative(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
