//! The project use cases: what a directory's identity rests on and where
//! it is inconsistent (`show`), repairing marker and registry skew and
//! checking the project's database for damage (`heal`), binding a directory to an existing project (`repoint`), and
//! the databases no live registered path points at (`orphans`, removed
//! only by name).
//!
//! The marker is authoritative: where `.project-id` and the registry
//! disagree, the registry heals toward the marker. A change is planned
//! here from a [`Survey`], shown, and made through [`ProjectAdmin::apply`]
//! only when asked, which refuses if the survey is stale. A database is
//! never rewritten: binding a directory elsewhere changes its marker and
//! the registry, and the database it was bound to stays, orphaned. Nor is
//! it repaired: damage [`StoreCheck`] finds in it is reported, never
//! changed.
//!
//! Nothing here depends on tasks, so any binary can present these.

mod error;

use std::path::PathBuf;

use annals_diagnostic::Report;

pub use error::{diagnostic_samples, ProjectError, EN_US};
pub use ports::{Change, Checked, IdSource, ProjectAdmin, Registered, StoreCheck, Survey};

/// Something about a directory's identity, or the registry, that is not as
/// it should be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The root is bound to more than one id: registered under several, or
    /// under one its marker does not name.
    TwoIds { root: String, ids: Vec<String> },
    /// The root is registered but has no marker.
    MissingMarker { root: String, id: String },
    /// Only Go's older marker names the root's id.
    LegacyMarkerOnly { root: String, id: String },
    /// The marker's id is not registered for the root (whether or not
    /// another tree holding it is: clones share by default).
    Unregistered { root: String, id: String },
    /// A registered path no longer exists.
    Gone { id: String, path: String },
    /// A database no live registered path points at.
    Orphan { id: String, database: PathBuf },
    /// The project's database reported damage in its own file: the
    /// store's diagnostic of it. Nothing here repairs it.
    Damaged {
        id: String,
        database: PathBuf,
        damage: Report,
    },
}

/// What `project show` shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub survey: Survey,
    /// The registry's entries for the root.
    pub entries: Vec<Registered>,
    /// The resolved project's database, if there is a project.
    pub store: Option<PathBuf>,
    /// The other live trees registered for the resolved project, as the
    /// registry holds their paths: a second clone with the same committed
    /// `.project-id` is the same project and shares its database, by
    /// design. Information, not a problem.
    pub shared_with: Vec<String>,
    pub problems: Vec<Problem>,
}

/// What a change would do, or did.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Plan {
    /// The changes, in the order they are made.
    pub changes: Vec<Change>,
    /// Whether they were made: asked to, and something to make.
    pub applied: bool,
    /// What it found and cannot repair on its own.
    pub unresolved: Vec<Problem>,
    /// For a repoint, the id the directory resolved to before.
    pub previous: Option<String>,
}

/// The project use cases, over the ports: the registry and markers, and
/// the check of a project's database.
pub struct Projects<A, C> {
    admin: A,
    checks: C,
}

impl<A: ProjectAdmin, C: StoreCheck> Projects<A, C> {
    pub fn new(admin: A, checks: C) -> Self {
        Projects { admin, checks }
    }

    /// The registry's port it runs over.
    pub fn admin(&self) -> &A {
        &self.admin
    }

    /// The database check it runs over.
    pub fn checks(&self) -> &C {
        &self.checks
    }

    /// What the working directory's identity rests on, and every
    /// inconsistency found.
    pub fn show(&mut self, pinned: Option<&str>) -> Result<Shown, ProjectError> {
        let survey = self.admin.survey(pinned).map_err(ProjectError::Port)?;
        let entries = survey.entries_at(&survey.root).cloned().collect();
        let store = survey.resolved.as_ref().map(|(id, _)| survey.store(id));
        let shared_with = match &survey.resolved {
            Some((id, _)) => survey
                .entries_of(id)
                .filter(|e| e.exists && e.canonical != survey.root)
                .map(|e| e.path.clone())
                .collect(),
            None => Vec::new(),
        };
        let problems = problems(&survey);
        Ok(Shown {
            survey,
            entries,
            store,
            shared_with,
            problems,
        })
    }

    /// Repair the skew between the root's marker and the registry, toward
    /// the marker: register what the marker names (dropping its entries
    /// whose paths are gone, a moved tree's; another live tree's entry for
    /// it stays, since clones share by default), unregister what else the
    /// root is registered as, write a missing marker from the registry's
    /// one entry, and adopt Go's older marker. Made only when `apply`.
    ///
    /// The resolved project's database is checked first, read-only, and
    /// damage found is left unresolved: reported, never repaired. No
    /// database is no damage. A check that cannot run fails the heal
    /// before anything is changed.
    pub fn heal(&mut self, apply: bool) -> Result<Plan, ProjectError> {
        let survey = self.admin.survey(None).map_err(ProjectError::Port)?;
        let damaged = self.damage(&survey)?;
        let root = survey.root.as_str();
        let at_root = ids_at(&survey, root);
        let mut plan = Plan::default();
        match survey.marker.as_ref().or(survey.legacy_marker.as_ref()) {
            Some(id) => {
                if survey.marker.is_none() {
                    plan.changes.push(write_marker(root, id));
                }
                plan.changes.extend(bind(&survey, root, id));
            }
            None => match at_root.as_slice() {
                [id] => plan.changes.push(write_marker(root, id)),
                [] => {}
                ids => plan.unresolved.push(Problem::TwoIds {
                    root: root.to_string(),
                    ids: ids.to_vec(),
                }),
            },
        }
        plan.unresolved.extend(damaged);
        self.make(&survey, plan, apply)
    }

    /// The damage the check finds in the resolved project's database; none
    /// when there is no project or no database.
    fn damage(&mut self, survey: &Survey) -> Result<Option<Problem>, ProjectError> {
        let Some((id, _)) = &survey.resolved else {
            return Ok(None);
        };
        let database = survey.store(id);
        let checked = self.checks.check(&database).map_err(ProjectError::Port)?;
        Ok(match checked {
            Checked::Damaged(damage) => Some(Problem::Damaged {
                id: id.clone(),
                database,
                damage,
            }),
            Checked::Absent | Checked::Intact => None,
        })
    }

    /// Bind the work root to the existing project `id`: its marker, and
    /// the registry. A move onto an id another live tree holds is refused
    /// unless `share`: clones with one committed marker share by default,
    /// but a tree that is some other project (or none) taking over a live
    /// tree's id is merging two projects' work, which `share` says is
    /// meant. A tree whose marker or registry already gives it `id` moves
    /// nowhere, and is not refused. The id it had before keeps its
    /// database. Made only when `apply`.
    pub fn repoint(&mut self, id: &str, share: bool, apply: bool) -> Result<Plan, ProjectError> {
        let survey = self.admin.survey(None).map_err(ProjectError::Port)?;
        if survey.entries_of(id).next().is_none() && !survey.databases.iter().any(|d| d == id) {
            return Err(ProjectError::UnknownProject { id: id.to_string() });
        }
        let root = survey.work_root.as_str();
        let already = survey.resolved.as_ref().is_some_and(|(resolved, source)| {
            resolved == id && !matches!(source, IdSource::Flag | IdSource::Environment)
        });
        if let (Some(path), false) = (held_elsewhere(&survey, root, id), share || already) {
            return Err(ProjectError::InUse {
                id: id.to_string(),
                path,
            });
        }
        let marked = (survey.root == survey.work_root)
            .then_some(survey.marker.as_deref())
            .flatten();
        let mut plan = Plan {
            previous: survey
                .resolved
                .as_ref()
                .filter(|(_, source)| !matches!(source, IdSource::Flag | IdSource::Environment))
                .map(|(id, _)| id.clone()),
            ..Plan::default()
        };
        if marked != Some(id) {
            plan.changes.push(write_marker(root, id));
        }
        plan.changes.extend(bind(&survey, root, id));
        self.make(&survey, plan, apply)
    }

    /// The databases no live registered path points at, other than the
    /// working directory's own.
    pub fn orphans(&mut self) -> Result<Vec<Problem>, ProjectError> {
        let survey = self.admin.survey(None).map_err(ProjectError::Port)?;
        Ok(orphans(&survey))
    }

    /// Remove the orphaned databases `ids`, each by name; refused, removing
    /// none, if any is not an orphan. Made only when `apply`.
    pub fn remove_orphans(&mut self, ids: &[String], apply: bool) -> Result<Plan, ProjectError> {
        let survey = self.admin.survey(None).map_err(ProjectError::Port)?;
        let orphaned = orphans(&survey);
        let mut plan = Plan::default();
        for id in ids {
            if !orphaned
                .iter()
                .any(|o| matches!(o, Problem::Orphan { id: o, .. } if o == id))
            {
                return Err(ProjectError::NotOrphan { id: id.clone() });
            }
            let change = Change::RemoveDatabase { id: id.clone() };
            if !plan.changes.contains(&change) {
                plan.changes.push(change);
            }
        }
        self.make(&survey, plan, apply)
    }

    /// `plan`, its changes made if `apply` and there are any.
    fn make(&mut self, survey: &Survey, mut plan: Plan, apply: bool) -> Result<Plan, ProjectError> {
        if apply && !plan.changes.is_empty() {
            self.admin
                .apply(None, survey, &plan.changes)
                .map_err(ProjectError::Port)?;
            plan.applied = true;
        }
        Ok(plan)
    }
}

fn write_marker(root: &str, id: &str) -> Change {
    Change::WriteMarker {
        root: root.to_string(),
        id: id.to_string(),
    }
}

/// The registry changes that bind `root` to `id` alone: `id` registered
/// for it, the other ids it is registered as unregistered, and `id`'s
/// entries whose paths are gone dropped.
fn bind(survey: &Survey, root: &str, id: &str) -> Vec<Change> {
    let mut changes = Vec::new();
    if !survey.entries_at(root).any(|e| e.id == id) {
        changes.push(Change::Register {
            id: id.to_string(),
            path: root.to_string(),
        });
    }
    for other in ids_at(survey, root).into_iter().filter(|other| other != id) {
        changes.push(Change::Unregister {
            id: other,
            path: root.to_string(),
        });
    }
    for gone in survey.entries_of(id).filter(|e| !e.exists) {
        let change = Change::Unregister {
            id: id.to_string(),
            path: gone.canonical.clone(),
        };
        if !changes.contains(&change) {
            changes.push(change);
        }
    }
    changes
}

/// The ids `root` is registered as, each once, in the registry's order.
fn ids_at(survey: &Survey, root: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for entry in survey.entries_at(root) {
        if !ids.contains(&entry.id) {
            ids.push(entry.id.clone());
        }
    }
    ids
}

/// Another tree that `id` is registered for and that still exists, when
/// `id` is not registered for `root` itself.
fn held_elsewhere(survey: &Survey, root: &str, id: &str) -> Option<String> {
    if survey.entries_at(root).any(|e| e.id == id) {
        return None;
    }
    survey
        .entries_of(id)
        .find(|e| e.exists && e.canonical != root)
        .map(|e| e.path.clone())
}

/// Every inconsistency in `survey`: the root's, then the registry's, then
/// the orphaned databases.
fn problems(survey: &Survey) -> Vec<Problem> {
    let root = survey.root.as_str();
    let mut ids = ids_at(survey, root);
    let mut problems = Vec::new();
    let marked = survey.marker.as_ref().or(survey.legacy_marker.as_ref());
    if let Some(id) = marked.filter(|id| !ids.contains(id) && !ids.is_empty()) {
        ids.push(id.clone());
    }
    if ids.len() > 1 {
        problems.push(Problem::TwoIds {
            root: root.to_string(),
            ids: ids.clone(),
        });
    } else if let Some(id) = marked.filter(|id| !ids.contains(id)) {
        problems.push(Problem::Unregistered {
            root: root.to_string(),
            id: id.clone(),
        });
    }
    match (&survey.marker, &survey.legacy_marker, ids.as_slice()) {
        (None, None, [id]) => problems.push(Problem::MissingMarker {
            root: root.to_string(),
            id: id.clone(),
        }),
        (None, Some(id), _) => problems.push(Problem::LegacyMarkerOnly {
            root: root.to_string(),
            id: id.clone(),
        }),
        _ => {}
    }
    for entry in survey.registry.iter().filter(|e| !e.exists) {
        problems.push(Problem::Gone {
            id: entry.id.clone(),
            path: entry.path.clone(),
        });
    }
    problems.extend(orphans(survey));
    problems
}

/// The databases no live registered path points at, other than the one
/// the working directory resolves to.
fn orphans(survey: &Survey) -> Vec<Problem> {
    let own = survey.resolved.as_ref().map(|(id, _)| id.as_str());
    survey
        .databases
        .iter()
        .filter(|id| Some(id.as_str()) != own)
        .filter(|id| !survey.entries_of(id).any(|e| e.exists))
        .map(|id| Problem::Orphan {
            id: id.clone(),
            database: survey.projects.join(id),
        })
        .collect()
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
