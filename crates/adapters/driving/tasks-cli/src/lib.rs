//! The tasks command line: `add`, `list`, `tag`, `prune` and `import`, translated
//! into the application's use cases and their answers rendered through
//! presentation, with room for another crate's commands mounted beside
//! them ([`run_mounted`]); and the companion contract ctxloom reads,
//! `version` and `loadout`. Text on a terminal, JSON otherwise, unless
//! `--format` or `--json` says; stdout holds the answer alone (one JSON
//! document), and notices, such as the project a write went to, go to
//! stderr. Exit 0, or 1 with the error on stderr: its catalog message in
//! text, its envelope in JSON.

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
use tasks_application::{
    ImportRequest, ImportService, ListQuery, NewTask, PruneRequest, PruneService, TagChange,
    TaskService, Written, DEFAULT_OLDER_THAN, DEFAULT_PRUNE_STATUSES,
};

pub use companion::{Asked, VersionJson, LOADOUT};
pub use error::{diagnostic_samples, CliError, EN_US};
pub use import::{ImportJson, Imported};
pub use notice::Notice;
pub use prune::{apply_command, PruneReport};
pub use render::{headline, table, Added, Listed, Tagged, HEADLINE_WIDTH};
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
    /// and every config.yaml).
    #[arg(long = "config-set", global = true, value_name = "PATH=VALUE")]
    config_set: Vec<String>,
    #[command(subcommand)]
    command: Command<X>,
}

#[derive(Subcommand, Debug)]
enum Command<X: Subcommand> {
    /// Add a new task.
    Add {
        /// The task's text; several words are joined with spaces.
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
        /// Initial status (default: "To Do").
        #[arg(long, default_value = "")]
        status: String,
        /// Revive condition for a Deferred task (required when --status Deferred).
        #[arg(long, default_value = "")]
        trigger: String,
        /// Tag to set at creation (repeatable).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// List tasks: by default the open ones (not Done, Archived or Deferred).
    List {
        /// Only tasks in this status (repeatable, or comma-separated).
        #[arg(long, value_delimiter = ',')]
        status: Vec<String>,
        /// Include the tasks hidden by default.
        #[arg(long)]
        all: bool,
        /// Only the tasks this tag query selects: postfix, tags first and
        /// the operator after (urgent/release/and, urgent/not), or infix
        /// (urgent and not release). Operators: and, or, not; atoms
        /// [ns:]key[op value] with op one of = != > >= < <= ~, and * (any)
        /// or + (present) for a namespace, key or value.
        #[arg(long = "tag-query", default_value = "")]
        tag_query: String,
    },
    /// Add and/or remove tags on a task; adds apply before removes.
    Tag {
        harp_id: String,
        /// Tag to add (repeatable).
        #[arg(long)]
        add: Vec<String>,
        /// Tag to remove (repeatable).
        #[arg(long)]
        remove: Vec<String>,
    },
    /// Drop old finished tasks from the store (reports only, without
    /// --yes). A task is selected when its status is one of --status, its
    /// last status change is older than --older-than, and it matches
    /// --tag-query, if given. With --yes each is removed, so every list
    /// drops it, and its stream purged, freeing its harp to be minted
    /// again; without it nothing changes. A surviving task naming a pruned
    /// one (a relates:<harp> tag, or the harp in its text or trigger) is
    /// reported, and left as written. To keep a finished task, archive it
    /// and leave Archived out of --status.
    Prune(PruneArgs),
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

/// `prune`'s flags.
#[derive(clap::Args, Debug)]
struct PruneArgs {
    /// Only tasks whose last status change is older than this age (30d,
    /// 12w, 720h) or date (2026-01-01).
    #[arg(long = "older-than", default_value = DEFAULT_OLDER_THAN)]
    older_than: String,
    /// The statuses that qualify a task (repeatable, or comma-separated).
    #[arg(long, value_delimiter = ',', default_values = DEFAULT_PRUNE_STATUSES)]
    status: Vec<String>,
    /// Only tasks matching this tag query (see list --tag-query).
    #[arg(long = "tag-query", default_value = "")]
    tag_query: String,
    /// Remove and purge; without it prune only reports what it would.
    #[arg(long)]
    yes: bool,
}

impl PruneArgs {
    /// The prune these flags ask for, and the command that applies it.
    fn request(self, project: Option<String>, config_set: Vec<String>) -> (PruneRequest, String) {
        let apply = apply_command(&self.older_than, &self.status, &self.tag_query);
        let request = PruneRequest {
            project,
            statuses: self.status,
            older_than: self.older_than,
            tag_query: self.tag_query,
            apply: self.yes,
            config_set,
        };
        (request, apply)
    }
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
    let result = match cli.command {
        Command::Add {
            text,
            status,
            trigger,
            tags,
        } => {
            let task = NewTask {
                project: cli.project,
                text: text.join(" "),
                status,
                trigger,
                tags,
                origin_session: context.session.clone(),
                config_set: cli.config_set,
            };
            match service.add(task).await {
                Ok(written) => wrote(&written, &Added(&written.task), format, catalog, out, err),
                Err(e) => Err(CliError::App(e)),
            }
        }
        Command::Tag {
            harp_id,
            add,
            remove,
        } => {
            let change = TagChange {
                project: cli.project,
                harp_id,
                add,
                remove,
                config_set: cli.config_set,
            };
            match service.tag(change).await {
                Ok(written) => wrote(
                    &written,
                    &Tagged(&written.task, &written.shown),
                    format,
                    catalog,
                    out,
                    err,
                ),
                Err(e) => Err(CliError::App(e)),
            }
        }
        Command::List {
            status,
            all,
            tag_query,
        } => {
            let query = ListQuery {
                project: cli.project,
                statuses: status,
                all,
                tag_query,
                config_set: cli.config_set,
            };
            match service.list(query).await {
                Ok(listing) => listed(&listing, format, catalog, out, err),
                Err(e) => Err(CliError::App(e)),
            }
        }
        Command::Prune(args) => {
            let asked = args.request(cli.project, cli.config_set);
            prune(service, asked, format, catalog, out, err).await
        }
        Command::Mounted(command) => {
            let flags = Mounted {
                format,
                project: cli.project,
            };
            return mounted(command, flags, catalog, out, err);
        }
        Command::Version => version(context, format, out),
        // Answered before the format is chosen: it has its own.
        Command::Loadout => Ok(()),
        Command::Import => {
            let request = ImportRequest {
                project: cli.project,
            };
            match service.import(request).await {
                Ok(report) => imported(&report, format, catalog, out),
                Err(e) => Err(CliError::App(e)),
            }
        }
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
fn notify(notice: &Notice, catalog: &Catalog, err: &mut dyn Write) -> io::Result<()> {
    writeln!(
        err,
        "{PROG}: {}",
        catalog.message(notice as &dyn Diagnostic)
    )
}

/// Name the project on stderr, then render the task.
fn wrote(
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

/// Say on stderr when there is no project here, then render the listing.
fn listed(
    listing: &tasks_application::Listing,
    format: Format,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let view = Listed {
        project: listing.project.as_ref(),
        tasks: &listing.tasks,
        shown: &listing.shown,
    };
    let noticed = match listing.project {
        None => notify(&Notice::NoProject, catalog, err),
        Some(_) => Ok(()),
    };
    noticed
        .and_then(|()| presentation::render(out, format, &view))
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

/// Run the prune `asked` (the request, and the command that applies it).
/// Say on stderr when there is no project here, then render the report,
/// and after a report that would take something, say nothing was taken
/// and how to take it.
async fn prune<S: PruneService>(
    service: &mut S,
    (request, apply): (PruneRequest, String),
    format: Format,
    catalog: &Catalog,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let pruning = service.prune(request).await.map_err(CliError::App)?;
    if pruning.project.is_none() {
        notify(&Notice::NothingToPrune, catalog, err).map_err(output_failed)?;
    }
    presentation::render(out, format, &PruneReport(&pruning)).map_err(output_failed)?;
    if pruning.applied || pruning.pruned.is_empty() {
        return Ok(());
    }
    let report_only = Notice::ReportOnly { command: apply };
    notify(&report_only, catalog, err).map_err(output_failed)
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

fn output_failed(error: io::Error) -> CliError {
    CliError::Output {
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
