//! Answers as the person reads them (text) or a program does (JSON), in
//! Go taskloom's shapes.

use std::io::{self, Write};

use serde::Serialize;
use tasks_application::{Project, Task, Timestamp};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// How many characters of a task's first line a listing shows.
pub const HEADLINE_WIDTH: usize = 80;

/// A task as JSON: Go's `tasks.Task`, its fields in its order.
#[derive(Serialize)]
pub struct TaskJson<'a> {
    harp_id: &'a str,
    text: &'a str,
    status: &'a str,
    checked: bool,
    text_hash: &'a str,
    trigger: &'a str,
    origin_session: &'a str,
    tags: &'a [String],
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
            tags: &task.tags,
            created_at: rfc3339(task.created_at),
        }
    }
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

/// `value` as indented JSON and a newline, as Go's clifmt writes it.
pub fn json<T: Serialize>(out: &mut dyn Write, value: &T) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)
}

/// `dir (id)`: the project a command ran in.
pub fn project_label(project: &Project) -> String {
    format!("{} ({})", project.root, project.id)
}

/// The first line of `text`, trailing spaces trimmed, cut to
/// [`HEADLINE_WIDTH`] characters with an ellipsis: Go's `Headline`.
pub fn headline(text: &str) -> String {
    let first = text
        .lines()
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    if first.chars().count() <= HEADLINE_WIDTH {
        return first.to_string();
    }
    let cut: String = first.chars().take(HEADLINE_WIDTH).collect();
    format!("{}\u{2026}", cut.trim_end_matches(' '))
}

/// The list view: one line per task, blank lines between, the id column as
/// wide as the widest id; `(no tasks)` for none.
pub fn table(out: &mut dyn Write, tasks: &[Task]) -> io::Result<()> {
    if tasks.is_empty() {
        return writeln!(out, "(no tasks)");
    }
    let width = tasks.iter().map(|t| t.harp_id.len()).max().unwrap_or(0);
    for (i, task) in tasks.iter().enumerate() {
        if i > 0 {
            writeln!(out)?;
        }
        let check = if task.checked { "x" } else { " " };
        let mut text = headline(&task.text);
        if !task.trigger.is_empty() {
            text = format!("{text}  (trigger: {})", task.trigger);
        }
        if !task.tags.is_empty() {
            text = format!("{text}  [{}]", task.tags.join(", "));
        }
        writeln!(
            out,
            "[{check}] {:<width$}  {:<11}  {text}",
            task.harp_id, task.status
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
