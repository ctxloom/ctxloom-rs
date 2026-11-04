//! The dependency read model (S3): what a project draws from other
//! repositories, and the decisions over it. Pure: the adapters hand it text.
//!
//! - [`address`]: a repository's address, its identity and where its clone
//!   is cached.
//! - [`reference`]: a bundle named in a profile, taken apart.
//! - [`short`]: `origin/demo` spelled out as the reference it stands for.
//! - [`registry`]: `remotes.yaml`, the sources content may come from.
//! - [`lock`]: `lock.yaml`, every dependency pinned to a commit.
//! - [`constraint`]: what a reference asks of a repository's versions, and
//!   the commit that satisfies it.
//! - [`forge`]: which forge a remote is on, how its API is reached and where
//!   its token comes from.
//! - [`version`]: semver where a version parses as semver, and what a move
//!   between two versions is worth: breaking, compatible or unknown.
//! - [`plan`]: the closure's pins, what a pull keeps, what an upgrade moves,
//!   and what upstream's absence proves.

pub mod address;
pub mod constraint;
mod error;
pub mod forge;
pub mod lock;
pub mod plan;
pub mod reference;
pub mod registry;
pub mod short;
pub mod version;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{DepsError, EN_US};
