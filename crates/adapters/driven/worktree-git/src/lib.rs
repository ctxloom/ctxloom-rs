//! The `Workspaces` port over the git command line (S11). Go:
//! `isolation/worktree.go`, `worktree_reap.go` and `none.go` (release/0.7).
//!
//! The workspace axis: a run asking for a worktree gets a checkout on its own
//! named branch (`ctxloom/<agent>`), in the session's work directory
//! (`~/.ctxloom/sessions/<harp>/work/ctxloom-wt-<agent>`), never under the
//! project. The checkout's path is a pure function of the agent, so a resume
//! finds an earlier run's own checkout and re-enters it; anything else at
//! that path is a refusal. The ctxloom-written config the run puts in the
//! checkout is hidden from the project's git status by the repository's
//! shared exclude file.
//!
//! The same adapter finds the scratch checkouts a crashed run left behind and
//! removes them WIP-safely: nested worktrees first, each checked, never
//! forced, and a doubtful state is never read as clean.

mod error;
mod git;
mod workspace;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{WorktreeError, EN_US};
pub use workspace::{branch_of, checkout_name, sanitize, GitWorkspaces, EXCLUDE_HEADER};

#[cfg(test)]
#[path = "error.test.rs"]
mod error_tests;
