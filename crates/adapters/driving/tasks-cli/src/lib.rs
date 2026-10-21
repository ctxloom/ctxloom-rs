//! The tasks command line: `add`, `list`, `tag` and `import`, translated
//! into the application's use cases and their answers rendered through
//! presentation, with room for another crate's commands mounted beside
//! them ([`run_mounted`]). Text on a terminal, JSON otherwise, unless
//! `--format` or `--json` says; stdout holds the answer alone (one JSON
//! document), and notices, such as the project a write went to, go to
//! stderr. Exit 0, or 1 with the error on stderr: its catalog message in
//! text, its envelope in JSON.

mod error;
mod import;
mod notice;
mod render;

use std::ffi::OsString;
use std::io::{self, Write};

use annals_diagnostic::Diagnostic;
use clap::{Parser, Subcommand};
use presentation::{Catalog, CatalogError, Format, Render, Source};
use tasks_application::{
    ImportRequest, ImportService, ListQuery, NewTask, TagChange, TaskService, Written,
};

pub use error::{diagnostic_samples, CliError, EN_US};
pub use import::{ImportJson, Imported};
pub use notice::Notice;
pub use render::{headline, rfc3339, table, Added, Listed, Tagged, TaskJson, HEADLINE_WIDTH};

/// The program's name: every diagnostic's prefix.
pub const PROG: &str = "taskloom";

/// What the command line reads from its process besides its arguments.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Context {
    /// CTXLOOM_SESSION_HARP: the session a task is added in.
    pub session: String,
    /// Whether stdout is a terminal, which picks text over JSON.
    pub stdout_is_terminal: bool,
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
    /// Output format: text, or json (the default when stdout is not a terminal).
    #[arg(long, global = true)]
    format: Option<Format>,
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
    /// Import Go taskloom's task logs (~/.ctxloom/tasks/<project-id>.jsonl)
    /// into each project's store: every project's, or --project's alone.
    /// Running it again imports only what is new.
    Import,
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
    /// The format asked for, else text on a terminal and JSON off one.
    fn format(&self, context: &Context) -> Format {
        Format::choose(self.json, self.format, context.stdout_is_terminal)
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
pub async fn run<S: TaskService + ImportService>(
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
pub async fn run_mounted<S: TaskService + ImportService, X: Subcommand>(
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
    let format = cli.format(context);
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
        Command::Mounted(command) => {
            let flags = Mounted {
                format,
                project: cli.project,
            };
            return mounted(command, flags, catalog, out, err);
        }
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
