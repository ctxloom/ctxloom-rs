//! The ports the dependency commands (S3) need from outside.
//!
//! - [`Remotes`]: the repositories content comes from, over git: clone,
//!   fetch, resolve a version to a commit, read a tree at a commit, publish.
//! - [`Forge`]: a forge's API (GitHub's): search its repositories, and open a
//!   bundle as a pull request. The credential is the forge's own, read from the
//!   environment by the adapter; it is never a value of this port.
//! - [`DepsFiles`]: the project's `remotes.yaml` and `lock.yaml`, the trees
//!   installed from remotes (the derived cache), and the bundles it authors.
//! - [`RemoteLibrary`]: what the library reads of all that: the installed
//!   bundles, a short reference spelled out, and a search of the remotes.
//!
//! A failure carries git's own account (third-party text, shown as it is): a
//! reason is something to say, not a code to branch on.

use std::collections::BTreeMap;

use angzarr_diagnostic::Diagnostic;

/// Why the library could not read what it was handed: a diagnostic of the
/// adapter or the use case behind the port.
pub type Failure = Box<dyn Diagnostic>;

/// Why a repository, a file or a tree could not be read or written: the
/// third party's own words.
pub type Why = String;

/// One file of a tree read from a repository.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RepoFile {
    pub data: Vec<u8>,
    /// Whether the publisher committed it executable.
    pub executable: bool,
}

/// A tree: files by path relative to its root.
pub type RepoTree = BTreeMap<String, RepoFile>;

/// The repositories content comes from, and where it is published.
///
/// A clone is cached per repository. Reads are from the commits and the
/// remote-tracking refs the last fetch left, never from a checked-out working
/// tree: a stale checkout must not leak into what is served.
pub trait Remotes {
    /// A repository given as a path on this machine, as the file URL of the
    /// repository there.
    fn local_repository(&self, path: &str) -> Result<String, Why>;

    /// Clone the repository at `url` into the cache if it is not there;
    /// nothing is fetched when it is.
    fn ensure(&self, url: &str) -> Result<(), Why>;

    /// Bring the cached clone up to date (cloning it when it is absent): the
    /// remote-tracking refs and the tags move to the live remote, and what the
    /// remote dropped is dropped. An error means the repository could not be
    /// read this run.
    fn refresh(&self, url: &str) -> Result<(), Why>;

    /// The repository's tag names, from the cached clone.
    fn tags(&self, url: &str) -> Result<Vec<String>, Why>;

    /// A branch (its remote-tracking tip), a tag or a commit name as the commit
    /// it is, in the cached clone.
    fn resolve_ref(&self, url: &str, name: &str) -> Result<String, Why>;

    /// A tag name as its commit, through the tags alone.
    fn resolve_tag(&self, url: &str, name: &str) -> Result<String, Why>;

    /// The default branch's name.
    fn default_branch(&self, url: &str) -> Result<String, Why>;

    /// The files of the bundle `bundle` at the commit `sha`, by path relative
    /// to the bundle; none where the commit has no such bundle.
    fn read_bundle(&self, url: &str, sha: &str, bundle: &str) -> Result<Option<RepoTree>, Why>;

    /// The names of the bundles at the tip of the default branch (a directory
    /// holding a `bundle.yaml` is a bundle, however deep), sorted.
    fn list_bundles(&self, url: &str) -> Result<Vec<String>, Why>;

    /// Whether the default branch's tip has the bundle `bundle` (answered of
    /// that one path, never of a listing).
    fn has_bundle(&self, url: &str, bundle: &str) -> Result<bool, Why>;

    /// Commit `files` (paths in the repository, with their bytes) to the
    /// default branch as one commit with `message` and push it; the commit now
    /// at the tip (the same one where nothing changed).
    fn publish(
        &self,
        url: &str,
        files: &BTreeMap<String, Vec<u8>>,
        message: &str,
    ) -> Result<String, Why>;
}

/// Where a forge's API is and which environment variables its token may be in.
/// The token itself is never here: the adapter reads the environment.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Endpoint {
    /// The REST API's base, such as `https://api.github.com`.
    pub api_url: String,
    /// The variables the token is read from, the first one set winning; none
    /// set sends the request unauthenticated.
    pub token_vars: Vec<String>,
}

/// A repository a forge's search found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ForgeRepo {
    pub owner: String,
    pub name: String,
    pub description: String,
    pub stars: u64,
    pub url: String,
}

/// A bundle to open as a pull request: one commit with every file, on a new
/// branch of the repository, into its default branch.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PullRequest {
    pub owner: String,
    pub repo: String,
    /// The new branch's name before the adapter makes it unique.
    pub branch: String,
    /// The commit message.
    pub message: String,
    pub title: String,
    pub body: String,
    /// Paths in the repository, with their bytes.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// What opening a pull request did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Opened {
    /// The pull request's page.
    pub url: String,
    /// The commit on the new branch.
    pub commit: String,
    /// The branch it was made on.
    pub branch: String,
}

/// A forge's API.
pub trait Forge {
    /// The repositories named `ctxloom` or `ctxloom-*` that match `query`
    /// (none for all), most starred first, at most `limit`.
    fn search_repositories(
        &self,
        endpoint: &Endpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ForgeRepo>, Why>;

    /// Put `request`'s files on a new branch as one commit and open a pull
    /// request from it into the repository's default branch.
    fn open_pull_request(&self, endpoint: &Endpoint, request: &PullRequest) -> Result<Opened, Why>;
}

/// The project's files this feature owns.
pub trait DepsFiles {
    /// Where `remotes.yaml` is, as the user is shown it.
    fn registry_path(&self) -> String;

    /// The registry's text; none where there is no file.
    fn read_registry(&self) -> Result<Option<String>, Why>;

    /// Write the registry (atomically), making its directory if need be.
    fn write_registry(&self, text: &str) -> Result<(), Why>;

    /// Where `lock.yaml` is, as the user is shown it.
    fn lock_path(&self) -> String;

    /// The lock's text; none where there is no file.
    fn read_lock(&self) -> Result<Option<String>, Why>;

    /// Write the lock (atomically).
    fn write_lock(&self, text: &str) -> Result<(), Why>;

    /// Whether the bundle `identity` is installed at `sha`.
    fn is_installed(&self, identity: &str, sha: &str) -> bool;

    /// Install the tree of `identity` at `sha`, replacing whatever was
    /// installed for the identity.
    fn install(&self, identity: &str, sha: &str, tree: &RepoTree) -> Result<(), Why>;

    /// Where the tree installed for `identity` at `sha` is, as shown.
    fn installed_path(&self, identity: &str, sha: &str) -> String;

    /// The tree installed for `identity` at `sha`; none where it is not.
    fn read_installed(&self, identity: &str, sha: &str) -> Result<Option<RepoTree>, Why>;

    /// Remove every installed tree not in `keep` (an identity at a commit);
    /// the identities whose trees went.
    fn prune(&self, keep: &[(String, String)]) -> Result<Vec<String>, Why>;

    /// Where the authored bundle `name`'s `bundle.yaml` is, as the user is
    /// shown it; none where there is no such bundle.
    fn local_bundle(&self, name: &str) -> Result<Option<String>, Why>;

    /// The authored bundle `name`'s files, by path relative to it.
    fn read_local_bundle(&self, name: &str) -> Result<BTreeMap<String, Vec<u8>>, Why>;

    /// Remove the authored bundle `name`, all of its tree.
    fn remove_local_bundle(&self, name: &str) -> Result<(), Why>;

    /// Whether `path` is a directory.
    fn is_directory(&self, path: &str) -> bool;

    /// Whether `path` exists.
    fn exists(&self, path: &str) -> bool;

    /// Put `files` into the bundles directory of the project (or `.ctxloom`)
    /// at `dir` — or `dir` itself where it is neither — as the bundle `name`;
    /// where they landed, as shown (the directory as given, joined).
    fn copy_bundle(
        &self,
        dir: &str,
        name: &str,
        files: &BTreeMap<String, Vec<u8>>,
        force: bool,
    ) -> Result<CopyOutcome, Why>;
}

/// What copying a bundle into another place did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CopyOutcome {
    /// It landed here.
    Copied(String),
    /// A bundle of that name is there already (and `force` was not given).
    Exists(String),
    /// The destination is the bundle's own directory.
    Itself(String),
}

/// A bundle installed from a remote, at its pin.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InstalledBundle {
    /// The version-less canonical reference.
    pub reference: String,
    /// The name it is known by: `<remote>/<bundle>` where a remote is
    /// registered for its repository, else the reference.
    pub name: String,
    /// The repository it is from (its address).
    pub url: String,
    /// The commit it is pinned at.
    pub sha: String,
    /// Where its installed tree is, as shown.
    pub path: String,
    /// Its tree.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The files of it committed executable.
    pub executable: std::collections::BTreeSet<String>,
}

/// A bundle a remote publishes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemoteHit {
    /// The registered remote's name.
    pub remote: String,
    pub name: String,
    pub tags: Vec<String>,
    pub description: String,
    pub author: String,
    /// The reference that installs it.
    pub pull_ref: String,
}

/// What a search of the remotes found.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct RemoteSearch {
    pub hits: Vec<RemoteHit>,
    /// What could not be searched, and why.
    pub warnings: Vec<String>,
}

/// What the library reads of the remotes.
pub trait RemoteLibrary {
    /// Every bundle installed from a remote that is locked and whose tree is
    /// there.
    fn installed(&self) -> Result<Vec<InstalledBundle>, Failure>;

    /// `reference` with a registered remote's name spelled out
    /// (`origin/demo#profiles/dev` as the canonical reference it stands for);
    /// any other reference as it is. `local_exists` tells a project bundle
    /// of that very name, which wins.
    fn canonical(&self, reference: &str, local_exists: &dyn Fn(&str) -> bool) -> String;

    /// `reference`, read out of the profile of a bundle of the repository at
    /// `url`, at the commit `sha`: a short sibling reference is that
    /// repository's, at that commit; any other as it is.
    fn within(&self, reference: &str, url: &str, sha: &str) -> String;

    /// The bundles every registered remote publishes whose name or
    /// description holds `query`.
    fn search(&self, query: &str) -> Result<RemoteSearch, Failure>;
}
