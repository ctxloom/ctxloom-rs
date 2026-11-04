//! The library's read side as an application (S1): every query over the
//! project's bundles and configuration, behind the content and config ports.
//! A command is a method of [`Library`] that answers a view (data, in the
//! shape the command prints) and the notices it raised on the way; the driving
//! adapter renders both.
//!
//! - [`Library::open`] reads the configuration (layers, overrides, the
//!   engine registry) and the bundles; a bundle that cannot be read is
//!   skipped with a notice and the listing goes on.
//! - Nothing here writes, and nothing is created: a read is a read.

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
mod yamlout;

pub use agents::ENGINES;
pub use config::Flags;
pub use config_show::ConfigShown;
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{AppError, Notice, EN_US};
pub use items::Kind as ItemKind;
pub use project::{Library, Outcome};
pub use run::{AssembleRequest, Previewed, RunRequest};
pub use search::SearchRequest;
pub use views::*;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "behaviour.test.rs"]
mod behaviour;
