//! The seam through which another crate's commands join the command line:
//! the library commands (S1) are one, the sessions' another. This crate parses
//! the whole line (so help, usage errors and the exit ladder are one
//! behaviour) and hands the extension's noun its matches; the extension
//! answers, and its errors are reported like any other.

use std::io::Write;

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{Catalog, Format};
use clap::{ArgMatches, Command};

/// One command, as an extension is asked to run it.
pub struct Call<'a> {
    /// The matches of the whole command line: the extension reads its noun's
    /// arguments from them.
    pub matches: &'a ArgMatches,
    /// The `--format` the line gave, if it gave one.
    pub explicit_format: Option<Format>,
    /// What the terminal gives when none was asked for: text on a terminal,
    /// json off one.
    pub default_format: Format,
    /// Whether stdout is a terminal.
    pub stdout_is_terminal: bool,
    /// `--degraded`: fatal findings are warnings.
    pub degraded: bool,
    /// `--no-companions`.
    pub no_companions: bool,
    /// Every `--config-set <path>=<value>`.
    pub config_set: Vec<String>,
}

impl Call<'_> {
    /// The format to answer in: the one asked for, else the terminal's.
    pub fn format(&self) -> Format {
        self.explicit_format.unwrap_or(self.default_format)
    }
}

/// Why an extension's command did not complete: its own diagnostic, and the
/// exit code (1 failed, 2 refused before any state was read, 3 fatal findings).
#[derive(Debug)]
pub struct Failure {
    pub cause: Box<dyn Diagnostic>,
    pub exit: u8,
}

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
