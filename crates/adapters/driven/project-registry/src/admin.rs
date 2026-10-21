//! The registry, the markers and the projects directory behind the
//! [`ProjectAdmin`] port: surveyed as resolution reads them, and changed
//! under the registry's lock only while a fresh survey still matches the
//! one the changes were planned from.

use std::fs;
use std::path::{Path, PathBuf};

use annals_diagnostic::Report;
use ports::{Change, Names, Project, ProjectAdmin, Registered, Survey};

use crate::path::canonical;
use crate::{
    id, index, legacy_marker, marker, read_marker, write_marker, ProjectRegistry, RegistryError,
};

impl<N: Names> ProjectRegistry<N> {
    /// The directory projects' databases are kept in.
    fn projects_dir(&self) -> Result<PathBuf, RegistryError> {
        Ok(self.home()?.join(".ctxloom").join("projects"))
    }

    /// What the working directory's identity rests on.
    pub(crate) fn survey_of(&self, pinned: Option<&str>) -> Result<Survey, RegistryError> {
        let work_root = self.work_root()?;
        let root = self.identity_root()?;
        let resolved = match self.pinned(pinned)? {
            Some(pin) => Some(pin),
            None => self.bound(&root, false)?,
        };
        let projects = self.projects_dir()?;
        let registry = index::load(&projects.join("index.yaml"))?
            .projects
            .into_iter()
            .map(|e| Registered {
                canonical: canonical(Path::new(&e.path)).display().to_string(),
                exists: Path::new(&e.path).is_dir(),
                id: e.project_id,
                path: e.path,
            })
            .collect();
        Ok(Survey {
            work_root: work_root.display().to_string(),
            root: root.display().to_string(),
            resolved,
            marker: read_marker(&marker(&root))?,
            legacy_marker: read_marker(&legacy_marker(&root))?,
            registry,
            databases: databases(&projects)?,
            projects,
        })
    }

    /// Every project with a database directory, by id, each rooted where
    /// the registry first holds it.
    pub(crate) fn every(&self) -> Result<Vec<Project>, RegistryError> {
        let projects = self.projects_dir()?;
        let registry = index::load(&projects.join("index.yaml"))?;
        databases(&projects)?
            .into_iter()
            .map(|id| {
                let root = registry
                    .projects
                    .iter()
                    .find(|e| e.project_id == id)
                    .map(|e| canonical(Path::new(&e.path)).display().to_string())
                    .unwrap_or_default();
                let store = projects.join(&id).join("state.db");
                Ok(Project { id, root, store })
            })
            .collect()
    }

    fn apply_changes(
        &self,
        pinned: Option<&str>,
        seen: &Survey,
        changes: &[Change],
    ) -> Result<(), RegistryError> {
        let projects = self.projects_dir()?;
        let path = projects.join("index.yaml");
        let _locked = index::lock_within(&path, self.lock_wait)?;
        if self.survey_of(pinned)? != *seen {
            return Err(RegistryError::Changed);
        }
        let mut registry = index::load(&path)?;
        let mut registry_changed = false;
        for change in changes {
            match change {
                Change::Register { id, path } => {
                    registry.register(&id::valid(id, "a change")?, path);
                    registry_changed = true;
                }
                Change::Unregister { id, path } => {
                    registry.projects.retain(|e| {
                        !(e.project_id == *id && canonical(Path::new(&e.path)) == Path::new(path))
                    });
                    registry_changed = true;
                }
                Change::WriteMarker { .. } | Change::RemoveDatabase { .. } => {}
            }
        }
        if registry_changed {
            index::save(&path, &registry)?;
        }
        for change in changes {
            match change {
                Change::WriteMarker { root, id } => {
                    write_marker(Path::new(root), &id::valid(id, "a change")?)?;
                }
                Change::RemoveDatabase { id } => {
                    remove_database(&projects, &id::valid(id, "a change")?)?;
                }
                Change::Register { .. } | Change::Unregister { .. } => {}
            }
        }
        Ok(())
    }
}

impl<N: Names> ProjectAdmin for ProjectRegistry<N> {
    fn survey(&mut self, pinned: Option<&str>) -> Result<Survey, Report> {
        self.survey_of(pinned).map_err(|e| Report::of(&e))
    }

    /// The registry's changes are saved first, in one write; then the
    /// markers are written and the databases removed, in order.
    fn apply(
        &mut self,
        pinned: Option<&str>,
        seen: &Survey,
        changes: &[Change],
    ) -> Result<(), Report> {
        self.apply_changes(pinned, seen, changes)
            .map_err(|e| Report::of(&e))
    }
}

/// The ids with a directory of their own under `projects`, sorted: what is
/// not a directory (the registry, its lock, a symlink) or not named as a
/// project id is left out.
fn databases(projects: &Path) -> Result<Vec<String>, RegistryError> {
    let listing = match fs::read_dir(projects) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(RegistryError::io(projects, e)),
    };
    let mut ids = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|e| RegistryError::io(projects, e))?;
        let is_dir = entry
            .file_type()
            .map_err(|e| RegistryError::io(&entry.path(), e))?
            .is_dir();
        let name = entry.file_name().to_string_lossy().to_string();
        if is_dir && id::valid(&name, "").is_ok() {
            ids.push(name);
        }
    }
    ids.sort_unstable();
    Ok(ids)
}

/// Delete `<projects>/<id>/`, which must be a directory of its own, not a
/// link to one elsewhere.
fn remove_database(projects: &Path, id: &str) -> Result<(), RegistryError> {
    let dir = projects.join(id);
    let is_dir = fs::symlink_metadata(&dir)
        .map_err(|e| RegistryError::io(&dir, e))?
        .is_dir();
    if !is_dir {
        return Err(RegistryError::NotADatabase {
            path: dir.display().to_string(),
        });
    }
    fs::remove_dir_all(&dir).map_err(|e| RegistryError::io(&dir, e))
}

#[cfg(test)]
#[path = "admin.test.rs"]
mod tests;
