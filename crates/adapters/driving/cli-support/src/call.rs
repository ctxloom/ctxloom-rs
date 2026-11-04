//! The call a command is run with and the failure it ends in.

use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::Format;
use clap::ArgMatches;

/// One command, as an adapter is asked to run it.
pub struct Call<'a> {
    /// The matches of the whole command line: the adapter reads its noun's
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

impl<'a> Call<'a> {
    /// A call with the global flags at their defaults.
    pub fn new(
        matches: &'a ArgMatches,
        explicit_format: Option<Format>,
        default_format: Format,
    ) -> Self {
        Call {
            matches,
            explicit_format,
            default_format,
            stdout_is_terminal: false,
            degraded: false,
            no_companions: false,
            config_set: Vec::new(),
        }
    }

    /// The format to answer in: the one asked for, else the terminal's.
    pub fn format(&self) -> Format {
        self.explicit_format.unwrap_or(self.default_format)
    }
}

/// Why a command did not complete: its own diagnostic, and the exit code
/// (1 failed, 2 refused before any state was read, 3 fatal findings).
#[derive(Debug)]
pub struct Failure {
    pub cause: Box<dyn Diagnostic>,
    pub exit: u8,
}

impl Failure {
    /// `cause`, ending the command with `exit`.
    pub fn new(cause: impl Diagnostic + 'static, exit: u8) -> Failure {
        Failure {
            cause: Box::new(cause),
            exit,
        }
    }
}
