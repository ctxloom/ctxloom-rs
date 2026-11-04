//! The ports the core needs from outside (S1): the project's content tree and
//! its config layers. The library read model holds no path and reads no file;
//! an adapter (`content-fs`, `config-files`) reads, and hands the text and the
//! bytes over.

use std::collections::{BTreeMap, BTreeSet};

use angzarr_diagnostic::Diagnostic;

/// Why an adapter could not do what it was asked: its own diagnostic.
pub type Failure = Box<dyn Diagnostic>;

/// A bundle's files by path relative to the bundle (`bundle.yaml`,
/// `fragments/solid.md`, `skills/.code-reviewer.meta.yaml`), slash separated.
pub type TreeFiles = BTreeMap<String, Vec<u8>>;

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
}

/// Where a program is on the user's `PATH`.
pub trait Executables {
    /// The path of the first file named `name` on the `PATH` that may be
    /// run, as it is shown; none where there is none.
    fn find(&self, name: &str) -> Option<String>;
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

    /// A `CTXLOOM_ROOT` that named no directory, and so was ignored.
    fn ignored_root(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
