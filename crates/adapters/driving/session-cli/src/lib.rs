//! The `session` commands (the driving adapter): arguments parsed into use
//! cases, answers rendered as json or text. It sees only the application
//! layer ([`SessionService`]).
//!
//! ```text
//! ctxloom session                          (bare: lists)
//! ctxloom session list [--all] [--state live|ended|unknown]
//! ctxloom session search <word>... [--all] [--state ...]
//! ctxloom session show <harp>
//! ctxloom session edit <harp> --name <new>
//! ctxloom session keep <harp> [--undo]
//! ctxloom session remove <harp> [--yes] [--even-if-live]
//! ctxloom session transcript               (bare: lists)
//! ctxloom session transcript list [<harp>] [--all]
//! ctxloom session transcript show <harp>
//! ```
//!
//! Go's `session purge`, `sweep`, `adopt`, `approvals`, `worktrees` and
//! `transcript purge` arrive with S10 to S12; `session watch` waits on the
//! owner's ruling on the watch lines. The old `rename` verb is gone, not
//! hidden.
//!
//! JSON rows: `harp, summary, state, start, end?, purged?, kept?,
//! transcript_path?, engine` (D16: Go 0.7's `harp, summary, state, start, end`
//! plus the plan's additions; the keys that are empty are left out as Go
//! leaves them).

mod error;
mod render;
mod run;

use clap::{Args, Subcommand};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{CliFault, EN_US};
pub use run::{run, Done, Failure};

/// The `session` group.
#[derive(Args, Debug)]
pub struct Session {
    #[command(subcommand)]
    pub verb: Option<SessionVerb>,
}

#[derive(Subcommand, Debug)]
pub enum SessionVerb {
    /// List recorded sessions (default: current project; --all for everything)
    #[command(
        after_help = "Examples:\n  ctxloom session list\n  ctxloom session list --all\n  ctxloom session list --state live"
    )]
    List(ListArgs),
    /// Search sessions by name, title and times (default: current project; --all for everything)
    #[command(
        after_help = "Examples:\n  ctxloom session search lockfile migration\n  ctxloom session search --state ended lockfile"
    )]
    Search(SearchArgs),
    /// Show a named session: its title, state, engine and times
    #[command(after_help = "Examples:\n  ctxloom session show amber-swift-owl")]
    Show(HarpArgs),
    /// Assign a recorded session's fields (today: --name, which renames the session)
    #[command(
        after_help = "Examples:\n  ctxloom session edit amber-swift-owl --name release-notes"
    )]
    Edit(EditArgs),
    /// Keep a session's transcript at any age (--undo releases it)
    #[command(
        after_help = "Examples:\n  ctxloom session keep amber-swift-owl\n  ctxloom session keep amber-swift-owl --undo"
    )]
    Keep(KeepArgs),
    /// Remove a session entirely: its index entry and its transcript
    #[command(after_help = "Examples:\n  ctxloom session remove amber-swift-owl --yes")]
    Remove(RemoveArgs),
    /// The recorded conversation behind a session: list it, show it
    Transcript(Transcript),
    /// Import Go ctxloom's sessions (~/.ctxloom/sessions) into each project's store, once
    #[command(after_help = "Examples:\n  ctxloom session import")]
    Import,
}

#[derive(Args, Debug, Default)]
pub struct ListArgs {
    /// Include sessions from every project (default: filter to the current one)
    #[arg(long)]
    pub all: bool,
    /// Only sessions in this state, from the session's liveness lock: live, ended, unknown
    #[arg(long, value_name = "STATE")]
    pub state: Option<String>,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Words that must all match, case-insensitively, the session's name, title or times
    #[arg(required = true, value_name = "WORD")]
    pub words: Vec<String>,
    /// Search sessions from every project (default: the current one)
    #[arg(long)]
    pub all: bool,
    /// Only sessions in this state, from the session's liveness lock: live, ended, unknown
    #[arg(long, value_name = "STATE")]
    pub state: Option<String>,
}

#[derive(Args, Debug)]
pub struct HarpArgs {
    /// The session's name
    #[arg(value_name = "SESSION-NAME")]
    pub harp: String,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    /// The session's name
    #[arg(value_name = "SESSION-NAME")]
    pub harp: String,
    /// Rename the session to this name. The engine's transcript is unaffected.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct KeepArgs {
    /// The session's name
    #[arg(value_name = "SESSION-NAME")]
    pub harp: String,
    /// Release the mark: the transcript becomes reclaimable once its retention has passed
    #[arg(long)]
    pub undo: bool,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// The session's name
    #[arg(value_name = "SESSION-NAME")]
    pub harp: String,
    /// Apply the plan this invocation printed (default: report only)
    #[arg(short = 'y', long)]
    pub yes: bool,
    /// Permit destroying a session whose owner may still be running: its lock is held, or it has none
    #[arg(long)]
    pub even_if_live: bool,
}

/// The `session transcript` group.
#[derive(Args, Debug)]
pub struct Transcript {
    #[command(subcommand)]
    pub verb: Option<TranscriptVerb>,
}

#[derive(Subcommand, Debug)]
pub enum TranscriptVerb {
    /// List which sessions have a captured transcript, and how large each one is
    List(TranscriptListArgs),
    /// Print a session's whole transcript, then exit
    Show(HarpArgs),
}

#[derive(Args, Debug, Default)]
pub struct TranscriptListArgs {
    /// Restrict the listing to this session
    #[arg(value_name = "SESSION-NAME")]
    pub harp: Option<String>,
    /// Include sessions from every project (default: the current one)
    #[arg(long)]
    pub all: bool,
}
