//! `taskloom run [task-harp-id]`: launch an agent session on a task, by
//! handing it to `ctxloom run`, as Go taskloom does. With a harp id that
//! task is launched; without one, on a terminal, a line-buffered picker
//! on stderr offers the open tasks (`a` shows every status, `q` quits).
//!
//! The launch is ``ctxloom run [--session <origin>] --seed-task <harp>
//! [--seed-status "To Do"] --prompt "Work on this task (`<harp>`): <text>"``,
//! Go's argv: ctxloom mints the new session, continues the task's
//! originating session when it has one, and marks the task In Progress
//! in Go taskloom's log, not this store (`--no-start` asks it to reset
//! the task to To Do instead). ctxloom
//! inherits stdin, stdout and stderr; taskloom writes nothing on stdout.
//! Who is launched is a [`Launch`]: [`Ctxloom`], `ctxloom` found on a
//! PATH, in the binary.

mod error;

use std::ffi::OsString;
use std::future::Future;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use annals_diagnostic::Diagnostic;
use clap::Subcommand;
use presentation::{Catalog, Format};
use tasks_application::{AppError, ListQuery, Task, TaskService};

pub use error::{diagnostic_samples, RunError, RunNotice, EN_US};

/// The status a task is waiting in, as the picker's open view and
/// `--no-start` name it.
pub const TO_DO: &str = "To Do";

/// The status of a task being worked on.
pub const IN_PROGRESS: &str = "In Progress";

/// The program `run` hands a task to.
pub const CTXLOOM: &str = "ctxloom";

/// The command, for the binary to mount.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Group {
    /// Browse a task and launch an agent on it (via ctxloom run)
    #[command(
        long_about = "Browse the project's open tasks and launch an agent on the chosen one.\n\n\
ctxloom marks the selected task \"In Progress\" (in Go taskloom's task log, not\n\
this store) under a brand-new session that continues the task's originating\n\
session. The agent starts with the task text as its prompt. Launching shells\n\
out to `ctxloom run`, so ctxloom must be on PATH.\n\n\
With a task harp id argument the picker is skipped and that task is launched\n\
directly. In a non-interactive shell a task harp id is required."
    )]
    Run {
        /// The task to launch, whatever its status; without one, a picker offers
        /// the open tasks
        #[arg(value_name = "task-harp-id")]
        harp_id: Option<String>,
        /// Reset the task to To Do instead of marking it In Progress
        #[arg(long)]
        no_start: bool,
    },
}

/// What a run is made with besides the command.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Settings {
    /// How an error is reported.
    pub format: Format,
    /// `--project`: the project whose tasks are offered.
    pub project: Option<String>,
    /// The program's name: every diagnostic's prefix.
    pub prog: &'static str,
    /// Whether stdin and stdout are terminals, so a task can be picked.
    pub interactive: bool,
}

/// The tasks a run chooses among: every task of the project, whatever
/// its status. Any [`TaskService`] lists them.
pub trait Tasks {
    fn every_task(
        &mut self,
        project: Option<String>,
    ) -> impl Future<Output = Result<Vec<Task>, AppError>>;
}

impl<S: TaskService> Tasks for S {
    async fn every_task(&mut self, project: Option<String>) -> Result<Vec<Task>, AppError> {
        // Every status: the picker hides finished tasks itself, so `a`
        // can show them, and a finished task named by its id still
        // launches.
        let query = ListQuery {
            project,
            all: true,
            ..ListQuery::default()
        };
        Ok(self.list(query).await?.tasks().cloned().collect())
    }
}

/// Who a task is handed to.
pub trait Launch {
    /// Run `ctxloom` with `args` to its end.
    fn launch(&mut self, args: &[String]) -> Result<(), RunError>;
}

/// `ctxloom` as found on `path` (a PATH's value), run with the process's
/// stdin, stdout and stderr.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ctxloom {
    pub path: Option<OsString>,
}

impl Ctxloom {
    /// The `ctxloom` on `path`: the first executable file of that name in
    /// its directories, in order.
    pub fn find(&self) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        std::env::split_paths(path)
            .map(|dir| dir.join(CTXLOOM))
            .find(|candidate| executable(candidate))
    }
}

/// Whether `path` is a file this process may run.
fn executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

impl Launch for Ctxloom {
    fn launch(&mut self, args: &[String]) -> Result<(), RunError> {
        let program = self.find().ok_or(RunError::NoCtxloom)?;
        let status = Command::new(program)
            .args(args)
            .status()
            .map_err(|e| RunError::Launch {
                detail: e.to_string(),
            })?;
        if status.success() {
            return Ok(());
        }
        Err(match status.code() {
            Some(status) => RunError::Exited { status },
            None => RunError::Launch {
                detail: status.to_string(),
            },
        })
    }
}

/// `ctxloom`'s arguments for launching `task`: Go's `launchTaskAgent`.
pub fn launch_args(task: &Task, no_start: bool) -> Vec<String> {
    let mut args = vec!["run".to_string()];
    if !task.origin_session.is_empty() {
        args.extend(["--session".to_string(), task.origin_session.clone()]);
    }
    args.extend(["--seed-task".to_string(), task.harp_id.clone()]);
    if no_start {
        args.extend(["--seed-status".to_string(), TO_DO.to_string()]);
    }
    args.extend([
        "--prompt".to_string(),
        format!("Work on this task (`{}`): {}", task.harp_id, task.text),
    ]);
    args
}

/// Run `command`: find or pick the task among `tasks`, and hand it to
/// `launcher`. The picker reads `input` and writes to `err`, as do errors
/// and notices, rendered from `catalog`; the exit code.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    tasks: &mut impl Tasks,
    command: Group,
    settings: &Settings,
    launcher: &mut impl Launch,
    input: &mut dyn BufRead,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> u8 {
    let Group::Run { harp_id, no_start } = command;
    let result = match chosen(tasks, harp_id, settings, input, err).await {
        Ok(Some(task)) => launcher.launch(&launch_args(&task, no_start)),
        Ok(None) => {
            let _ = writeln!(
                err,
                "{}: {}",
                settings.prog,
                catalog.message(&RunNotice::Cancelled as &dyn Diagnostic)
            );
            Ok(())
        }
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = report(&error, settings, catalog, err);
            1
        }
    }
}

/// The task to launch: the one `harp_id` names, else the one picked;
/// `None` when the person quits the picker.
async fn chosen(
    tasks: &mut impl Tasks,
    harp_id: Option<String>,
    settings: &Settings,
    input: &mut dyn BufRead,
    err: &mut dyn Write,
) -> Result<Option<Task>, RunError> {
    let tasks = tasks
        .every_task(settings.project.clone())
        .await
        .map_err(RunError::App)?;
    if let Some(harp_id) = harp_id {
        return match tasks.into_iter().find(|t| t.harp_id == harp_id) {
            Some(task) => Ok(Some(task)),
            None => Err(RunError::NotFound { harp_id }),
        };
    }
    if !settings.interactive {
        return Err(RunError::NotInteractive);
    }
    pick(err, input, &tasks).map_err(|e| RunError::ReadSelection {
        detail: e.to_string(),
    })
}

/// Go's line-buffered picker: the open tasks (To Do, In Progress) on
/// `out`, a row number read from `input` choosing one, `a` showing every
/// status (and back), and a blank line, `q`, `quit`, `exit` or the end of
/// `input` quitting. `None` on a quit or when there is nothing to offer;
/// an error only when reading `input` failed.
pub fn pick(
    out: &mut dyn Write,
    input: &mut dyn BufRead,
    all: &[Task],
) -> io::Result<Option<Task>> {
    if all.is_empty() {
        writeln!(out, "(no tasks)")?;
        return Ok(None);
    }
    let mut show_all = false;
    loop {
        let view: Vec<&Task> = all
            .iter()
            .filter(|t| show_all || t.status == TO_DO || t.status == IN_PROGRESS)
            .collect();
        picker(out, &view, show_all)?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        match line.trim() {
            "" | "q" | "quit" | "exit" => return Ok(None),
            "a" | "all" => show_all = !show_all,
            choice => match choice.parse::<usize>() {
                Ok(n) if (1..=view.len()).contains(&n) => return Ok(Some(view[n - 1].clone())),
                _ => writeln!(
                    out,
                    "  (enter a row number, a to toggle all statuses, q to quit)"
                )?,
            },
        }
    }
}

/// One view of the picker and its prompt: Go's `renderTaskPicker`.
fn picker(out: &mut dyn Write, view: &[&Task], show_all: bool) -> io::Result<()> {
    if view.is_empty() {
        writeln!(
            out,
            "(no open tasks; press a to show all statuses, q to quit)"
        )?;
    }
    for (i, task) in view.iter().enumerate() {
        let origin = if task.origin_session.is_empty() {
            "(none)"
        } else {
            task.origin_session.as_str()
        };
        writeln!(
            out,
            " [{}] {} {} \u{b7} {origin}",
            i + 1,
            padded(&task.status, 12),
            padded(&task.text, 50)
        )?;
    }
    let scope = if show_all { "all" } else { "open" };
    write!(
        out,
        "Choose [1-{}] launch \u{b7} a toggle all (showing {scope}) \u{b7} q quit\n> ",
        view.len()
    )
}

/// `text` padded with spaces to `width` characters, as Go's `%-<width>s`
/// pads (by rune); longer text is left whole.
fn padded(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(pad))
}

/// `error` on stderr: its envelope in JSON; in text its catalog message
/// after the program's name.
fn report(
    error: &RunError,
    settings: &Settings,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> io::Result<()> {
    match settings.format {
        Format::Json => presentation::write_json(err, &catalog.envelope(error)),
        Format::Text => writeln!(err, "{}: {}", settings.prog, catalog.message(error)),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
