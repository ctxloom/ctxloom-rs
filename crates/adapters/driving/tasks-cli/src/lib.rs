//! The tasks command line: `add`, `status`, `edit`, `list`, `show`,
//! `summary`, `statuses`, `tag`, `tags`, `lint`, `prune` and `import`, translated
//! into the application's use cases and their answers rendered through
//! presentation, with room for another crate's commands mounted beside
//! them ([`run_mounted`]); and the companion contract ctxloom reads,
//! `version` and `loadout`. Text on a terminal, JSON otherwise, unless
//! `--format` or `--json` says; stdout holds the answer alone (one JSON
//! document), and notices, such as the project a write went to, go to
//! stderr. Exit 0, or 1 with the error on stderr: its catalog message in
//! text, its envelope in JSON.

mod commands;
mod companion;
mod error;
mod import;
mod notice;
mod prune;
mod render;

use std::ffi::OsString;
use std::io::{self, Write};

use annals_diagnostic::Diagnostic;
use clap::{Parser, Subcommand};
use presentation::{Catalog, CatalogError, Format, Render, Source, UnknownFormat};
use tasks_application::{ImportRequest, ImportService, PruneService, TaskService, Written};

pub use companion::{Asked, VersionJson, LOADOUT};
pub use error::{diagnostic_samples, CliError, EN_US};
pub use import::{ImportJson, Imported};
pub use notice::Notice;
pub use prune::{apply_command, PruneReport};
pub use render::{headline, table, Added, CompactJson, Listed, RowJson, Tagged, HEADLINE_WIDTH};
pub use tasks_presentation::{rfc3339, TaskJson};

/// The program's name: every diagnostic's prefix.
pub const PROG: &str = "taskloom";

/// What the command line reads from its process besides its arguments.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Context {
    /// CTXLOOM_SESSION_HARP: the session a task is added in.
    pub session: String,
    /// Whether stdout is a terminal, which picks text over JSON.
    pub stdout_is_terminal: bool,
    /// The binary's version, as `version` reports it.
    pub version: String,
    /// The lock-taking protocol the binary's lock takers follow, as
    /// `version` reports it (Go's `safefs.LockProtocol`): ctxloom asks
    /// every running binary for it before it removes a lock file. 0 when
    /// the binary takes no shared lock, and then left out.
    pub lock_protocol: u32,
}

/// The catalog of every diagnostic the command line prints: its own, the
/// use cases', and `adapters'`, the driven adapters' catalogs, which only
/// the composition root knows.
pub fn catalog(adapters: &[Source]) -> Result<Catalog, CatalogError> {
    let mut sources = vec![EN_US];
    sources.extend(tasks_application::CATALOGS);
    sources.extend_from_slice(adapters);
    Catalog::en_us(&sources)
}

#[derive(Parser, Debug)]
#[command(name = PROG, about = "Manage the per-project task store")]
struct Cli<X: Subcommand> {
    /// Output format: text, or json (the default when stdout is not a
    /// terminal); yaml, for loadout alone.
    #[arg(long, global = true)]
    format: Option<Asked>,
    /// Shorthand for --format json.
    #[arg(long, global = true)]
    json: bool,
    /// Project id to act on (overrides CTXLOOM_PROJECT_ID and the working directory's).
    #[arg(long, global = true)]
    project: Option<String>,
    /// Override a configuration key for this run: <path>=<value>, e.g.
    /// tag_schema=<declaration>,... (repeatable; beats TASKLOOM_CONFIG_*
    /// and every config.yaml). The task commands take it; project, mcp,
    /// watch, run and manage do not.
    #[arg(long = "config-set", global = true, value_name = "PATH=VALUE")]
    config_set: Vec<String>,
    #[command(subcommand)]
    command: Command<X>,
}

#[derive(Subcommand, Debug)]
enum Command<X: Subcommand> {
    /// Add a new task.
    Add(commands::AddArgs),
    /// List tasks: by default the active ones (not Done, Archived or
    /// Deferred), of this project, or of every project with --global (and
    /// where the working directory is in no project, no git repository and
    /// no CTXLOOM_ROOT, saying so on stderr).
    List(commands::ListArgs),
    /// Change the status of a task; Deferred needs a revive trigger, which
    /// hides the task from the default list until it fires.
    Status(commands::StatusArgs),
    /// Replace a task's text whole; its status and trigger are kept.
    Edit(commands::EditArgs),
    /// Show tasks whole, in the order asked: status, tags, trigger and
    /// every line of their text. Every id must be found, or nothing is
    /// shown. In JSON one id is an object, several an array.
    Show(commands::ShowArgs),
    /// Check every task's tags, whatever its status, against the tag
    /// schema: a value outside its enum or range, a scalar target with two
    /// values, and a formula naming a value no enum declares. Advisory,
    /// never a write gate; it exits 1 on a violation, and when the schema
    /// gave it no target to check.
    Lint(commands::GlobalArgs),
    /// Count the tasks in each status, every task included whatever list
    /// would hide, and name those In Progress.
    Summary(commands::GlobalArgs),
    /// List the statuses: the defaults in their display order (In
    /// Progress, To Do, Deferred, Done, Archived), then any other in use,
    /// each saying whether it is terminal or requires a trigger. @open and
    /// @terminal are the classes of the statuses not terminal and terminal.
    Statuses(commands::GlobalArgs),
    /// Add and/or remove tags on a task; adds apply before removes.
    Tag(commands::TagArgs),
    /// List the tags in use, with how many tasks hold each: "active" those
    /// a default list shows, "total" every one, finished or Deferred too.
    /// The tag schema's hidden tags are left out. The filters narrow the
    /// tasks counted.
    Tags(commands::TagsArgs),
    /// Drop old finished tasks from the store (reports only, without
    /// --yes). A task is selected when its status is one of --status, its
    /// last status change is older than --older-than, and it matches
    /// --tag-query, if given. With --yes each is removed, so every list
    /// drops it, and its stream purged, freeing its harp to be minted
    /// again; without it nothing changes. A surviving task naming a pruned
    /// one (a relates:<harp> tag, or the harp in its text or trigger) is
    /// reported, and left as written. A task an earlier prune removed but
    /// could not purge is taken too, whatever the flags select. To keep a
    /// finished task, archive it and leave Archived out of --status.
    Prune(commands::PruneArgs),
    /// Import Go taskloom's task logs (~/.ctxloom/tasks/<project-id>.jsonl)
    /// into each project's store: every project's, or --project's alone.
    /// Running it again imports only what is new.
    Import,
    /// Print the taskloom version.
    Version,
    /// Print the context, commands, hooks and MCP servers taskloom
    /// contributes to a session: the loadout document ctxloom reads
    /// (`taskloom loadout --format yaml`).
    Loadout,
    /// Commands another crate defines, mounted beside these.
    #[command(flatten)]
    Mounted(X),
}

/// Nothing mounted.
#[derive(Subcommand, Debug)]
pub enum Unmounted {}

/// What a mounted command is handed besides itself: the global flags
/// parsed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mounted {
    /// The format asked for, or chosen by whether stdout is a terminal.
    pub format: Format,
    /// `--project`.
    pub project: Option<String>,
}

impl<X: Subcommand> Cli<X> {
    /// The format asked for, else text on a terminal and JSON off one;
    /// refused when it is yaml, which only `loadout` speaks.
    fn format(&self, context: &Context) -> Result<Format, CliError> {
        let asked = match self.format {
            Some(Asked::Yaml) => {
                return Err(CliError::Format(UnknownFormat {
                    value: "yaml".to_string(),
                }))
            }
            Some(Asked::Format(format)) => Some(format),
            None => None,
        };
        Ok(Format::choose(self.json, asked, context.stdout_is_terminal))
    }
}

/// The format `args` ask for, read without parsing them: for a usage
/// error, where clap parsed nothing.
fn asked_format(args: &[OsString], context: &Context) -> Format {
    let (mut json, mut asked) = (false, None);
    let mut words = args.iter().skip(1).map(|a| a.to_str().unwrap_or_default());
    while let Some(word) = words.next() {
        let value = match word {
            "--" => break,
            "--json" => {
                json = true;
                continue;
            }
            "--format" => words.next(),
            _ => word.strip_prefix("--format="),
        };
        asked = value.and_then(|v| v.parse().ok()).or(asked);
    }
    Format::choose(json, asked, context.stdout_is_terminal)
}

/// Run the command line `args` (the program name first) against `service`,
/// writing to `out` and `err` with messages from `catalog`; the exit code.
pub async fn run<S: TaskService + ImportService + PruneService>(
    args: impl IntoIterator<Item = OsString>,
    context: &Context,
    catalog: &Catalog,
    service: &mut S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let unmounted =
        |never: Unmounted, _: Mounted, _: &Catalog, _: &mut dyn Write, _: &mut dyn Write| {
            match never {}
        };
    run_mounted(args, context, catalog, service, unmounted, out, err).await
}

/// The command line parsed, or the exit code of answering it without a
/// command: help and --version on stdout, a usage error on `err`.
fn parse<X: Subcommand>(
    args: impl IntoIterator<Item = OsString>,
    context: &Context,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Cli<X>, u8> {
    let args: Vec<OsString> = args.into_iter().collect();
    Cli::<X>::try_parse_from(&args).map_err(|e| {
        if e.use_stderr() {
            let usage = CliError::Usage {
                detail: e.render().to_string(),
            };
            let _ = report(&usage, asked_format(&args, context), catalog, err);
            return 1;
        }
        let _ = write!(out, "{}", e.render());
        0
    })
}

/// [`run`], with the commands `X` mounted beside the task commands: one
/// of them parsed is handed to `mounted` with the global flags and the
/// catalog it renders its diagnostics from, and its exit code is the
/// run's.
pub async fn run_mounted<S: TaskService + ImportService + PruneService, X: Subcommand>(
    args: impl IntoIterator<Item = OsString>,
    context: &Context,
    catalog: &Catalog,
    service: &mut S,
    mounted: impl FnOnce(X, Mounted, &Catalog, &mut dyn Write, &mut dyn Write) -> u8,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let cli = match parse::<X>(args, context, catalog, out, err) {
        Ok(cli) => cli,
        Err(code) => return code,
    };
    let format = match answer_format(&cli, context, catalog, out, err) {
        Ok(format) => format,
        Err(code) => return code,
    };
    let ctx = commands::Ctx {
        project: cli.project,
        config_set: cli.config_set,
        format,
        catalog,
        session: &context.session,
    };
    let result = match cli.command {
        Command::Add(args) => commands::add(args, ctx, service, out, err).await,
        Command::Status(args) => commands::status(args, ctx, service, out, err).await,
        Command::Edit(args) => commands::edit(args, ctx, service, out, err).await,
        Command::Show(args) => commands::show(args, ctx, service, out, err).await,
        Command::Lint(args) => commands::lint(args, ctx, service, out, err).await,
        Command::Summary(args) => commands::summary(args, ctx, service, out, err).await,
        Command::Statuses(args) => commands::statuses(args, ctx, service, out, err).await,
        Command::Tag(args) => commands::tag(args, ctx, service, out, err).await,
        Command::Tags(args) => commands::tags(args, ctx, service, out, err).await,
        Command::List(args) => commands::list(args, ctx, service, out, err).await,
        Command::Import => {
            let request = ImportRequest {
                project: ctx.project,
            };
            match service.import(request).await {
                Ok(report) => imported(&report, format, catalog, out),
                Err(e) => Err(CliError::App(e)),
            }
        }
        Command::Prune(args) => commands::prune(args, ctx, service, out, err).await,
        Command::Mounted(command) => {
            let flags = Mounted {
                format,
                project: ctx.project,
            };
            return mounted(command, flags, catalog, out, err);
        }
        Command::Version => version(context, format, out),
        // Answered before the format is chosen: it has its own.
        Command::Loadout => Ok(()),
    };
    finish(result, format, catalog, err)
}

/// The format `cli`'s answer is written in; or the exit code when there
/// is no format to choose: `loadout`, printed in its own, or a format
/// refused.
fn answer_format<X: Subcommand>(
    cli: &Cli<X>,
    context: &Context,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Format, u8> {
    let fallback = Format::choose(cli.json, None, context.stdout_is_terminal);
    if let Command::Loadout = cli.command {
        let printed = companion::loadout(cli.json, cli.format, out);
        return Err(finish(printed, fallback, catalog, err));
    }
    cli.format(context)
        .map_err(|error| finish(Err(error), fallback, catalog, err))
}

/// The exit code of `result`, its error reported in `format`.
fn finish(
    result: Result<(), CliError>,
    format: Format,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> u8 {
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = report(&error, format, catalog, err);
            1
        }
    }
}

/// `error` on stderr: its envelope in JSON; in text its catalog message
/// after the program's name, or a usage error as clap words it.
fn report(
    error: &CliError,
    format: Format,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> io::Result<()> {
    match (format, error) {
        (Format::Json, _) => presentation::write_json(err, &catalog.envelope(error)),
        (Format::Text, CliError::Usage { .. }) => write!(err, "{}", catalog.message(error)),
        (Format::Text, _) => writeln!(err, "{PROG}: {}", catalog.message(error)),
    }
}

/// `notice` on stderr, after the program's name.
pub(crate) fn notify(
    notice: &dyn Diagnostic,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> io::Result<()> {
    writeln!(err, "{PROG}: {}", catalog.message(notice))
}

/// Name the project on stderr, then render the task.
pub(crate) fn wrote(
    written: &Written,
    view: &impl Render,
    format: Format,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    notify(&Notice::project(&written.project), catalog, err)
        .and_then(|()| presentation::render(out, format, view))
        .map_err(output_failed)
}

/// The version, as ctxloom's boot probe reads it.
fn version(context: &Context, format: Format, out: &mut dyn Write) -> Result<(), CliError> {
    let version = VersionJson {
        name: PROG,
        version: &context.version,
        lock_protocol: context.lock_protocol,
    };
    presentation::render(out, format, &version).map_err(output_failed)
}

/// Render the report; refuse when it left something behind, so a script
/// sees it in the exit code.
fn imported(
    report: &tasks_application::ImportReport,
    format: Format,
    catalog: &Catalog,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    presentation::render(out, format, &Imported { report, catalog }).map_err(output_failed)?;
    if report.complete() {
        return Ok(());
    }
    let stopped = report
        .projects
        .iter()
        .filter(|p| p.failed.is_some())
        .count();
    let lost = report.projects.iter().map(|p| p.lost.len()).sum();
    Err(CliError::Incomplete { stopped, lost })
}

pub(crate) fn output_failed(error: io::Error) -> CliError {
    CliError::Output {
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
