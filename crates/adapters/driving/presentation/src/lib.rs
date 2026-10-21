//! Presentation at the driving edge, for any domain: a command line, an MCP
//! server. The core hands up typed values; this crate turns them into one
//! of two formats, [`Format::Json`] for jq and other programs and
//! [`Format::Text`] (UTF-8) for a person, and renders diagnostics from
//! Fluent catalogs.
//!
//! It is domain-free (no ctxloom crate but `annals-diagnostic`; its tests
//! hold it to that), so any binary reuses it. A domain's shapes and views
//! live with that domain's driving adapter and implement [`Render`].
//!
//! - [`Format`]: which format, from flags and whether stdout is a terminal.
//! - [`render`] and [`write_json`]: one JSON document per call, fields in a
//!   fixed order; [`write_json_line`], one line of a JSON Lines stream.
//! - [`Catalog`]: a diagnostic's message from each crate's embedded en-US
//!   catalog, and its JSON [`Envelope`] with nested causes.
//! - [`text`]: display-width columns and headlines.
//!
//! Output goes to an injected `&mut dyn Write`, never straight to stdout.

mod catalog;
mod envelope;
mod error;
mod format;
pub mod text;

use std::io::{self, Write};

use serde::Serialize;

pub use catalog::{Catalog, Source};
pub use envelope::Envelope;
pub use error::{diagnostic_samples, CatalogError, UnknownFormat, Unrendered, EN_US};
pub use format::Format;

/// A value a driving adapter answers with, in both formats.
pub trait Render {
    /// The value's JSON shape: stable field names in a fixed order, absent
    /// values explicit (`[]`, `""`, `null`), so a jq filter holds between
    /// calls.
    fn json(&self) -> impl Serialize + '_;

    /// The value as a person reads it, in UTF-8.
    fn text(&self, out: &mut dyn Write) -> io::Result<()>;
}

/// Write `view` to `out` in `format`.
pub fn render(out: &mut dyn Write, format: Format, view: &impl Render) -> io::Result<()> {
    match format {
        Format::Text => view.text(out),
        Format::Json => write_json(out, &view.json()),
    }
}

/// `value` as one indented JSON document and a newline.
pub fn write_json<T: Serialize + ?Sized>(out: &mut dyn Write, value: &T) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)
}

/// `value` as one line of a JSON Lines stream (one compact document and a
/// newline), flushed, so a reader following the stream sees it at once.
pub fn write_json_line<T: Serialize + ?Sized>(out: &mut dyn Write, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    writeln!(out)?;
    out.flush()
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
