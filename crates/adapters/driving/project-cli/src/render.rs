//! The project commands' answers as a person reads them (text) or a
//! program does (JSON: one document, for jq).

use std::io::{self, Write};
use std::path::Path;

use project_application::{Change, Plan, Problem, Registered, Shown};
use serde::Serialize;

fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// A problem as JSON, tagged by its kind.
#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProblemJson<'a> {
    TwoIds {
        root: &'a str,
        project_ids: &'a [String],
    },
    MissingMarker {
        root: &'a str,
        project_id: &'a str,
    },
    LegacyMarkerOnly {
        root: &'a str,
        project_id: &'a str,
    },
    Unregistered {
        root: &'a str,
        project_id: &'a str,
    },
    Shared {
        root: &'a str,
        project_id: &'a str,
        other: &'a str,
    },
    Gone {
        project_id: &'a str,
        path: &'a str,
    },
    Orphan {
        project_id: &'a str,
        database: String,
    },
}

impl<'a> ProblemJson<'a> {
    pub fn of(problem: &'a Problem) -> ProblemJson<'a> {
        match problem {
            Problem::TwoIds { root, ids } => ProblemJson::TwoIds {
                root,
                project_ids: ids,
            },
            Problem::MissingMarker { root, id } => ProblemJson::MissingMarker {
                root,
                project_id: id,
            },
            Problem::LegacyMarkerOnly { root, id } => ProblemJson::LegacyMarkerOnly {
                root,
                project_id: id,
            },
            Problem::Unregistered { root, id } => ProblemJson::Unregistered {
                root,
                project_id: id,
            },
            Problem::Shared { root, id, other } => ProblemJson::Shared {
                root,
                project_id: id,
                other,
            },
            Problem::Gone { id, path } => ProblemJson::Gone {
                project_id: id,
                path,
            },
            Problem::Orphan { id, database } => ProblemJson::Orphan {
                project_id: id,
                database: shown(database),
            },
        }
    }
}

/// A problem in a sentence, with what fixes it.
pub fn problem_line(problem: &Problem) -> String {
    match problem {
        Problem::TwoIds { root, ids } => format!(
            "{root} is bound to more than one project: {}; run `project repoint <project-id>` to choose one",
            ids.join(", ")
        ),
        Problem::MissingMarker { root, id } => format!(
            "{root} is registered as {id} but has no .project-id; run `project heal`"
        ),
        Problem::LegacyMarkerOnly { root, id } => format!(
            "{root} names {id} only in Go's .ctxloom/project-id; run `project heal` to adopt it"
        ),
        Problem::Unregistered { root, id } => format!(
            "{root}/.project-id names {id}, which the registry does not hold for it; run `project heal`"
        ),
        Problem::Shared { root, id, other } => format!(
            "{root}/.project-id names {id}, the project of {other}, which still exists; run `project repoint {id} --share` if the two are meant to share it"
        ),
        Problem::Gone { id, path } => {
            format!("{id} is registered for {path}, which no longer exists")
        }
        Problem::Orphan { id, database } => format!(
            "{} is orphaned: no live registered path points at {id}; `project orphans remove {id} --yes` deletes it",
            shown(database)
        ),
    }
}

/// A registry entry as JSON.
#[derive(Serialize, Debug, PartialEq)]
pub struct EntryJson<'a> {
    project_id: &'a str,
    path: &'a str,
    exists: bool,
}

impl<'a> EntryJson<'a> {
    fn of(entry: &'a Registered) -> EntryJson<'a> {
        EntryJson {
            project_id: &entry.id,
            path: &entry.path,
            exists: entry.exists,
        }
    }
}

/// `project show` as JSON.
#[derive(Serialize, Debug, PartialEq)]
pub struct ShowJson<'a> {
    project_id: Option<&'a str>,
    /// Where the id came from: flag, environment, marker, legacy-marker or
    /// registry.
    source: Option<&'static str>,
    work_root: &'a str,
    root: &'a str,
    marker: Option<&'a str>,
    legacy_marker: Option<&'a str>,
    entries: Vec<EntryJson<'a>>,
    database: Option<String>,
    problems: Vec<ProblemJson<'a>>,
}

impl<'a> ShowJson<'a> {
    pub fn of(shown_: &'a Shown) -> ShowJson<'a> {
        let survey = &shown_.survey;
        ShowJson {
            project_id: survey.resolved.as_ref().map(|(id, _)| id.as_str()),
            source: survey.resolved.as_ref().map(|(_, source)| source.as_str()),
            work_root: &survey.work_root,
            root: &survey.root,
            marker: survey.marker.as_deref(),
            legacy_marker: survey.legacy_marker.as_deref(),
            entries: shown_.entries.iter().map(EntryJson::of).collect(),
            database: shown_.store.as_deref().map(shown),
            problems: shown_.problems.iter().map(ProblemJson::of).collect(),
        }
    }
}

/// `project show` as text.
pub fn show_text(out: &mut dyn Write, shown_: &Shown) -> io::Result<()> {
    let survey = &shown_.survey;
    match &survey.resolved {
        Some((id, source)) => writeln!(out, "Project:  {id} (from the {})", source.as_str())?,
        None => writeln!(out, "Project:  none yet (a write would mint one)")?,
    }
    writeln!(out, "Root:     {}", survey.root)?;
    if survey.work_root != survey.root {
        writeln!(
            out,
            "Worktree: {} (a linked worktree of the root, with no marker of its own)",
            survey.work_root
        )?;
    }
    writeln!(
        out,
        "Marker:   {}",
        survey.marker.as_deref().unwrap_or("none")
    )?;
    if let Some(id) = &survey.legacy_marker {
        writeln!(out, "Go's marker: {id}")?;
    }
    if shown_.entries.is_empty() {
        writeln!(out, "Registry: no entry for the root")?;
    }
    for entry in &shown_.entries {
        writeln!(out, "Registry: {}\t{}", entry.id, entry.path)?;
    }
    match &shown_.store {
        Some(store) => writeln!(out, "Database: {}", shown(store))?,
        None => writeln!(out, "Database: none")?,
    }
    problems_text(out, "Problems:", &shown_.problems, "No problems found.")
}

fn problems_text(
    out: &mut dyn Write,
    heading: &str,
    problems: &[Problem],
    none: &str,
) -> io::Result<()> {
    if problems.is_empty() {
        return if none.is_empty() {
            Ok(())
        } else {
            writeln!(out, "{none}")
        };
    }
    writeln!(out, "{heading}")?;
    for problem in problems {
        writeln!(out, "  - {}", problem_line(problem))?;
    }
    Ok(())
}

/// A change as JSON, tagged by its kind.
#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChangeJson<'a> {
    WriteMarker { root: &'a str, project_id: &'a str },
    Register { project_id: &'a str, path: &'a str },
    Unregister { project_id: &'a str, path: &'a str },
    RemoveDatabase { project_id: &'a str },
}

impl<'a> ChangeJson<'a> {
    pub fn of(change: &'a Change) -> ChangeJson<'a> {
        match change {
            Change::WriteMarker { root, id } => ChangeJson::WriteMarker {
                root,
                project_id: id,
            },
            Change::Register { id, path } => ChangeJson::Register {
                project_id: id,
                path,
            },
            Change::Unregister { id, path } => ChangeJson::Unregister {
                project_id: id,
                path,
            },
            Change::RemoveDatabase { id } => ChangeJson::RemoveDatabase { project_id: id },
        }
    }
}

/// A change in a line.
pub fn change_line(change: &Change) -> String {
    match change {
        Change::WriteMarker { root, id } => format!("write {id} into {root}/.project-id"),
        Change::Register { id, path } => format!("register {id} for {path}"),
        Change::Unregister { id, path } => format!("unregister {id} for {path}"),
        Change::RemoveDatabase { id } => format!("delete the database directory of {id}"),
    }
}

/// A plan as JSON.
#[derive(Serialize, Debug, PartialEq)]
pub struct PlanJson<'a> {
    applied: bool,
    changes: Vec<ChangeJson<'a>>,
    unresolved: Vec<ProblemJson<'a>>,
    previous: Option<&'a str>,
}

impl<'a> PlanJson<'a> {
    pub fn of(plan: &'a Plan) -> PlanJson<'a> {
        PlanJson {
            applied: plan.applied,
            changes: plan.changes.iter().map(ChangeJson::of).collect(),
            unresolved: plan.unresolved.iter().map(ProblemJson::of).collect(),
            previous: plan.previous.as_deref(),
        }
    }
}

/// A plan as text: what changed, or would with `--yes`, and what is left.
pub fn plan_text(out: &mut dyn Write, plan: &Plan) -> io::Result<()> {
    match (plan.changes.is_empty(), plan.applied) {
        (true, _) => writeln!(out, "Nothing to change.")?,
        (false, true) => writeln!(out, "Changed:")?,
        (false, false) => writeln!(
            out,
            "Would change (run again with --yes to make the change):"
        )?,
    }
    for change in &plan.changes {
        writeln!(out, "  - {}", change_line(change))?;
    }
    if let Some(previous) = &plan.previous {
        writeln!(
            out,
            "It was {previous}, whose database stays; `project show` reports it once nothing points at it."
        )?;
    }
    problems_text(out, "Not resolved:", &plan.unresolved, "")
}

/// An orphan as JSON.
#[derive(Serialize, Debug, PartialEq)]
pub struct OrphanJson<'a> {
    project_id: &'a str,
    database: String,
}

/// The orphans as JSON; anything else in `orphans` is left out.
pub fn orphans_json(orphans: &[Problem]) -> Vec<OrphanJson<'_>> {
    orphans
        .iter()
        .filter_map(|o| match o {
            Problem::Orphan { id, database } => Some(OrphanJson {
                project_id: id,
                database: shown(database),
            }),
            _ => None,
        })
        .collect()
}

/// The orphans as text, one a line: the id, then its directory.
pub fn orphans_text(out: &mut dyn Write, orphans: &[Problem]) -> io::Result<()> {
    let listed = orphans_json(orphans);
    if listed.is_empty() {
        return writeln!(out, "No orphaned databases.");
    }
    for orphan in listed {
        writeln!(out, "{}\t{}", orphan.project_id, orphan.database)?;
    }
    Ok(())
}

/// `value` as one JSON document and a newline.
pub fn json(out: &mut dyn Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
