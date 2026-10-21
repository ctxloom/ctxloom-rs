//! The tasks views: answers as a person reads them (text) or a program
//! does (JSON), in Go taskloom's shapes, on presentation's [`Render`].

use std::io::{self, Write};

use presentation::{text, Render};
use serde::Serialize;
use tasks_application::{Project, Shown, Tag, Task, Timestamp};
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

/// `dir (id)`: the project a command ran in.
pub fn project_label(project: &Project) -> String {
    format!("{} ({})", project.root, project.id)
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
pub fn table(out: &mut dyn Write, tasks: &[Task], shown: &Shown) -> io::Result<()> {
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

/// A task `add` wrote: JSON the task; text its id, status and text.
pub struct Added<'a>(pub &'a Task);

/// A task `tag` wrote: JSON the task, every tag; text its id, status and
/// the tags a person is shown.
pub struct Tagged<'a>(pub &'a Task, pub &'a Shown);

/// A listing: JSON an array of tasks, every tag; text the project, then
/// the table, the tags a person is shown.
pub struct Listed<'a> {
    pub project: Option<&'a Project>,
    pub tasks: &'a [Task],
    pub shown: &'a Shown,
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
        self.tasks.iter().map(TaskJson::of).collect::<Vec<_>>()
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        if let Some(project) = self.project {
            writeln!(out, "Project: {}\n", project_label(project))?;
        }
        table(out, self.tasks, self.shown)
    }
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
