//! The `project` command group: `show`, `heal`, `repoint` and `orphans`,
//! translated into the project use cases and their answers rendered, as
//! text or one JSON document. It depends on no tasks crate, so any binary
//! mounts it: taskloom does, beside its task commands, and ctxloom can.
//!
//! A binary flattens [`Group`] into its own subcommands, parses its own
//! global flags (the format, the pinned project), and hands the group and
//! those to [`run`]. Exit 0, or 1 with the error on stderr.

mod error;
mod render;

use std::io::{self, Write};

use clap::Subcommand;
use project_application::{ProjectAdmin, Projects};

pub use error::{diagnostic_samples, CliError};
pub use render::{
    change_line, problem_line, ChangeJson, EntryJson, OrphanJson, PlanJson, ProblemJson, ShowJson,
};

/// The group, mounted as `project`.
#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Group {
    /// Show, heal and rebind which project this directory is.
    #[command(subcommand)]
    Project(ProjectCommand),
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum ProjectCommand {
    /// Show this directory's project: its id, root, marker, registry
    /// entries and database, and every inconsistency found.
    Show,
    /// Repair the skew between this directory's .project-id and the
    /// registry, toward the marker. Reports what it would change; --yes
    /// changes it.
    Heal {
        /// Make the changes.
        #[arg(long)]
        yes: bool,
    },
    /// Bind this directory to an existing project and its database. The id
    /// it had before keeps its database. Reports what it would change;
    /// --yes changes it.
    Repoint {
        /// The project to bind to.
        project_id: String,
        /// Bind it even though the project's registered tree still exists:
        /// the two trees are meant to share it.
        #[arg(long)]
        share: bool,
        /// Make the changes.
        #[arg(long)]
        yes: bool,
    },
    /// List the databases no live registered path points at.
    Orphans {
        #[command(subcommand)]
        action: Option<OrphansAction>,
    },
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum OrphansAction {
    /// Delete orphaned databases, each named. Reports what it would
    /// delete; --yes deletes it.
    Remove {
        #[arg(required = true, num_args = 1..)]
        project_ids: Vec<String>,
        /// Delete them.
        #[arg(long)]
        yes: bool,
    },
}

/// What the mounting binary parsed for the group.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Settings {
    /// JSON (one document) rather than text.
    pub json: bool,
    /// `--project`: the id `show` reports, overriding the directory's.
    pub project: Option<String>,
    /// The program's name: every diagnostic's prefix.
    pub prog: &'static str,
}

/// Run `group` against `projects`, writing to `out` and `err`; the exit
/// code.
pub fn run<A: ProjectAdmin>(
    group: Group,
    settings: &Settings,
    projects: &mut Projects<A>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let Group::Project(command) = group;
    match answer(command, settings, projects, out) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(err, "{}: {error}", settings.prog);
            1
        }
    }
}

fn answer<A: ProjectAdmin>(
    command: ProjectCommand,
    settings: &Settings,
    projects: &mut Projects<A>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let json = settings.json;
    let wrote = match command {
        ProjectCommand::Show => {
            let shown = projects
                .show(settings.project.as_deref())
                .map_err(CliError::Project)?;
            if json {
                render::json(out, &ShowJson::of(&shown))
            } else {
                render::show_text(out, &shown)
            }
        }
        ProjectCommand::Heal { yes } => {
            let plan = projects.heal(yes).map_err(CliError::Project)?;
            planned(out, json, &plan)
        }
        ProjectCommand::Repoint {
            project_id,
            share,
            yes,
        } => {
            let plan = projects
                .repoint(&project_id, share, yes)
                .map_err(CliError::Project)?;
            planned(out, json, &plan)
        }
        ProjectCommand::Orphans { action: None } => {
            let orphans = projects.orphans().map_err(CliError::Project)?;
            if json {
                render::json(out, &render::orphans_json(&orphans))
            } else {
                render::orphans_text(out, &orphans)
            }
        }
        ProjectCommand::Orphans {
            action: Some(OrphansAction::Remove { project_ids, yes }),
        } => {
            let plan = projects
                .remove_orphans(&project_ids, yes)
                .map_err(CliError::Project)?;
            planned(out, json, &plan)
        }
    };
    wrote.map_err(output_failed)
}

fn planned(out: &mut dyn Write, json: bool, plan: &project_application::Plan) -> io::Result<()> {
    if json {
        render::json(out, &PlanJson::of(plan))
    } else {
        render::plan_text(out, plan)
    }
}

fn output_failed(error: io::Error) -> CliError {
    CliError::Output {
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
