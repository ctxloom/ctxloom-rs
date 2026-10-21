//! What a directory's project identity rests on, and the changes that
//! repair or rebind it, and whether a project's database is sound: the
//! ports behind the `project` commands.

use std::path::{Path, PathBuf};

use annals_diagnostic::Report;

/// Where a resolved project id came from, in the order resolution tries
/// them.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IdSource {
    /// `--project`.
    Flag,
    /// CTXLOOM_PROJECT_ID.
    Environment,
    /// `<root>/.project-id`.
    Marker,
    /// Go's older `<root>/.ctxloom/project-id`.
    LegacyMarker,
    /// The registry's entry for the root's path.
    Registry,
}

impl IdSource {
    /// A stable name, for output.
    pub fn as_str(self) -> &'static str {
        match self {
            IdSource::Flag => "flag",
            IdSource::Environment => "environment",
            IdSource::Marker => "marker",
            IdSource::LegacyMarker => "legacy-marker",
            IdSource::Registry => "registry",
        }
    }
}

/// One entry of the registry, and what is at its path now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registered {
    pub id: String,
    /// The path as the registry holds it.
    pub path: String,
    /// The path compared canonical, as resolution compares it.
    pub canonical: String,
    /// Whether a directory is at the path now.
    pub exists: bool,
}

/// What the identity of the directory a command runs in rests on: read,
/// never written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Survey {
    /// The work root, canonical.
    pub work_root: String,
    /// Where its identity is kept, canonical: the work root, or a linked
    /// worktree's primary checkout when the worktree has no marker of its
    /// own.
    pub root: String,
    /// The id resolution gives, and where it came from; `None` when it
    /// finds none (a write would mint one).
    pub resolved: Option<(String, IdSource)>,
    /// The id in `<root>/.project-id`, if it holds one.
    pub marker: Option<String>,
    /// The id in Go's older `<root>/.ctxloom/project-id`, if it holds one.
    pub legacy_marker: Option<String>,
    /// Every registry entry, in the registry's order.
    pub registry: Vec<Registered>,
    /// The ids with a directory under [`Survey::projects`], sorted.
    pub databases: Vec<String>,
    /// The directory projects' databases are kept in
    /// (`~/.ctxloom/projects`).
    pub projects: PathBuf,
}

impl Survey {
    /// The database of the project `id`.
    pub fn store(&self, id: &str) -> PathBuf {
        self.projects.join(id).join("state.db")
    }

    /// The registry's entries for the canonical path `path`.
    pub fn entries_at<'s>(&'s self, path: &'s str) -> impl Iterator<Item = &'s Registered> + 's {
        self.registry.iter().filter(move |e| e.canonical == path)
    }

    /// The registry's entries for `id`.
    pub fn entries_of<'s>(&'s self, id: &'s str) -> impl Iterator<Item = &'s Registered> + 's {
        self.registry.iter().filter(move |e| e.id == id)
    }
}

/// One change to a directory's binding. A database is never rewritten:
/// binding a directory elsewhere changes only its marker and the registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Write `id` into `<root>/.project-id`.
    WriteMarker { root: String, id: String },
    /// Register `id` for the canonical path `path`.
    Register { id: String, path: String },
    /// Remove every registry entry for `id` whose path is `path`,
    /// canonical.
    Unregister { id: String, path: String },
    /// Delete the project's directory under the projects directory, its
    /// database with it.
    RemoveDatabase { id: String },
}

/// The registry, the markers and the projects directory, surveyed and
/// changed.
///
/// Synchronous, and it blocks on the filesystem and the registry's lock,
/// as [`crate::ProjectIdentity`] does.
pub trait ProjectAdmin {
    /// What the working directory's identity rests on; `pinned` is
    /// `--project`.
    fn survey(&mut self, pinned: Option<&str>) -> Result<Survey, Report>;

    /// Make `changes`, in order, under the registry's lock, if a survey
    /// taken under it (with `pinned`) is still `seen`; refused, changing
    /// nothing, if anything changed since.
    fn apply(
        &mut self,
        pinned: Option<&str>,
        seen: &Survey,
        changes: &[Change],
    ) -> Result<(), Report>;
}

/// What checking a project's database found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checked {
    /// No database is there: nothing to check, and no damage.
    Absent,
    /// The database reported no problem.
    Intact,
    /// The database reported problems in its own file: the store's
    /// diagnostic of them, its code and args.
    Damaged(Report),
}

/// A project's database, checked whole for damage, and never repaired.
///
/// Synchronous, and it blocks on the database, as [`ProjectAdmin`] does.
pub trait StoreCheck {
    /// The database at `store`, checked as it was last committed, opened
    /// read-only: nothing is created, migrated or written, and no writer
    /// is waited on. A failure to run the check is the error, never
    /// damage.
    fn check(&mut self, store: &Path) -> Result<Checked, Report>;
}

#[cfg(test)]
#[path = "project.test.rs"]
mod tests;
