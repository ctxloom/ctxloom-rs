//! The library's write side as pure planning (S2; D4: the library is
//! files-as-truth, so a write is the text of the files it leaves). Nothing here
//! does I/O: the application hands text and bytes in, takes text and bytes
//! out, and a driven adapter puts them on disk.
//!
//! - [`section`]: a config write is a section merge with sorted keys, and
//!   never heals a corrupt file.
//! - [`item`], [`envelope`], [`mcp`], [`skill`]: the files of a bundle's tree,
//!   in the bytes Go writes them.
//! - [`profile`]: a profile document, made and changed in place.
//! - [`scaffold`]: what `config create` leaves, and the registry's engines.
//! - [`archive`]: a skill package as a zip, packed and unpacked (adversarial
//!   input).
//! - [`loadout`]: ctxloom's own loadout document.

pub mod archive;
pub mod envelope;
pub mod item;
pub mod loadout;
pub mod mcp;
pub mod profile;
pub mod scaffold;
pub mod section;
pub mod skill;
mod yamledit;
