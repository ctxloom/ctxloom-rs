//! A warning said on the error stream beside an answer.

use std::io::Write;

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{write_json, Catalog, Format};

use crate::PROG;

/// Say `notice` on `err` in `format` and carry on: a warning, not an error.
pub fn say_warning(
    notice: &dyn Diagnostic,
    format: Format,
    catalog: &Catalog,
    err: &mut dyn Write,
) {
    let _ = match format {
        Format::Json => write_json(err, &catalog.envelope(notice)),
        Format::Text => writeln!(err, "{PROG}: {}", catalog.message(notice)),
    };
}
