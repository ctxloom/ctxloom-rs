//! The tasks views: answers as a person reads them (text) or a program
//! does (JSON), in Go taskloom's shapes, on presentation's [`Render`].

use std::io::{self, Write};

use presentation::{text, Render};
use serde::Serialize;
use std::collections::BTreeMap;

use tasks_application::{
    Details, Listing, Project, Reach, Row, Shown, StatusInfo, Statuses, Summary, Tag, Task,
    Timestamp,
};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// How many characters of a task's first line a listing shows.
pub const HEADLINE_WIDTH: usize = 80;

/// A task as JSON: Go's `tasks.Task`, its fields in its order, each tag in
/// its canonical spelling.
#[derive(Serialize)]
pub struct TaskJson<'a> {
    harp_id: &'a str,
    text: &'a str,
    status: &'a str,
    checked: bool,
    text_hash: &'a str,
    trigger: &'a str,
    origin_session: &'a str,
    tags: Vec<String>,
    created_at: String,
}

impl<'a> TaskJson<'a> {
    pub fn of(task: &'a Task) -> TaskJson<'a> {
        TaskJson {
            harp_id: &task.harp_id,
            text: &task.text,
            status: &task.status,
            checked: task.checked,
            text_hash: &task.text_hash,
            trigger: &task.trigger,
            origin_session: &task.origin_session,
            tags: spelled(&task.tags),
            created_at: rfc3339(task.created_at),
        }
    }
}

/// Each tag in its canonical spelling (tagma's `Display`).
pub fn spelled(tags: &[Tag]) -> Vec<String> {
    tags.iter().map(ToString::to_string).collect()
}

/// `at` in RFC 3339, UTC, with as many fractional digits as it needs (Go's
/// `RFC3339Nano`).
pub fn rfc3339(at: Timestamp) -> String {
    let nanos = i128::from(at.seconds) * 1_000_000_000 + i128::from(at.nanos);
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .ok()
        .and_then(|t| t.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// A task as one row of a compact listing: Go's `CompactTask`, its
/// fields in its order, the text cut to its headline.
#[derive(Serialize)]
pub struct CompactJson<'a> {
    harp_id: &'a str,
    status: &'a str,
    checked: bool,
    tags: Vec<String>,
    headline: String,
}

impl<'a> CompactJson<'a> {
    pub fn of(task: &'a Task) -> CompactJson<'a> {
        CompactJson {
            harp_id: &task.harp_id,
            status: &task.status,
            checked: task.checked,
            tags: spelled(&task.tags),
            headline: headline(&task.text),
        }
    }
}

/// A row of a listing that spans projects (Go's `taskRow`): the task, then
/// the project it came from.
#[derive(Serialize)]
pub struct RowJson<'a, T: Serialize> {
    #[serde(flatten)]
    task: T,
    project_id: &'a str,
    project_dir: &'a str,
}

impl<'a, T: Serialize> RowJson<'a, T> {
    pub fn of(task: T, project: &'a Project) -> RowJson<'a, T> {
        RowJson {
            task,
            project_id: &project.id,
            project_dir: &project.root,
        }
    }
}

/// `dir (id)`: the project a command ran in; its id alone where its
/// directory is not known (Go's `formatProjectLabel`).
pub fn project_label(project: &Project) -> String {
    if project.root.is_empty() {
        return project.id.clone();
    }
    format!("{} ({})", project.root, project.id)
}

/// The heading of an answer read across every project.
pub fn projects_heading(out: &mut dyn Write, projects: usize) -> io::Result<()> {
    writeln!(out, "Projects: {projects} (--global)\n")
}

/// The first line of `text` as a listing shows it: [`HEADLINE_WIDTH`]
/// characters at most.
pub fn headline(text: &str) -> String {
    text::headline(text, HEADLINE_WIDTH)
}

/// How wide the status column is: wide enough for every default status.
const STATUS_WIDTH: usize = 11;

/// The tags of `tags` a person is shown, in their canonical spelling.
fn shown(tags: &[Tag], shown: &Shown) -> Vec<String> {
    shown
        .shown(tags)
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

/// The list view: one line per task, blank lines between, the id column as
/// wide as the widest id, columns measured in display width, the tags
/// `shown` shows; `(no tasks)` for none.
pub fn table(out: &mut dyn Write, tasks: &[&Task], shown: &Shown) -> io::Result<()> {
    if tasks.is_empty() {
        return writeln!(out, "(no tasks)");
    }
    let width = tasks
        .iter()
        .map(|t| text::width(&t.harp_id))
        .max()
        .unwrap_or(0);
    for (i, task) in tasks.iter().enumerate() {
        if i > 0 {
            writeln!(out)?;
        }
        let check = if task.checked { "x" } else { " " };
        let mut line = headline(&task.text);
        if !task.trigger.is_empty() {
            line = format!("{line}  (trigger: {})", task.trigger);
        }
        let tags = self::shown(&task.tags, shown);
        if !tags.is_empty() {
            line = format!("{line}  [{}]", tags.join(", "));
        }
        writeln!(
            out,
            "[{check}] {}  {}  {line}",
            text::pad(&task.harp_id, width),
            text::pad(&task.status, STATUS_WIDTH)
        )?;
    }
    Ok(())
}

/// A task `add`, `status` or `edit` wrote: JSON the task; text its id,
/// status and text.
pub struct Added<'a>(pub &'a Task);

/// A task `tag` wrote: JSON the task, every tag; text its id, status and
/// the tags a person is shown.
pub struct Tagged<'a>(pub &'a Task, pub &'a Shown);

/// A listing: JSON an array of tasks, every tag, each a compact row if
/// `compact`, and each with its project where the listing spans projects;
/// text the project, then the table, the tags a person is shown, or across
/// projects a table for each.
pub struct Listed<'a> {
    pub listing: &'a Listing,
    pub compact: bool,
}

/// One row of a listing as JSON: the task whole or compact, alone or with
/// its project.
#[derive(Serialize)]
#[serde(untagged)]
pub enum RowView<'a> {
    Whole(TaskJson<'a>),
    Compact(CompactJson<'a>),
    WholeIn(RowJson<'a, TaskJson<'a>>),
    CompactIn(RowJson<'a, CompactJson<'a>>),
}

/// `rows` as JSON: each task whole, or compact if `compact`, with its
/// project if `attributed`.
pub fn rows_json(rows: &[Row], compact: bool, attributed: bool) -> Vec<RowView<'_>> {
    rows.iter()
        .map(|row| row_view(row, compact, attributed))
        .collect()
}

fn row_view(row: &Row, compact: bool, attributed: bool) -> RowView<'_> {
    match (compact, attributed) {
        (false, false) => RowView::Whole(TaskJson::of(&row.task)),
        (true, false) => RowView::Compact(CompactJson::of(&row.task)),
        (false, true) => RowView::WholeIn(RowJson::of(TaskJson::of(&row.task), &row.project)),
        (true, true) => RowView::CompactIn(RowJson::of(CompactJson::of(&row.task), &row.project)),
    }
}

/// `rows` as tables, one for each project in turn, each headed by it.
pub fn tables(out: &mut dyn Write, rows: &[Row], shown: &Shown) -> io::Result<()> {
    if rows.is_empty() {
        return writeln!(out, "(no tasks)");
    }
    for group in rows.chunk_by(|a, b| a.project.id == b.project.id) {
        writeln!(out, "Project: {}", project_label(&group[0].project))?;
        let tasks: Vec<&Task> = group.iter().map(|row| &row.task).collect();
        table(out, &tasks, shown)?;
        writeln!(out)?;
    }
    Ok(())
}

impl Render for Added<'_> {
    fn json(&self) -> impl Serialize + '_ {
        TaskJson::of(self.0)
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let task = self.0;
        writeln!(out, "{}\t{}\t{}", task.harp_id, task.status, task.text)
    }
}

impl Render for Tagged<'_> {
    fn json(&self) -> impl Serialize + '_ {
        TaskJson::of(self.0)
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let task = self.0;
        writeln!(
            out,
            "{}\t{}\t{}",
            task.harp_id,
            task.status,
            shown(&task.tags, self.1).join(",")
        )
    }
}

impl Render for Listed<'_> {
    fn json(&self) -> impl Serialize + '_ {
        let listing = self.listing;
        rows_json(&listing.rows, self.compact, listing.reach.is_every())
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let listing = self.listing;
        match &listing.reach {
            Reach::Project(project) => writeln!(out, "Project: {}\n", project_label(project))?,
            Reach::NoProject => {}
            Reach::Every { projects, .. } => {
                projects_heading(out, *projects)?;
                return tables(out, &listing.rows, &listing.shown);
            }
        }
        let tasks: Vec<&Task> = listing.tasks().collect();
        table(out, &tasks, &listing.shown)
    }
}

/// The tasks `show` found: JSON one task with its project when one id was
/// asked for, else an array of them (Go's rule: a group is a list, one
/// value an object); text each task whole, a blank line between, each
/// headed by its project when read across projects.
pub struct Detailed<'a>(pub &'a Details);

impl Render for Detailed<'_> {
    fn json(&self) -> impl Serialize + '_ {
        let rows = &self.0.rows;
        match rows.as_slice() {
            [one] => OneOrMany::One(attributed(one)),
            _ => OneOrMany::Many(rows.iter().map(attributed).collect()),
        }
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let every = self.0.reach.is_every();
        for (i, row) in self.0.rows.iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            if every {
                writeln!(out, "project: {}", project_label(&row.project))?;
            }
            detail(out, &row.task, &self.0.shown)?;
        }
        Ok(())
    }
}

/// `row`'s task whole, with its project.
fn attributed(row: &Row) -> RowJson<'_, TaskJson<'_>> {
    RowJson::of(TaskJson::of(&row.task), &row.project)
}

/// One value, or several.
#[derive(Serialize)]
#[serde(untagged)]
enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

/// A summary: JSON Go's `Summary`, the counts by status and each task In
/// Progress with its project; text a line per status, then the tasks In
/// Progress, by id (with its project across projects).
pub struct Summed<'a>(pub &'a Summary);

#[derive(Serialize)]
struct SummaryJson<'a> {
    counts: &'a BTreeMap<String, usize>,
    in_progress: Vec<InProgressJson<'a>>,
}

#[derive(Serialize)]
struct InProgressJson<'a> {
    harp_id: &'a str,
    project: String,
}

impl Render for Summed<'_> {
    fn json(&self) -> impl Serialize + '_ {
        SummaryJson {
            counts: &self.0.counts,
            in_progress: self
                .0
                .in_progress
                .iter()
                .map(|row| InProgressJson {
                    harp_id: &row.task.harp_id,
                    project: project_label(&row.project),
                })
                .collect(),
        }
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let summary = self.0;
        if let Reach::Every { projects, .. } = summary.reach {
            projects_heading(out, projects)?;
        }
        for (status, count) in &summary.counts {
            writeln!(out, "{status}\t{count}")?;
        }
        if summary.in_progress.is_empty() {
            return Ok(());
        }
        let every = summary.reach.is_every();
        let names: Vec<String> = summary
            .in_progress
            .iter()
            .map(|row| match every {
                true => format!("{} ({})", row.task.harp_id, project_label(&row.project)),
                false => row.task.harp_id.clone(),
            })
            .collect();
        writeln!(out, "\nIn-progress: {}", names.join(", "))
    }
}

/// The statuses: JSON Go's `StatusInfo` array; text a line each, its
/// order, its name, and whether it is terminal or requires a trigger.
pub struct Taxonomy<'a>(pub &'a Statuses);

#[derive(Serialize)]
struct StatusJson<'a> {
    name: &'a str,
    order: usize,
    terminal: bool,
    requires_trigger: bool,
}

impl<'a> StatusJson<'a> {
    fn of(status: &'a StatusInfo) -> StatusJson<'a> {
        StatusJson {
            name: &status.name,
            order: status.order,
            terminal: status.terminal,
            requires_trigger: status.requires_trigger,
        }
    }
}

impl Render for Taxonomy<'_> {
    fn json(&self) -> impl Serialize + '_ {
        self.0
            .statuses
            .iter()
            .map(StatusJson::of)
            .collect::<Vec<_>>()
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        for status in &self.0.statuses {
            write!(out, "{}\t{}", status.order, status.name)?;
            if status.terminal {
                write!(out, "\tterminal")?;
            }
            if status.requires_trigger {
                write!(out, "\trequires-trigger")?;
            }
            writeln!(out)?;
        }
        Ok(())
    }
}

/// One task whole: its check, id and status; the tags a person is shown
/// and its trigger, if any; then its whole text.
fn detail(out: &mut dyn Write, task: &Task, shown: &Shown) -> io::Result<()> {
    let check = if task.checked { "x" } else { " " };
    writeln!(out, "[{check}] {}  {}", task.harp_id, task.status)?;
    let tags = self::shown(&task.tags, shown);
    if !tags.is_empty() {
        writeln!(out, "    tags: {}", tags.join(", "))?;
    }
    if !task.trigger.is_empty() {
        writeln!(out, "    trigger: {}", task.trigger)?;
    }
    writeln!(out)?;
    writeln!(out, "{}", task.text)
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
