//! The library's read side as an application (S1): every query over the
//! project's bundles and configuration, behind the content and config ports.
//! A command is a method of [`Library`] that answers a view (data, in the
//! shape the command prints) and the notices it raised on the way; the driving
//! adapter renders both.
//!
//! - [`Library::open`] reads the configuration (layers, overrides, the
//!   engine registry) and the bundles; a bundle that cannot be read is
//!   skipped with a notice and the listing goes on.
//! - A read is a read: nothing is written or created. The write side (S2,
//!   [`write`]) plans a change, checks that what it leaves reads, and only
//!   then hands it to the ports; a refused write has written nothing, and a
//!   preview or an edit with nothing to change has written nothing either.

mod agents;
mod bundles;
mod config;
mod config_show;
mod error;
/// In-memory ports, for this crate's tests and a driving adapter's.
#[cfg(any(test, feature = "test-support"))]
pub mod fakes;
mod items;
mod json;
mod profiles;
mod project;
mod run;
mod search;
mod views;
pub mod write;
mod yamlout;

pub use agents::ENGINES;
pub use config::Flags;
pub use config_show::ConfigShown;
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{AppError, Notice, EN_US};
pub use items::Kind as ItemKind;
#[allow(unused_imports)]
pub use library::write::loadout::DOCUMENT as LOADOUT;
pub use library::write::mcp::McpPatch;
pub use library::write::profile::ProfileEdit;
pub use project::{Library, Outcome};
pub use run::{AssembleRequest, Previewed, RunRequest};
pub use search::SearchRequest;
pub use views::*;
pub use write::requests::*;
pub use write::views::*;
pub use write::{WriteError, WritePorts};

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "behaviour.test.rs"]
mod behaviour;

#[cfg(test)]
#[path = "remotes.test.rs"]
mod remotes;
