//! The `project` command group: `show`, `heal`, `repoint` and `orphans`,
//! translated into the project use cases and their answers rendered
//! through presentation, as text or one JSON document. It depends on no
//! tasks crate, so any binary mounts it: taskloom does, beside its task
//! commands, and ctxloom can.
//!
//! A binary flattens [`Group`] into its own subcommands, parses its own
//! global flags (the format, the pinned project), and hands the group,
//! those and its catalog (with this crate's [`EN_US`], the use cases' and
//! the store's, whose damage `heal` reports) to [`run`]. Exit 0, or 1 with the error on stderr: its catalog message
//! in text, its envelope in JSON.

mod error;
mod render;

use std::io::{self, Write};

use clap::Subcommand;
use presentation::{Catalog, Format};
use project_application::{ProjectAdmin, Projects, StoreCheck};

pub use error::{diagnostic_samples, CliError, EN_US};
pub use render::{
    change_line, problem_line, ChangeJson, EntryJson, OrphanJson, Orphans, PlanJson, Planned,
    ProblemJson, Show, ShowJson,
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
    /// entries and database, the other trees sharing it (a second clone
    /// with the same committed .project-id is the same project, by
    /// design), and every inconsistency found.
    Show,
    /// Repair the skew between this directory's .project-id and the
    /// registry, toward the marker (registering this tree beside any other
    /// sharing its id), and check the project's database for damage, which
    /// it reports and never repairs. Reports what it would change; --yes
    /// changes it.
    Heal {
        /// Make the changes.
        #[arg(long)]
        yes: bool,
    },
    /// Bind this directory to an existing project and its database. The id
    /// it had before keeps its database. Moving onto a project another
    /// existing tree holds needs --share. Reports what it would change;
    /// --yes changes it.
    Repoint {
        /// The project to bind to.
        project_id: String,
        /// Bind it even though another existing tree holds the project:
        /// the two trees are meant to share it, as clones with one
        /// .project-id do.
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
        /// The orphaned databases' project ids.
        #[arg(required = true, num_args = 1..)]
        project_ids: Vec<String>,
        /// Delete them.
        #[arg(long)]
        yes: bool,
    },
}

/// What the mounting binary parsed for the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Text, or JSON (one document).
    pub format: Format,
    /// `--project`: the id `show` reports, overriding the directory's.
    pub project: Option<String>,
    /// The program's name: every diagnostic's prefix.
    pub prog: &'static str,
}

/// Run `group` against `projects`, writing to `out` and `err` with
/// messages from `catalog`; the exit code.
pub fn run<A: ProjectAdmin, C: StoreCheck>(
    group: Group,
    settings: &Settings,
    catalog: &Catalog,
    projects: &mut Projects<A, C>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let Group::Project(command) = group;
    match answer(command, settings, catalog, projects, out) {
        Ok(()) => 0,
        Err(error) => {
            let _ = report(&error, settings, catalog, err);
            1
        }
    }
}

/// `error` on stderr: its envelope in JSON; in text its catalog message
/// after the program's name.
fn report(
    error: &CliError,
    settings: &Settings,
    catalog: &Catalog,
    err: &mut dyn Write,
) -> io::Result<()> {
    match settings.format {
        Format::Json => presentation::write_json(err, &catalog.envelope(error)),
        Format::Text => writeln!(err, "{}: {}", settings.prog, catalog.message(error)),
    }
}

fn answer<A: ProjectAdmin, C: StoreCheck>(
    command: ProjectCommand,
    settings: &Settings,
    catalog: &Catalog,
    projects: &mut Projects<A, C>,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let wrote = match command {
        ProjectCommand::Show => {
            let shown = projects
                .show(settings.project.as_deref())
                .map_err(CliError::Project)?;
            presentation::render(out, settings.format, &Show(&shown, catalog))
        }
        ProjectCommand::Heal { yes } => {
            let plan = projects.heal(yes).map_err(CliError::Project)?;
            presentation::render(out, settings.format, &Planned(&plan, catalog))
        }
        ProjectCommand::Repoint {
            project_id,
            share,
            yes,
        } => {
            let plan = projects
                .repoint(&project_id, share, yes)
                .map_err(CliError::Project)?;
            presentation::render(out, settings.format, &Planned(&plan, catalog))
        }
        ProjectCommand::Orphans { action: None } => {
            let orphans = projects.orphans().map_err(CliError::Project)?;
            presentation::render(out, settings.format, &Orphans(&orphans))
        }
        ProjectCommand::Orphans {
            action: Some(OrphansAction::Remove { project_ids, yes }),
        } => {
            let plan = projects
                .remove_orphans(&project_ids, yes)
                .map_err(CliError::Project)?;
            presentation::render(out, settings.format, &Planned(&plan, catalog))
        }
    };
    wrote.map_err(output_failed)
}

fn output_failed(error: io::Error) -> CliError {
    CliError::Output {
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
