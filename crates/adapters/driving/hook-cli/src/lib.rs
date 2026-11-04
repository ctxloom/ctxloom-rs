//! The engine callbacks ctxloom installs into engines (the hidden `hook`
//! commands). Each is a machine callback, not for people, and answers to a
//! different contract: stdout carries the hook's output only, and a
//! diagnostic goes on the warning channel (`HookWarning`), never to stdout.
//!
//! - `hook session-bind`: the engine's SessionStart hook. Emits the harp
//!   self-id marker as session-start context, then binds the engine's
//!   session and transcript to the harp.
//! - `hook hud`: the statusline. Rendering only; it records nothing.
//! - `hook session-start`: the engine's SessionStart hook. Answers with the
//!   session-start notices, never the project's context (S9).
//! - `hook skill-mates`: the engine's PostToolUse hook. Names the linked skills
//!   a just-completed skill leaves uninvoked this session (S9).
//!
//! `mail-drain`, `mail-peek` and `permission` arrive with the slices that need
//! them (S10). The retired `turn-changed` is not a command; `stamp-plan` is
//! taskloom's (D17).
//!
//! Each engine's codec is its adapter's (S9): `session-start` and
//! `skill-mates` reach it through the application ([`HookService`]), by the
//! engine `--engine` names. `session-bind` still reads claude's payload here.

mod bind;
mod error;
mod hud;

use angzarr_diagnostic::Diagnostic;
use clap::{Args, Subcommand};

pub use bind::{marker, session_bind, session_start_reply, Binding, Outcome, CLAUDE_CODE};
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{HookError, HookWarning, EN_US};
pub use hud::{
    bar_color, context_bar, hud, render, StatusPayload, CYAN, DIM, GREEN, RED, RESET, YELLOW,
};

/// The `hook` group: hidden, for engines.
#[derive(Args, Debug)]
pub struct Hook {
    /// The engine firing this hook; written by the hooks approach that
    /// delivered it
    #[arg(long, global = true, value_name = "ENGINE")]
    pub engine: Option<String>,
    #[command(subcommand)]
    pub verb: HookVerb,
}

#[derive(Subcommand, Debug)]
pub enum HookVerb {
    /// Bind the current engine session to the active harp (internal: the SessionStart hook)
    SessionBind,
    /// Output a formatted statusline for the active agent's HUD (internal: the statusline command)
    Hud,
    /// Deliver session-start notices to an AI tool's SessionStart hook (internal)
    SessionStart,
    /// Name a completed skill's linked skills not yet invoked (internal: the PostToolUse hook)
    SkillMates,
}

/// What the new callbacks call: the application's answer to one payload, as
/// the engine's own wire (one JSON line for stdout).
pub trait HookService {
    /// The answer to a SessionStart payload read by `engine`'s codec.
    fn session_start(&self, engine: &str, payload: &str) -> Result<String, Box<dyn Diagnostic>>;

    /// The answer to a PostToolUse payload read by `engine`'s codec.
    fn skill_mates(&self, engine: &str, payload: &str) -> Result<String, Box<dyn Diagnostic>>;
}
