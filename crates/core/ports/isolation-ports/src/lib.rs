//! The ports isolation needs from outside. Each is a trait a driven adapter
//! implements; nothing here does I/O.
//!
//! - [`Workspaces`]: prepare, list and reclaim the workspace axis's
//!   checkouts (`worktree-git`). A prepared workspace is a git worktree on
//!   the agent's own branch, in the session's work directory; the scratch
//!   checkouts a crashed run left behind are found, inspected and removed
//!   through it.
//! - [`ContainerRuntimes`]: the runtime axis's facts about this machine
//!   (`container-runtime`): whether this process is inside a container, which
//!   runtimes are installed and reachable.
//! - [`Backends`]: the engines `container` may be asked about.
//! - [`Companions`]: the tooling the registered companions declare.

use angzarr_diagnostic::Diagnostic;

/// Why an adapter could not do what it was asked: its own diagnostic.
pub type Failure = Box<dyn Diagnostic>;

/// What a workspace is asked for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Request {
    /// The project's directory (a git repository, or not).
    pub project_dir: String,
    /// The session's work directory: where the checkout lives, never under
    /// the project.
    pub work_dir: String,
    /// The agent the checkout is for; it names the checkout and its branch.
    pub agent: String,
    /// The paths ctxloom writes into a working tree, hidden from the
    /// project's git status by its shared exclude file.
    pub artifact_patterns: Vec<String>,
}

/// A checkout prepared for an agent.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Checkout {
    pub path: String,
    /// The agent's own branch (`ctxloom/<agent>`).
    pub branch: String,
    /// The repository it was cut from.
    pub repo_dir: String,
    /// Whether this is an earlier run's own checkout, re-entered in place.
    pub reused: bool,
}

/// What preparing a workspace came to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Prepared {
    Checkout(Checkout),
    /// No worktree could be had (the project is not a git repository, or the
    /// add failed): the run goes ahead in the live directory, and the reason
    /// is a warning. A worktree is a convenience, not a boundary.
    Unavailable(String),
}

/// One scratch checkout under a session's work directory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scratch {
    pub harp: String,
    pub path: String,
}

/// Whether a checkout holds anything worth keeping.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Safety {
    Clean,
    /// Uncommitted changes, gitignored files, or a state git would not
    /// answer for: why. An unreadable state is never read as clean.
    Unsafe(String),
}

/// What a checkout is, asked of git without changing anything.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Standing {
    /// The repository it belongs to could not be resolved: why.
    Unresolved(String),
    Resolved {
        repo_dir: String,
        safety: Safety,
    },
}

/// What removing a checkout came to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Removal {
    Removed,
    /// Left in place, and why. Removal is never forced.
    Left(String),
}

/// The workspace axis.
pub trait Workspaces {
    /// A worktree for `request.agent` on its own branch in `request.work_dir`,
    /// or the reason none is available. An earlier run's own checkout at the
    /// agent's path is re-entered; anything else at that path is a refusal
    /// (the `Err`), never a degrade and never adopted.
    fn prepare(&self, request: &Request) -> Result<Prepared, Failure>;

    /// Whether `harp` has a session directory at all.
    fn session_known(&self, harp: &str) -> Result<bool, Failure>;

    /// The scratch checkouts under the sessions' work directories: one
    /// session's, or every session's.
    fn scratch(&self, harp: Option<&str>) -> Result<Vec<Scratch>, Failure>;

    /// What `path` is, changing nothing.
    fn inspect(&self, path: &str) -> Standing;

    /// Remove the checkout WIP-safely (nested worktrees first, each checked;
    /// never forced), and say whether it is gone.
    fn remove(&self, repo_dir: &str, path: &str) -> Removal;
}

/// A container runtime installed on this machine.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RuntimeFacts {
    /// `docker` or `podman`.
    pub name: String,
    /// Whether its daemon answered.
    pub reachable: bool,
}

/// The names ctxloom knows container runtimes by.
pub const RUNTIMES: [&str; 2] = ["docker", "podman"];

/// The runtime axis's facts about this machine.
pub trait ContainerRuntimes {
    /// Why this process looks to be inside a container (the markers that
    /// matched); empty when it is not.
    fn in_container(&self) -> Vec<String>;

    /// The runtimes installed, in preference order, each probed.
    fn installed(&self) -> Vec<RuntimeFacts>;

    /// Whether `image` exists locally under `runtime`.
    fn image_present(&self, runtime: &str, image: &str) -> bool;
}

/// The engines a `container` command may be asked about.
pub trait Backends {
    /// Every engine's name.
    fn names(&self) -> Vec<String>;

    /// The engine a bare command means: the configured default, else the
    /// first engine. None when there are no engines.
    fn default_backend(&self) -> Option<String>;
}

/// One companion's declaration of the tools its content needs in the agent
/// image.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tooling {
    /// The companion's ref, so every proposed change traces to it.
    pub source: String,
    pub content: String,
}

/// The registered companions' declarations. Only a REGISTERED companion is
/// ever run; one nobody registered declares nothing.
pub trait Companions {
    fn tooling(&self) -> Result<Vec<Tooling>, Failure>;
}
