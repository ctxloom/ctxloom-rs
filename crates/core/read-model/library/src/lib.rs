//! The library read model (S1; D4: the library is files-as-truth, so reading
//! it is a pure projection of the parsed files). Nothing here does I/O: the
//! adapters hand it text and bytes.
//!
//! - [`refs`]: the reference grammar.
//! - [`model`]: a bundle and its items.
//! - [`document`] and [`tree`]: a bundle read from an inline document, and
//!   from its tree of files; [`frontmatter`] splits an item file.
//! - [`loadout`]: a companion's loadout document, its `run:` bundle and
//!   typed `init:` section.
//! - [`profile`]: profiles, and their resolution with parents.
//! - [`catalog`]: the project's bundles and the lookups over them.
//! - [`compose`]: selection (bookend, priority, dedupe) and assembly of the
//!   context; [`mustache`] substitutes the profile's variables.
//! - [`premise`]: the premise index (D24); [`tokens`]: the token estimate;
//!   [`sha256`]: the digests.
//! - [`config`]: the layered configuration, its schema, overrides and output.
//! - [`findings`]: what a read noticed and did not stop for.
//! - [`write`]: the write side as pure planning (S2): the text of the files a
//!   command leaves.

pub mod catalog;
pub mod compose;
pub mod config;
pub mod document;
mod error;
pub mod findings;
pub mod frontmatter;
pub mod loadout;
pub mod model;
pub mod mustache;
pub mod premise;
pub mod profile;
pub mod refs;
pub mod sha256;
#[cfg(test)]
mod testdata;
pub mod tokens;
pub mod tree;
pub mod write;
mod yamlx;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{BundleError, CatalogError, ConfigError, ProfileError, RefError, EN_US};
