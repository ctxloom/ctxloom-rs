//! Project identity: which project a directory is, and where its store is.
//!
//! The work root is `CTXLOOM_ROOT`, else the nearest ancestor holding
//! `.git`, else the working directory; canonical, as Go's `cleanPath` makes
//! it (symlinks resolved, else cleaned lexically), and registered so.
//! Registry entries are compared canonical too. Its project id is, in order: the
//! one pinned (`--project`), `CTXLOOM_PROJECT_ID`, the marker
//! `<root>/.project-id`, Go's older marker `<root>/.ctxloom/project-id`
//! (adopted into `.project-id` on a write), and the registry entry for the
//! root's path. A write with none of these mints an id, under the
//! registry's lock, and binds it in both the registry and the marker; a read
//! with none finds no project. The store is
//! `~/.ctxloom/projects/<id>/state.db`.
//!
//! Healing a marker and registry that disagree, and repointing a directory
//! at another id, are not here yet.

mod error;
mod id;
mod index;
mod path;

use std::fs;
use std::path::{Path, PathBuf};

use annals_diagnostic::Report;
use ports::{Names, Project, ProjectIdentity};

pub use error::{diagnostic_samples, RegistryError, EN_US};

/// How many project ids a mint draws before giving up, each already
/// registered: Go's `harp.UniqueFrom`.
pub const MINT_ATTEMPTS: u32 = 100;

/// What resolution reads from the process: its environment and where it
/// runs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Environment {
    /// HOME.
    pub home: Option<PathBuf>,
    /// The working directory, or why it cannot be read (it was deleted).
    pub cwd: Result<PathBuf, String>,
    /// CTXLOOM_ROOT: the work root, overriding the search for `.git`.
    pub root: Option<PathBuf>,
    /// CTXLOOM_PROJECT_ID: the project, overriding the work root's.
    pub project_id: Option<String>,
}

/// Project identity over the filesystem, minting ids from `names`.
pub struct ProjectRegistry<N> {
    env: Environment,
    names: N,
}

impl<N: Names> ProjectRegistry<N> {
    pub fn new(env: Environment, names: N) -> Self {
        ProjectRegistry { env, names }
    }

    /// The work root, canonical: CTXLOOM_ROOT (under the working directory
    /// if relative), else the nearest ancestor of the working directory
    /// holding `.git` (a directory, or a linked worktree's file), else the
    /// working directory.
    pub fn work_root(&self) -> Result<PathBuf, RegistryError> {
        if let Some(root) = self.env.root.as_ref().filter(|r| !r.as_os_str().is_empty()) {
            if root.is_absolute() {
                return Ok(path::canonical(root));
            }
            return Ok(path::canonical(&self.cwd()?.join(root)));
        }
        let cwd = self.cwd()?;
        let root = cwd
            .ancestors()
            .find(|dir| dir.join(".git").exists())
            .unwrap_or(cwd);
        Ok(path::canonical(root))
    }

    fn cwd(&self) -> Result<&Path, RegistryError> {
        self.env
            .cwd
            .as_deref()
            .map_err(|detail| RegistryError::NoWorkingDirectory {
                detail: detail.clone(),
            })
    }

    fn home(&self) -> Result<&Path, RegistryError> {
        self.env
            .home
            .as_deref()
            .filter(|h| !h.as_os_str().is_empty())
            .ok_or(RegistryError::NoHome)
    }

    /// The project `id` names, resolved for `root`.
    fn project(&self, id: String, root: &Path) -> Result<Project, RegistryError> {
        let store = self
            .home()?
            .join(".ctxloom")
            .join("projects")
            .join(&id)
            .join("state.db");
        Ok(Project {
            id,
            root: root.display().to_string(),
            store,
        })
    }

    /// The id given explicitly: the pin, else CTXLOOM_PROJECT_ID.
    fn pinned(&self, pinned: Option<&str>) -> Result<Option<String>, RegistryError> {
        if let Some(id) = pinned.filter(|p| !p.is_empty()) {
            return id::valid(id, "--project").map(Some);
        }
        match self.env.project_id.as_deref().filter(|p| !p.is_empty()) {
            Some(id) => id::valid(id, "CTXLOOM_PROJECT_ID").map(Some),
            None => Ok(None),
        }
    }

    /// The id the work root is already bound to, adopting Go's older marker
    /// into `.project-id` when `adopt`.
    fn bound(&self, root: &Path, adopt: bool) -> Result<Option<String>, RegistryError> {
        if let Some(id) = read_marker(&marker(root))? {
            return Ok(Some(id));
        }
        if let Some(id) = read_marker(&legacy_marker(root))? {
            if adopt {
                write_marker(root, &id)?;
            }
            return Ok(Some(id));
        }
        let home = self.home()?;
        let index = index::load(&index::path(home))?;
        Ok(index.id_for(root).map(str::to_string))
    }

    /// Bind the work root to a project id under the registry's lock: the
    /// one another process bound while this one waited, else a new one.
    fn bind(&mut self, root: &Path) -> Result<String, RegistryError> {
        let path = index::path(self.home()?);
        let _locked = index::lock(&path)?;
        if let Some(id) = self.bound(root, true)? {
            return Ok(id);
        }
        let mut index = index::load(&path)?;
        let id = self.mint(&index)?;
        index.register(&id, &root.display().to_string());
        index::save(&path, &index)?;
        write_marker(root, &id)?;
        Ok(id)
    }

    /// A project id `index` does not hold.
    fn mint(&mut self, index: &index::Index) -> Result<String, RegistryError> {
        for _ in 0..MINT_ATTEMPTS {
            let id = self.names.project_id();
            if !index.holds(&id) {
                return Ok(id);
            }
        }
        Err(RegistryError::Exhausted {
            attempts: MINT_ATTEMPTS,
        })
    }

    fn resolve(
        &mut self,
        pinned: Option<&str>,
        write: bool,
    ) -> Result<Option<Project>, RegistryError> {
        let root = self.work_root()?;
        let id = match self.pinned(pinned)? {
            Some(id) => Some(id),
            None => match self.bound(&root, write)? {
                Some(id) => Some(id),
                None if write => Some(self.bind(&root)?),
                None => None,
            },
        };
        id.map(|id| self.project(id, &root)).transpose()
    }
}

impl<N: Names> ProjectIdentity for ProjectRegistry<N> {
    fn for_write(&mut self, pinned: Option<&str>) -> Result<Project, Report> {
        match self.resolve(pinned, true) {
            Ok(Some(project)) => Ok(project),
            Ok(None) => unreachable!("a write binds a project"),
            Err(e) => Err(Report::of(&e)),
        }
    }

    fn for_read(&mut self, pinned: Option<&str>) -> Result<Option<Project>, Report> {
        self.resolve(pinned, false).map_err(|e| Report::of(&e))
    }
}

/// The marker naming `root`'s project.
fn marker(root: &Path) -> PathBuf {
    root.join(".project-id")
}

/// Go's older, private marker.
fn legacy_marker(root: &Path) -> PathBuf {
    root.join(".ctxloom").join("project-id")
}

/// The id a marker at `path` holds; none if it is absent or blank.
fn read_marker(path: &Path) -> Result<Option<String>, RegistryError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(RegistryError::io(path, e)),
    };
    match text.trim() {
        "" => Ok(None),
        id => id::valid(id, &path.display().to_string()).map(Some),
    }
}

fn write_marker(root: &Path, id: &str) -> Result<(), RegistryError> {
    index::write_atomically(&marker(root), format!("{id}\n").as_bytes())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
