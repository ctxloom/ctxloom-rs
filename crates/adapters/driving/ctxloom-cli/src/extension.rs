//! The seam through which another crate's commands join the command line:
//! the library commands (S1) are one, the sessions' another. This crate parses
//! the whole line (so help, usage errors and the exit ladder are one
//! behaviour) and hands the extension's noun its matches; the extension
//! answers, and its errors are reported like any other.

use std::io::Write;

use angzarr_presentation::Catalog;
use clap::Command;
use library_application::Flags;

pub use cli_support::Failure;

/// One command, with the global flags the library's use cases take.
pub type Call<'a> = cli_support::Call<'a, Flags>;

/// A group of nouns another crate owns.
pub trait Extension {
    /// The nouns it adds to the command, each already shaped (its verbs, its
    /// default verb, no `help` command).
    fn nouns(&self) -> Vec<Command>;

    /// Run `noun`. The exit code on success (0, or 3 when fatal findings
    /// followed an answer that was still printed).
    fn run(
        &mut self,
        noun: &str,
        call: &Call<'_>,
        catalog: &Catalog,
        out: &mut dyn Write,
        err: &mut dyn Write,
    ) -> Result<u8, Failure>;
}

/// No extension: the command line holds only this crate's own commands.
pub struct NoExtension;

impl Extension for NoExtension {
    fn nouns(&self) -> Vec<Command> {
        Vec::new()
    }

    fn run(
        &mut self,
        _noun: &str,
        _call: &Call<'_>,
        _catalog: &Catalog,
        _out: &mut dyn Write,
        _err: &mut dyn Write,
    ) -> Result<u8, Failure> {
        Ok(0)
    }
}
