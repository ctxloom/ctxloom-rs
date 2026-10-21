//! The tasks command line: `add`, `list`, `tag` and `import`, translated
//! into the application's use cases and their answers rendered. Text on a terminal,
//! JSON otherwise, unless `--format` or `--json` says; the project a write
//! went to on stderr, so stdout stays parseable. Exit 0, or 1 with the
//! error on stderr.

mod error;
mod import;
mod render;

use std::ffi::OsString;
use std::io::{self, Write};

use clap::{Parser, Subcommand, ValueEnum};
use tasks_application::{
    ImportRequest, ImportService, ListQuery, NewTask, TagChange, TaskService, Written,
};

pub use error::{diagnostic_samples, CliError};
pub use import::ImportJson;
pub use render::{headline, rfc3339, table, TaskJson, HEADLINE_WIDTH};

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

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
pub enum Format {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(name = PROG, about = "Manage the per-project task store")]
struct Cli {
    /// Output format: text, or json (the default when stdout is not a terminal).
    #[arg(long, global = true, value_enum)]
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
    /// Import Go taskloom's task logs (~/.ctxloom/tasks/<project-id>.jsonl)
    /// into each project's store: every project's, or --project's alone.
    /// Running it again imports only what is new.
    Import,
}

impl Cli {
    /// The format asked for, else text on a terminal and JSON off one.
    fn format(&self, context: &Context) -> Format {
        match (self.json, self.format) {
            (true, _) => Format::Json,
            (false, Some(format)) => format,
            (false, None) if context.stdout_is_terminal => Format::Text,
            (false, None) => Format::Json,
        }
    }
}

/// Run the command line `args` (the program name first) against `service`,
/// writing to `out` and `err`; the exit code.
pub async fn run<S: TaskService + ImportService>(
    args: impl IntoIterator<Item = OsString>,
    context: &Context,
    service: &mut S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            // Help and --version are answers, on stdout; the rest are
            // usage errors.
            if e.use_stderr() {
                let _ = write!(err, "{}", e.render());
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
                Ok(written) => written_task(&written, format, out, err, |t| {
                    format!("{}\t{}\t{}", t.harp_id, t.status, t.text)
                }),
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
                Ok(written) => written_task(&written, format, out, err, |t| {
                    format!("{}\t{}\t{}", t.harp_id, t.status, t.tags.join(","))
                }),
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
                Ok(listing) => listed(&listing, format, out, err),
                Err(e) => Err(CliError::App(e)),
            }
        }
        Command::Import => {
            let request = ImportRequest {
                project: cli.project,
            };
            match service.import(request).await {
                Ok(report) => imported(&report, format, out),
                Err(e) => Err(CliError::App(e)),
            }
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(err, "{PROG}: {error}");
            1
        }
    }
}

/// Name the project on stderr, then render the task.
fn written_task(
    written: &Written,
    format: Format,
    out: &mut dyn Write,
    err: &mut dyn Write,
    line: impl Fn(&tasks_application::Task) -> String,
) -> Result<(), CliError> {
    let wrote = writeln!(
        err,
        "{PROG}: project {}",
        render::project_label(&written.project)
    )
    .and_then(|()| match format {
        Format::Text => writeln!(out, "{}", line(&written.task)),
        Format::Json => render::json(out, &TaskJson::of(&written.task)),
    });
    wrote.map_err(output_failed)
}

fn listed(
    listing: &tasks_application::Listing,
    format: Format,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let wrote = match (&listing.project, format) {
        (None, format) => writeln!(err, "{PROG}: no project here yet: nothing to list").and_then(
            |()| match format {
                Format::Text => render::table(out, &[]),
                Format::Json => render::json(out, &Vec::<TaskJson>::new()),
            },
        ),
        (Some(project), Format::Text) => {
            writeln!(out, "Project: {}\n", render::project_label(project))
                .and_then(|()| render::table(out, &listing.tasks))
        }
        (Some(_), Format::Json) => {
            let tasks: Vec<TaskJson> = listing.tasks.iter().map(TaskJson::of).collect();
            render::json(out, &tasks)
        }
    };
    wrote.map_err(output_failed)
}

/// Render the report; refuse when it left something behind, so a script
/// sees it in the exit code.
fn imported(
    report: &tasks_application::ImportReport,
    format: Format,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    match format {
        Format::Text => import::text(out, report),
        Format::Json => render::json(out, &ImportJson::of(report)),
    }
    .map_err(output_failed)?;
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
