//! Parse, dispatch and report.

use std::ffi::OsString;
use std::io::Write;

#[cfg(feature = "test-support")]
use angzarr_diagnostic::Diagnostic;
use angzarr_presentation::{write_json, Catalog, Format};
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Command, FromArgMatches};
use serde::Serialize;

use crate::commands::{command, Cli, Verb, PROG};
use crate::error::CliError;
use crate::extension::{Call, Extension};

/// What the command line reads from the process besides its arguments.
#[derive(Clone, Debug)]
pub struct Context {
    pub version: String,
    /// The lock protocol this binary speaks (`version` reports it).
    pub lock_protocol: u32,
    pub stdout_is_terminal: bool,
}

/// What the commands call on the wiring.
pub trait Services {
    /// The commands another crate owns, joined to this command line.
    fn extension(&mut self) -> &mut dyn Extension;

    /// A newly minted session id.
    #[cfg(feature = "test-support")]
    fn session_name(&mut self) -> String;

    /// The run id of an incarnation of a session.
    #[cfg(feature = "test-support")]
    fn run_id(&mut self, session: &str, incarnation: u64) -> Result<String, Box<dyn Diagnostic>>;

    /// A project's store, opened under the home: where its database is.
    #[cfg(feature = "test-support")]
    fn open_store(&mut self, project: &str) -> Result<String, Box<dyn Diagnostic>>;

    /// One value of every diagnostic the binary can raise besides this
    /// crate's own.
    #[cfg(feature = "test-support")]
    fn diagnostic_samples(&self) -> Vec<Box<dyn Diagnostic>>;
}

/// Run `args` and answer: the exit code.
pub fn run(
    args: impl IntoIterator<Item = OsString>,
    context: &Context,
    catalog: &Catalog,
    services: &mut impl Services,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let args: Vec<OsString> = args.into_iter().collect();
    let nouns = services.extension().nouns();
    let names: Vec<String> = nouns.iter().map(|n| n.get_name().to_string()).collect();
    let command = command(&context.version, nouns);
    let matches = match command.clone().try_get_matches_from(&args) {
        Ok(matches) => matches,
        Err(e) => return refused(e, &command, &args, context, catalog, out, err),
    };
    if let Some(noun) = matches
        .subcommand_name()
        .filter(|name| names.iter().any(|n| n == name))
    {
        return extended(noun, &matches, &args, context, catalog, services, out, err);
    }
    let Ok(cli) = Cli::from_arg_matches(&matches) else {
        return 2;
    };
    let format = match chosen(cli.globals.format.as_deref(), context) {
        Ok(format) => format,
        Err(e) => return report(&e, asked_format(&args, context), catalog, err),
    };
    let _ = cli.globals.verbose;
    let answered = match cli.command {
        Some(Verb::Version) => version(context, format, out),
        #[cfg(feature = "test-support")]
        Some(Verb::Test(verb)) => crate::test_commands::run(verb, format, catalog, services, out),
        None => Ok(()),
    };
    match answered {
        Ok(()) => 0,
        Err(e) => report(&e, format, catalog, err),
    }
}

/// Run a command an extension owns.
#[allow(clippy::too_many_arguments)]
fn extended(
    noun: &str,
    matches: &clap::ArgMatches,
    args: &[OsString],
    context: &Context,
    catalog: &Catalog,
    services: &mut impl Services,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let asked = matches.get_one::<String>("format").map(String::as_str);
    let explicit_format = match asked
        .map(|value| {
            value
                .parse::<Format>()
                .map_err(|e| CliError::Format { value: e.value })
        })
        .transpose()
    {
        Ok(format) => format,
        Err(e) => return report(&e, asked_format(args, context), catalog, err),
    };
    let call = Call {
        matches,
        explicit_format,
        default_format: Format::choose(false, None, context.stdout_is_terminal),
        stdout_is_terminal: context.stdout_is_terminal,
        degraded: matches.get_flag("degraded"),
        no_companions: matches.get_flag("no_companions"),
        config_set: matches
            .get_many::<String>("config_set")
            .map(|values| values.cloned().collect())
            .unwrap_or_default(),
    };
    match services.extension().run(noun, &call, catalog, out, err) {
        Ok(code) => code,
        Err(failure) => report(&CliError::Extension(failure), call.format(), catalog, err),
    }
}

/// The format `--format` asks for, else the terminal's.
fn chosen(asked: Option<&str>, context: &Context) -> Result<Format, CliError> {
    let asked = asked
        .map(|value| {
            value
                .parse::<Format>()
                .map_err(|e| CliError::Format { value: e.value })
        })
        .transpose()?;
    Ok(Format::choose(false, asked, context.stdout_is_terminal))
}

/// The format the words ask for, since clap parsed nothing: the last
/// `--format` that is one, else the terminal's.
fn asked_format(args: &[OsString], context: &Context) -> Format {
    let words: Vec<&str> = args.iter().filter_map(|a| a.to_str()).collect();
    let asked = words.iter().enumerate().rev().find_map(|(i, word)| {
        let value = match word.strip_prefix("--format=") {
            Some(value) => value,
            None if *word == "--format" => words.get(i + 1).copied()?,
            None => return None,
        };
        value.parse::<Format>().ok()
    });
    Format::choose(false, asked, context.stdout_is_terminal)
}

#[derive(Serialize)]
struct Version<'a> {
    name: &'a str,
    version: &'a str,
    lock_protocol: u32,
}

/// `version`: json is Go's `{name, version, lock_protocol}`; text the bare
/// version.
fn version(context: &Context, format: Format, out: &mut dyn Write) -> Result<(), CliError> {
    let written = match format {
        Format::Json => write_json(
            out,
            &Version {
                name: PROG,
                version: &context.version,
                lock_protocol: context.lock_protocol,
            },
        ),
        Format::Text => writeln!(out, "{}", context.version),
    };
    written.map_err(|e| CliError::Output {
        detail: e.to_string(),
    })
}

/// A parse that did not yield a command: help and the version flag are
/// answers; the rest are usage errors.
fn refused(
    e: clap::Error,
    command: &Command,
    args: &[OsString],
    context: &Context,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match e.kind() {
        // Go's cobra line, which the VS Code extension's probe reads.
        ErrorKind::DisplayVersion => {
            let _ = writeln!(out, "{PROG} version {}", context.version);
            0
        }
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = write!(out, "{}", e.render());
            0
        }
        ErrorKind::InvalidSubcommand => {
            let unknown = unknown_verb(&e, command, args);
            report(&unknown, asked_format(args, context), catalog, err)
        }
        _ => {
            let usage = CliError::Usage {
                detail: e.render().to_string().trim_end().to_string(),
            };
            report(&usage, asked_format(args, context), catalog, err)
        }
    }
}

/// The unknown-verb error: the verb, the command it is not under, and the
/// verbs there are, the closest first.
fn unknown_verb(e: &clap::Error, root: &Command, args: &[OsString]) -> CliError {
    let verb = match e.get(ContextKind::InvalidSubcommand) {
        Some(ContextValue::String(verb)) => verb.clone(),
        _ => String::new(),
    };
    let (parent, path) = locate(root, args);
    let mut choices: Vec<String> = Vec::new();
    if let Some(ContextValue::Strings(suggested)) = e.get(ContextKind::SuggestedSubcommand) {
        choices.extend(suggested.iter().cloned());
    }
    for sub in parent.get_subcommands().filter(|s| !s.is_hide_set()) {
        let name = sub.get_name().to_string();
        if !choices.contains(&name) {
            choices.push(name);
        }
    }
    CliError::UnknownVerb {
        verb,
        command: path,
        choices: choices.join(", "),
    }
}

/// The deepest command `args` name, and its path: the words that are
/// commands, skipping flags and the value of `--format`.
fn locate<'a>(root: &'a Command, args: &[OsString]) -> (&'a Command, String) {
    let mut at = root;
    let mut path = PROG.to_string();
    let mut words = args.iter().skip(1).filter_map(|a| a.to_str());
    while let Some(word) = words.next() {
        if word == "--format" {
            words.next();
        } else if !word.starts_with('-') {
            match at.find_subcommand(word) {
                Some(sub) => {
                    at = sub;
                    path = format!("{path} {word}");
                }
                None => break,
            }
        }
    }
    (at, path)
}

/// Say `error` on `err` in `format`; its exit code.
pub(crate) fn report(
    error: &CliError,
    format: Format,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> u8 {
    let _ = match format {
        Format::Json => write_json(err, &catalog.envelope(error)),
        Format::Text => text_error(error, catalog, err),
    };
    error.exit_code()
}

/// `ctxloom: <message>`, then `  fix: <remedy>` where the catalog has one.
fn text_error(error: &CliError, catalog: &Catalog, err: &mut dyn Write) -> std::io::Result<()> {
    writeln!(err, "{PROG}: {}", catalog.message(error))?;
    if let Ok(fix) = catalog.render(&error.fix()) {
        writeln!(err, "  fix: {fix}")?;
    }
    Ok(())
}
