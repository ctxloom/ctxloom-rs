//! Project identity: which project a directory is, and where its store is.
//!
//! The work root is `CTXLOOM_ROOT`, else the nearest ancestor holding
//! `.git`, else the working directory; canonical, as Go's `cleanPath` makes
//! it (symlinks resolved, else cleaned lexically), and registered so.
//! Registry entries are compared canonical too. Its project id is, in order: the
//! one pinned (`--project`), `CTXLOOM_PROJECT_ID`, the marker
//! `<root>/.project-id`, Go's older marker `<root>/.ctxloom/project-id`
//! (adopted into `.project-id` on a write), and the registry entry for the
//! root's path. A linked worktree with no marker of its own (neither) is
//! its primary checkout's project, found through git's common directory:
//! resolved, bound and marked there, so no marker is written in the
//! worktree to be committed and merged back over the primary's. A write with none of these mints an id, under the
//! registry's lock, and binds it in both the registry and the marker; a read
//! with none finds no project. The store is
//! `~/.ctxloom/projects/<id>/state.db`.
//!
//! The same registry is behind the `ProjectAdmin` port: a survey of what
//! a directory's identity rests on, and changes to markers, the registry
//! and the projects directory, made under the registry's lock only while
//! a fresh survey matches the one they were planned from. The lock is
//! file-lock's: taken again when what it was granted is no longer the file
//! at its path (`ctxloom clean` reaps lock files), and waited for at most
//! [`LOCK_WAIT`].

mod admin;
mod error;
mod id;
mod index;
mod path;
mod worktree;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use annals_diagnostic::Report;
use ports::{IdSource, Names, Project, ProjectIdentity};

pub use error::{diagnostic_samples, RegistryError, EN_US};
pub use index::LOCK_WAIT;

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
    /// How long a change waits for the registry's lock.
    lock_wait: Duration,
}

impl<N: Names> ProjectRegistry<N> {
    pub fn new(env: Environment, names: N) -> Self {
        ProjectRegistry {
            env,
            names,
            lock_wait: index::LOCK_WAIT,
        }
    }

    /// The same, waiting up to `wait` for the registry's lock rather than
    /// [`LOCK_WAIT`].
    pub fn with_lock_wait(self, wait: Duration) -> Self {
        ProjectRegistry {
            lock_wait: wait,
            ..self
        }
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

    /// The working directory, canonical, if neither CTXLOOM_ROOT nor a
    /// `.git` above it gives it a work root of its own.
    fn rootless(&self) -> Result<Option<String>, RegistryError> {
        if self
            .env
            .root
            .as_ref()
            .is_some_and(|r| !r.as_os_str().is_empty())
        {
            return Ok(None);
        }
        let cwd = self.cwd()?;
        if cwd.ancestors().any(|dir| dir.join(".git").exists()) {
            return Ok(None);
        }
        Ok(Some(path::canonical(cwd).display().to_string()))
    }

    fn cwd(&self) -> Result<&Path, RegistryError> {
        self.env
            .cwd
            .as_deref()
            .map_err(|detail| RegistryError::NoWorkingDirectory {
                detail: detail.clone(),
            })
    }

    pub(crate) fn home(&self) -> Result<&Path, RegistryError> {
        self.env
            .home
            .as_deref()
            .filter(|h| !h.as_os_str().is_empty())
            .ok_or(RegistryError::NoHome)
    }

    /// The directory the work root's identity is kept in: the work root
    /// itself when it has a marker of its own or is no linked worktree,
    /// else its primary checkout, so a linked worktree is its primary
    /// checkout's project and writes no marker of its own.
    pub fn identity_root(&self) -> Result<PathBuf, RegistryError> {
        let root = self.work_root()?;
        if has_marker(&root) {
            return Ok(root);
        }
        Ok(worktree::primary_checkout(&root)?.unwrap_or(root))
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
    pub(crate) fn pinned(
        &self,
        pinned: Option<&str>,
    ) -> Result<Option<(String, IdSource)>, RegistryError> {
        if let Some(id) = pinned.filter(|p| !p.is_empty()) {
            return id::valid(id, "--project").map(|id| Some((id, IdSource::Flag)));
        }
        match self.env.project_id.as_deref().filter(|p| !p.is_empty()) {
            Some(id) => {
                id::valid(id, "CTXLOOM_PROJECT_ID").map(|id| Some((id, IdSource::Environment)))
            }
            None => Ok(None),
        }
    }

    /// The id the work root is already bound to, and what binds it,
    /// adopting Go's older marker into `.project-id` when `adopt`.
    pub(crate) fn bound(
        &self,
        root: &Path,
        adopt: bool,
    ) -> Result<Option<(String, IdSource)>, RegistryError> {
        if let Some(id) = read_marker(&marker(root))? {
            return Ok(Some((id, IdSource::Marker)));
        }
        if let Some(id) = read_marker(&legacy_marker(root))? {
            if adopt {
                write_marker(root, &id)?;
            }
            return Ok(Some((id, IdSource::LegacyMarker)));
        }
        let home = self.home()?;
        let index = index::load(&index::path(home))?;
        Ok(index
            .id_for(root)
            .map(|id| (id.to_string(), IdSource::Registry)))
    }

    /// Bind the work root to a project id under the registry's lock: the
    /// one another process bound while this one waited, else a new one.
    fn bind(&mut self, root: &Path) -> Result<String, RegistryError> {
        let path = index::path(self.home()?);
        let _locked = index::lock_within(&path, self.lock_wait)?;
        if let Some((id, _)) = self.bound(root, true)? {
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
        let root = self.identity_root()?;
        let id = match self.pinned(pinned)? {
            Some((id, _)) => Some(id),
            None => match self.bound(&root, write)? {
                Some((id, _)) => Some(id),
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

    fn every_project(&mut self) -> Result<Vec<Project>, Report> {
        self.every().map_err(|e| Report::of(&e))
    }

    fn rootless_directory(&mut self) -> Result<Option<String>, Report> {
        self.rootless().map_err(|e| Report::of(&e))
    }
}

/// The marker naming `root`'s project.
pub(crate) fn marker(root: &Path) -> PathBuf {
    root.join(".project-id")
}

/// Go's older, private marker.
pub(crate) fn legacy_marker(root: &Path) -> PathBuf {
    root.join(".ctxloom").join("project-id")
}

/// Whether `root` holds a marker, either one, blank or not: a marker is
/// the tree's own say in its identity.
fn has_marker(root: &Path) -> bool {
    marker(root).exists() || legacy_marker(root).exists()
}

/// The id a marker at `path` holds; none if it is absent or blank.
pub(crate) fn read_marker(path: &Path) -> Result<Option<String>, RegistryError> {
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

pub(crate) fn write_marker(root: &Path, id: &str) -> Result<(), RegistryError> {
    index::write_atomically(&marker(root), format!("{id}\n").as_bytes())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
