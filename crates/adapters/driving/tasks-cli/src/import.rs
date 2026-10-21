//! `import`'s answer: what each project's Go log came to, and everything
//! that could not be brought over, as text or JSON.

use std::io::{self, Write};

use annals_diagnostic::{Diagnostic, Report};
use serde::Serialize;
use tasks_application::{ImportReport, Lost, ProjectImport};

use crate::render;

/// The report as JSON.
#[derive(Serialize)]
pub struct ImportJson<'a> {
    /// Whether everything was brought over.
    complete: bool,
    projects: Vec<ProjectJson<'a>>,
}

#[derive(Serialize)]
struct ProjectJson<'a> {
    project_id: &'a str,
    failed: Option<DiagnosticJson>,
    records: usize,
    repeated: usize,
    tasks: usize,
    applied: usize,
    already: usize,
    unchanged: usize,
    ignored: usize,
    repaired: usize,
    lost: Vec<LostJson<'a>>,
}

#[derive(Serialize)]
struct LostJson<'a> {
    line: u32,
    op: &'static str,
    harp_id: &'a str,
    text: &'a str,
    why: DiagnosticJson,
}

/// A diagnostic as a program reads it: its code and its English.
#[derive(Serialize)]
struct DiagnosticJson {
    code: &'static str,
    message: String,
}

impl DiagnosticJson {
    fn of(diagnostic: &dyn Diagnostic) -> DiagnosticJson {
        DiagnosticJson {
            code: diagnostic.code(),
            message: diagnostic.to_string(),
        }
    }
}

impl<'a> ImportJson<'a> {
    pub fn of(report: &'a ImportReport) -> ImportJson<'a> {
        ImportJson {
            complete: report.complete(),
            projects: report.projects.iter().map(project_json).collect(),
        }
    }
}

fn project_json(project: &ProjectImport) -> ProjectJson<'_> {
    ProjectJson {
        project_id: &project.project_id,
        failed: project
            .failed
            .as_ref()
            .map(|r: &Report| DiagnosticJson::of(r)),
        records: project.records,
        repeated: project.repeated,
        tasks: project.tasks,
        applied: project.applied,
        already: project.already,
        unchanged: project.unchanged,
        ignored: project.ignored,
        repaired: project.repaired,
        lost: project
            .lost
            .iter()
            .map(|lost| LostJson {
                line: lost.line,
                op: lost.op.as_str(),
                harp_id: &lost.harp_id,
                text: &lost.text,
                why: DiagnosticJson::of(&lost.why),
            })
            .collect(),
    }
}

/// The report as text: a line per project, then a line per record left
/// behind and the reason the project stopped, if it did.
pub fn text(out: &mut dyn Write, report: &ImportReport) -> io::Result<()> {
    for project in &report.projects {
        writeln!(out, "{}: {}", project.project_id, summary(project))?;
        for lost in &project.lost {
            writeln!(out, "  not imported: {}", lost_line(lost))?;
        }
        if let Some(failed) = &project.failed {
            writeln!(out, "  stopped: {failed}")?;
        }
    }
    Ok(())
}

/// What a project's import counted, the counts that are zero left out
/// after the first three.
fn summary(project: &ProjectImport) -> String {
    let mut parts = vec![
        format!("{} records", project.records),
        format!("{} tasks added", project.tasks),
        format!("{} records imported", project.applied),
    ];
    let optional = [
        (project.already, "already imported"),
        (project.repeated, "repeated lines skipped"),
        (project.unchanged, "retags changing nothing"),
        (project.ignored, "to no live task, as Go ignores them"),
        (project.repaired, "displaced adds Go's repair re-added"),
        (project.lost.len(), "not imported"),
    ];
    parts.extend(
        optional
            .iter()
            .filter(|(count, _)| *count > 0)
            .map(|(count, what)| format!("{count} {what}")),
    );
    parts.join(", ")
}

fn lost_line(lost: &Lost) -> String {
    let mut line = format!("line {}, {} {}", lost.line, lost.op.as_str(), lost.harp_id);
    if !lost.text.is_empty() {
        line.push_str(&format!(" {:?}", render::headline(&lost.text)));
    }
    line.push_str(&format!(": {}", lost.why));
    line
}

#[cfg(test)]
#[path = "import.test.rs"]
mod tests;
