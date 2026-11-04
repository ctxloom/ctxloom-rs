//! The engine callbacks ctxloom installs into engines (the hidden `hook`
//! commands). Each is a machine callback, not for people, and answers to a
//! different contract: stdout carries the hook's output only, and a
//! diagnostic goes on the warning channel (`HookWarning`), never to stdout.
//!
//! - `hook session-bind`: the engine's SessionStart hook. Emits the harp
//!   self-id marker as session-start context, then binds the engine's
//!   session and transcript to the harp.
//! - `hook hud`: the statusline. Rendering only; it records nothing.
//!
//! `hook session-start`, `mail-drain`, `mail-peek`, `skill-mates` and
//! `permission` arrive with the slices that need them (S1, S9, S10). The
//! retired `turn-changed` is not a command.
//!
//! Only the claude-code codec exists yet; each engine's codec is its
//! adapter's (S9).

mod bind;
mod error;
mod hud;

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
}
