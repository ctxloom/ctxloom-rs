//! The tasks command line: `add`, `list` and `tag`, translated into the
//! application's use cases and their answers rendered through
//! presentation. Text on a terminal, JSON otherwise, unless `--format` or
//! `--json` says; stdout holds the answer alone (one JSON document), and
//! notices, such as the project a write went to, go to stderr. Exit 0, or 1
//! with the error on stderr: its catalog message in text, its envelope in
//! JSON.

mod error;
mod notice;
mod render;

use std::ffi::OsString;
use std::io::{self, Write};

use annals_diagnostic::Diagnostic;
use clap::{Parser, Subcommand};
use presentation::{Catalog, CatalogError, Format, Render, Source};
use tasks_application::{ListQuery, NewTask, TagChange, TaskService, Written};

pub use error::{diagnostic_samples, CliError, EN_US};
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
struct Cli {
    /// Output format: text, or json (the default when stdout is not a terminal).
    #[arg(long, global = true)]
    format: Option<Format>,
    /// Shorthand for --format json.
    #[arg(long, global = true)]
    json: bool,
    /// Project id to act on (overrides CTXLOOM_PROJECT_ID and the working directory's).
    #[arg(long, global = true)]
    project: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
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
}

impl Cli {
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
pub async fn run<S: TaskService>(
    args: impl IntoIterator<Item = OsString>,
    context: &Context,
    catalog: &Catalog,
    service: &mut S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let args: Vec<OsString> = args.into_iter().collect();
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            // Help and --version are answers, on stdout; the rest are
            // usage errors.
            if e.use_stderr() {
                let usage = CliError::Usage {
                    detail: e.render().to_string(),
                };
                let _ = report(&usage, asked_format(&args, context), catalog, err);
                return 1;
            }
            let _ = write!(out, "{}", e.render());
            return 0;
        }
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
            };
            match service.tag(change).await {
                Ok(written) => wrote(&written, &Tagged(&written.task), format, catalog, out, err),
                Err(e) => Err(CliError::App(e)),
            }
        }
        Command::List { status, all } => {
            let query = ListQuery {
                project: cli.project,
                statuses: status,
                all,
            };
            match service.list(query).await {
                Ok(listing) => listed(&listing, format, catalog, out, err),
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
    };
    let noticed = match listing.project {
        None => notify(&Notice::NoProject, catalog, err),
        Some(_) => Ok(()),
    };
    noticed
        .and_then(|()| presentation::render(out, format, &view))
        .map_err(output_failed)
}

fn output_failed(error: io::Error) -> CliError {
    CliError::Output {
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
