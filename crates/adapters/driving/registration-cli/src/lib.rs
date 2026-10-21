//! `manage install|uninstall|check`, Go taskloom's flags and words: the
//! taskloom MCP server written into agents' configurations, taken out,
//! and found. Notices go to stderr as Go's do (text and JSON alike); in
//! JSON stdout carries one document, in text `check`'s table alone.
//!
//! Go's `manage install --project` is a switch (the project-scoped file
//! under `--dir`) where every other command's `--project` names a project
//! id. So the binary mounts `manage` with its words unparsed ([`Group`])
//! and this crate reads them with its own parser, which knows `--format`
//! and `--json` too.

mod error;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use annals_diagnostic::Diagnostic;
use clap::{Parser, Subcommand};
use presentation::{Catalog, Format, Render};
use registration_application::{
    Checked, Install, Installed, Outcome, Registrations, State, Uninstall,
};
use serde::Serialize;

pub use error::{diagnostic_samples, ManageError, ManageNotice, EN_US};
pub use registration_application::{AgentConfigs, Scope};

/// The command, its words left for [`run`] to read.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Group {
    /// Register the taskloom MCP server with agent backends
    #[command(disable_help_flag = true)]
    Manage {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        words: Vec<String>,
    },
}

#[derive(Parser, Debug)]
#[command(
    name = "taskloom manage",
    about = "Register the taskloom MCP server with agent backends"
)]
struct ManageCli {
    /// Output format: text, or json.
    #[arg(long, global = true)]
    format: Option<Format>,
    /// Shorthand for --format json.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Add the taskloom MCP server to backend configs
    #[command(
        long_about = "Register `taskloom mcp` as an MCP server. By default every backend\n\
present at the chosen scope is updated (user-level: each engine's config under\n\
your home directory).\n\
Name one with --engine to register just that backend \u{2014} creating its config\n\
if needed. --project writes the project-scoped config under --dir instead of\n\
the user-level one."
    )]
    Install {
        #[command(flatten)]
        target: Target,
        /// Print the merged configs to stderr instead of writing them
        #[arg(long)]
        print_only: bool,
    },
    /// Remove the taskloom MCP server from backend configs
    Uninstall {
        #[command(flatten)]
        target: Target,
    },
    /// Report where the taskloom MCP server is registered
    Check {
        /// Project directory for project-scoped configs
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
}

#[derive(clap::Args, Debug)]
struct Target {
    /// Backend to target: claude-code (default: all present)
    #[arg(long)]
    engine: Option<String>,
    /// Write the project-scoped config under --dir instead of the user-level one
    #[arg(long)]
    project: bool,
    /// Project directory for project-scoped configs
    #[arg(long, default_value = ".")]
    dir: PathBuf,
}

impl Target {
    fn scope(&self) -> Scope {
        if self.project {
            Scope::Project(self.dir.clone())
        } else {
            Scope::User
        }
    }
}

/// What `manage` is run with besides its words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    /// The format the global flags chose; `manage`'s own override it.
    pub format: Format,
    pub stdout_is_terminal: bool,
    /// The program's name: every diagnostic's prefix.
    pub prog: &'static str,
}

/// Run `manage`'s `words` against `registrations`, writing to `out` and
/// `err` with messages from `catalog`; the exit code.
pub fn run<A: AgentConfigs>(
    words: Vec<String>,
    settings: &Settings,
    registrations: &mut Registrations<A>,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let argv = std::iter::once(OsString::from("taskloom manage"))
        .chain(words.into_iter().map(OsString::from));
    let cli = match ManageCli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(e) if e.use_stderr() => {
            let usage = ManageError::Usage {
                detail: e.render().to_string().trim_end().to_string(),
            };
            let _ = report(&usage, settings.format, settings.prog, catalog, err);
            return 1;
        }
        Err(e) => {
            let _ = write!(out, "{}", e.render());
            return 0;
        }
    };
    let format = match (cli.json, cli.format) {
        (false, None) => settings.format,
        (json, asked) => Format::choose(json, asked, settings.stdout_is_terminal),
    };
    let speaker = Speaker {
        prog: settings.prog,
        catalog,
    };
    let result = answer(cli.command, format, registrations, &speaker, out, err);
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = report(&error, format, settings.prog, catalog, err);
            1
        }
    }
}

/// Says notices on stderr: a first line after the program's name, those
/// under it indented.
struct Speaker<'a> {
    prog: &'static str,
    catalog: &'a Catalog,
}

impl Speaker<'_> {
    fn say(&self, err: &mut dyn Write, notice: &ManageNotice) -> io::Result<()> {
        let message = self.catalog.message(notice as &dyn Diagnostic);
        writeln!(err, "{}: {message}", self.prog)
    }

    fn under(&self, err: &mut dyn Write, notice: &ManageNotice) -> io::Result<()> {
        let message = self.catalog.message(notice as &dyn Diagnostic);
        writeln!(err, "  {message}")
    }
}

fn answer<A: AgentConfigs>(
    command: Command,
    format: Format,
    registrations: &mut Registrations<A>,
    speaker: &Speaker,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), ManageError> {
    let command_name = registrations.server.command.clone();
    match command {
        Command::Install { target, print_only } => {
            let request = Install {
                engine: target.engine.clone(),
                scope: target.scope(),
                print_only,
            };
            let installed = registrations.install(&request).map_err(ManageError::App)?;
            installed_notices(&installed, &command_name, print_only, speaker, err)
                .map_err(output_failed)?;
            json_only(format, out, &Written::install(&installed, print_only))
        }
        Command::Uninstall { target } => {
            let request = Uninstall {
                engine: target.engine.clone(),
                scope: target.scope(),
            };
            let removed = registrations
                .uninstall(&request)
                .map_err(ManageError::App)?;
            removed_notices(&removed, speaker, err).map_err(output_failed)?;
            json_only(format, out, &Written::uninstall(&removed))
        }
        Command::Check { dir } => {
            let checked = registrations.check(&dir);
            let view = Found {
                checked: &checked,
                command: &command_name,
                catalog: speaker.catalog,
            };
            presentation::render(out, format, &view).map_err(output_failed)
        }
    }
}

fn installed_notices(
    installed: &Installed,
    command: &str,
    print_only: bool,
    speaker: &Speaker,
    err: &mut dyn Write,
) -> io::Result<()> {
    if !installed.resolvable {
        let command = command.to_string();
        speaker.say(
            err,
            &ManageNotice::Unresolvable {
                command: command.clone(),
            },
        )?;
        speaker.under(err, &ManageNotice::UnresolvableHint { command })?;
    }
    for outcome in &installed.outcomes {
        let path = outcome.config.display().to_string();
        if print_only {
            write!(
                err,
                "# {} \u{2192} {path}\n{}",
                outcome.engine, outcome.rewritten.after
            )?;
            continue;
        }
        speaker.say(
            err,
            &ManageNotice::Registered {
                engine: outcome.engine.clone(),
            },
        )?;
        speaker.under(err, &ManageNotice::Config { path })?;
    }
    Ok(())
}

fn removed_notices(removed: &[Outcome], speaker: &Speaker, err: &mut dyn Write) -> io::Result<()> {
    if removed.is_empty() {
        return speaker.say(err, &ManageNotice::NothingToRemove);
    }
    for outcome in removed {
        let engine = outcome.engine.clone();
        let notice = if outcome.rewritten.changed {
            ManageNotice::Removed { engine }
        } else {
            ManageNotice::NotRegistered { engine }
        };
        speaker.say(err, &notice)?;
        speaker.under(
            err,
            &ManageNotice::Config {
                path: outcome.config.display().to_string(),
            },
        )?;
    }
    Ok(())
}

/// In JSON, `view` on stdout; in text, stdout stays empty, as Go's.
fn json_only(
    format: Format,
    out: &mut dyn Write,
    view: &impl Serialize,
) -> Result<(), ManageError> {
    match format {
        Format::Json => presentation::write_json(out, view).map_err(output_failed),
        Format::Text => Ok(()),
    }
}

/// An install's or uninstall's answer in JSON.
#[derive(Serialize, Debug, PartialEq)]
pub struct Written {
    /// Whether the server's command resolves on this PATH; null for an
    /// uninstall, which does not ask.
    pub command_resolves: Option<bool>,
    pub configs: Vec<ConfigJson>,
}

/// One configuration as a registration left it.
#[derive(Serialize, Debug, PartialEq)]
pub struct ConfigJson {
    pub engine: String,
    pub config: String,
    /// Whether its bytes change.
    pub changed: bool,
    /// Whether they were written (not for `--print-only`, nor when they
    /// do not change).
    pub written: bool,
    /// The file as written, or as it would be.
    pub after: String,
}

impl Written {
    fn configs(outcomes: &[Outcome], dry_run: bool) -> Vec<ConfigJson> {
        outcomes
            .iter()
            .map(|o| ConfigJson {
                engine: o.engine.clone(),
                config: o.config.display().to_string(),
                changed: o.rewritten.changed,
                written: o.rewritten.changed && !dry_run,
                after: o.rewritten.after.clone(),
            })
            .collect()
    }

    fn install(installed: &Installed, print_only: bool) -> Written {
        Written {
            command_resolves: Some(installed.resolvable),
            configs: Written::configs(&installed.outcomes, print_only),
        }
    }

    fn uninstall(removed: &[Outcome]) -> Written {
        Written {
            command_resolves: None,
            configs: Written::configs(removed, false),
        }
    }
}

/// `check`'s answer.
struct Found<'a> {
    checked: &'a Checked,
    command: &'a str,
    catalog: &'a Catalog,
}

#[derive(Serialize)]
struct FoundJson {
    binary_on_path: bool,
    configs: Vec<FoundConfig>,
}

#[derive(Serialize)]
struct FoundConfig {
    engine: String,
    scope: &'static str,
    path: String,
    /// "registered", "not-registered" or "unreadable".
    state: &'static str,
    /// Why it is unreadable; null otherwise.
    error: Option<presentation::Envelope>,
}

impl Render for Found<'_> {
    fn json(&self) -> impl Serialize + '_ {
        FoundJson {
            binary_on_path: self.checked.resolvable,
            configs: self
                .checked
                .found
                .iter()
                .map(|f| {
                    let (state, error) = match &f.state {
                        State::Registered => ("registered", None),
                        State::NotRegistered => ("not-registered", None),
                        State::Unreadable(why) => ("unreadable", Some(self.catalog.envelope(why))),
                    };
                    FoundConfig {
                        engine: f.engine.clone(),
                        scope: f.scope,
                        path: f.path.display().to_string(),
                        state,
                        error,
                    }
                })
                .collect(),
        }
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let on = if self.checked.resolvable {
            "on"
        } else {
            "NOT on"
        };
        writeln!(out, "binary: {} {on} PATH", self.command)?;
        for f in &self.checked.found {
            let state = match &f.state {
                State::Registered => "registered".to_string(),
                State::NotRegistered => "not registered".to_string(),
                State::Unreadable(why) => format!("unreadable: {}", self.catalog.message(why)),
            };
            let engine = presentation_pad(&f.engine, 12);
            let scope = presentation_pad(f.scope, 8);
            writeln!(out, "{engine} {scope} {state} ({})", f.path.display())?;
        }
        Ok(())
    }
}

/// `text` padded with spaces to `width` characters, as Go's `%-<width>s`.
fn presentation_pad(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(pad))
}

/// `error` on stderr: its envelope in JSON; in text its catalog message
/// after the program's name.
fn report(
    error: &ManageError,
    format: Format,
    prog: &str,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> io::Result<()> {
    match (format, error) {
        (Format::Json, _) => presentation::write_json(err, &catalog.envelope(error)),
        (Format::Text, ManageError::Usage { detail }) => writeln!(err, "{detail}"),
        (Format::Text, _) => writeln!(err, "{prog}: {}", catalog.message(error)),
    }
}

fn output_failed(error: io::Error) -> ManageError {
    ManageError::Output {
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
