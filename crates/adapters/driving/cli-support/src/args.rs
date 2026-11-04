//! Reading arguments and writing answers.

use std::io::{self, Write};

use angzarr_presentation::{write_json, Format};
use clap::ArgMatches;
use serde::Serialize;

/// Write `value` as json, or `text` as words, by `format`.
pub fn emit<T: Serialize>(
    format: Format,
    out: &mut dyn Write,
    value: &T,
    text: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    match format {
        Format::Json => write_json(out, value),
        Format::Text => text(out),
    }
}

/// Whether the flag `name` was given (false for a name the command lacks).
pub fn flag(matches: &ArgMatches, name: &str) -> bool {
    matches
        .try_get_one::<bool>(name)
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}

/// The one value of `name`, if it has one.
pub fn one(matches: &ArgMatches, name: &str) -> Option<String> {
    matches.try_get_one::<String>(name).ok().flatten().cloned()
}

/// Every value of `name`.
pub fn many(matches: &ArgMatches, name: &str) -> Vec<String> {
    matches
        .try_get_many::<String>(name)
        .ok()
        .flatten()
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}
