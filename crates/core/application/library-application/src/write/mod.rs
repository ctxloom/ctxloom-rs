//! The library's write side as an application (S2): create, edit and remove
//! for bundles, items, profiles, agents, llm labels, MCP servers, skills and
//! companions; `config create` and `config edit`; export and import; the
//! editor round trips. Files are the truth (D4): a command plans the files it
//! leaves, checks that they read, and hands them to the ports.
//!
//! - A preview (a remove without `--yes`) and an edit with nothing to change
//!   call no port that writes.
//! - A refused write has written nothing: every check is made on the plan,
//!   before the first change goes out.
//! - A config write is a section merge with sorted keys and never heals a
//!   corrupt file ([`library::write::section`]).
//! - An import over what is there refuses, and the local edit survives,
//!   unless `--force`.

use std::sync::Arc;

use ctxloom_ports::{Editor, LoadoutProbe, Scaffold, UserFiles};

mod agents;
mod bundles;
mod companions;
mod configfile;
mod configs;
mod editor;
mod engines;
pub(crate) mod error;
mod items;
mod llms;
mod mcp;
mod plan;
mod profiles;
pub mod requests;
mod skills;
pub mod views;

pub use error::WriteError;

/// The ports the write side needs beyond the library's: the user's own files
/// (where export writes and import reads), the scaffold of a new project, the
/// editor, and the probe a companion is asked to answer.
#[derive(Clone)]
pub struct WritePorts {
    pub user: Arc<dyn UserFiles>,
    pub scaffold: Arc<dyn Scaffold>,
    pub editor: Arc<dyn Editor>,
    pub probe: Arc<dyn LoadoutProbe>,
}
