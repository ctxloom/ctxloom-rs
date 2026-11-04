//! The adapter.

use std::path::PathBuf;

use isolation_ports::{Failure, Prepared, Removal, Request, Scratch, Standing, Workspaces};

/// The header of the block of ctxloom's per-agent config patterns in the
/// repository's shared `info/exclude` (Go: `gitignore.WorktreeComment`).
pub const EXCLUDE_HEADER: &str =
    "# ctxloom per-agent worktree config (isolation; NEVER merge back)";

/// An agent id as one path segment: anything outside `[A-Za-z0-9_.-]` becomes
/// `-`, the ends are trimmed of separators, and an empty result is `agent`.
pub fn sanitize(agent: &str) -> String {
    let _ = agent;
    todo!("S11 green")
}

/// The checkout directory's name for an agent: `ctxloom-wt-<agent>`.
pub fn checkout_name(agent: &str) -> String {
    let _ = agent;
    todo!("S11 green")
}

/// The branch a checkout directory is on: its name with the scratch prefix
/// swapped for the `ctxloom/` namespace. A pure function of the path, so
/// whoever holds the checkout can name the branch with no side table.
pub fn branch_of(checkout: &str) -> String {
    let _ = checkout;
    todo!("S11 green")
}

/// Workspaces under one home's sessions.
pub struct GitWorkspaces {
    sessions: PathBuf,
}

impl GitWorkspaces {
    /// The sessions under `home` (`<home>/.ctxloom/sessions`).
    pub fn new(home: impl Into<PathBuf>) -> GitWorkspaces {
        GitWorkspaces {
            sessions: home.into().join(".ctxloom").join("sessions"),
        }
    }
}

impl Workspaces for GitWorkspaces {
    fn prepare(&self, request: &Request) -> Result<Prepared, Failure> {
        let _ = (&self.sessions, request);
        todo!("S11 green")
    }

    fn session_known(&self, harp: &str) -> Result<bool, Failure> {
        let _ = harp;
        todo!("S11 green")
    }

    fn scratch(&self, harp: Option<&str>) -> Result<Vec<Scratch>, Failure> {
        let _ = harp;
        todo!("S11 green")
    }

    fn inspect(&self, path: &str) -> Standing {
        let _ = path;
        todo!("S11 green")
    }

    fn remove(&self, repo_dir: &str, path: &str) -> Removal {
        let _ = (repo_dir, path);
        todo!("S11 green")
    }
}
