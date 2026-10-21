//! The tasks views: answers as a person reads them (text) or a program
//! does (JSON), in Go taskloom's shapes, on presentation's [`Render`]. A
//! task's JSON is tasks-presentation's, shared with the MCP server.

use std::io::{self, Write};

use presentation::{text, Render};
use serde::Serialize;
use tasks_application::{Project, Task};
use tasks_presentation::TaskJson;

/// How many characters of a task's first line a listing shows.
pub const HEADLINE_WIDTH: usize = 80;

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

/// The list view: one line per task, blank lines between, the id column as
/// wide as the widest id, columns measured in display width; `(no tasks)`
/// for none.
pub fn table(out: &mut dyn Write, tasks: &[Task]) -> io::Result<()> {
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
        if !task.tags.is_empty() {
            line = format!("{line}  [{}]", task.tags.join(", "));
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

/// A task `tag` wrote: JSON the task; text its id, status and tags.
pub struct Tagged<'a>(pub &'a Task);

/// A listing: JSON an array of tasks; text the project, then the table.
pub struct Listed<'a> {
    pub project: Option<&'a Project>,
    pub tasks: &'a [Task],
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
            task.tags.join(",")
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
        table(out, self.tasks)
    }
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
