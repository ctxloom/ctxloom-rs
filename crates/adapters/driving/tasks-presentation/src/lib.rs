//! The tasks domain's shapes on the driving edge: what a task looks like
//! to a program, the same whichever adapter answers, so one jq filter
//! works on the command line's JSON and on the MCP tools' results alike.
//! presentation itself stays domain-free; this is the tasks domain's.

use std::collections::BTreeMap;

use serde::Serialize;
use tasks_application::{Project, Summary, Tag, TagCount, Task, Timestamp};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// A task as JSON: Go's `tasks.Task`, its fields in its order, each tag in
/// its canonical spelling. Every tag, hidden ones too: hiding is for what a
/// person reads, and a program reads this. `derived_priority` is null but
/// in a listing sorted by priority.
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
    #[serde(serialize_with = "go_number")]
    derived_priority: Option<f64>,
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
            derived_priority: None,
        }
    }

    /// The task with its derived priority.
    pub fn with_priority(self, derived_priority: Option<f64>) -> TaskJson<'a> {
        TaskJson {
            derived_priority,
            ..self
        }
    }
}

/// A derived priority, in `[0, 5]`, as Go's encoding/json writes a float64
/// there: a whole number without a fraction (`5`, where serde_json writes
/// `5.0`).
fn go_number<S: serde::Serializer>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
    match *value {
        Some(n) if n.fract() == 0.0 => serializer.serialize_i64(n as i64),
        Some(n) => serializer.serialize_f64(n),
        None => serializer.serialize_none(),
    }
}

/// How many characters of a task's first line a listing shows.
pub const HEADLINE_WIDTH: usize = 80;

/// The first line of `text` as a listing shows it: [`HEADLINE_WIDTH`]
/// characters at most.
pub fn headline(text: &str) -> String {
    presentation::text::headline(text, HEADLINE_WIDTH)
}

/// `dir (id)`: the project a command ran in; its id alone where its
/// directory is not known (Go's `formatProjectLabel`).
pub fn project_label(project: &Project) -> String {
    if project.root.is_empty() {
        return project.id.clone();
    }
    format!("{} ({})", project.root, project.id)
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

/// A task with the project it is in (Go's `taskRow`): the task, then the
/// project.
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

/// A summary: Go's `Summary`, the counts by status and each task In
/// Progress by id, with its project's label.
#[derive(Serialize)]
pub struct SummaryJson<'a> {
    counts: &'a BTreeMap<String, usize>,
    in_progress: Vec<InProgressJson<'a>>,
}

/// A task In Progress: Go's `InProgressTask`.
#[derive(Serialize)]
struct InProgressJson<'a> {
    harp_id: &'a str,
    project: String,
}

impl<'a> SummaryJson<'a> {
    pub fn of(summary: &'a Summary) -> SummaryJson<'a> {
        SummaryJson {
            counts: &summary.counts,
            in_progress: summary
                .in_progress
                .iter()
                .map(|row| InProgressJson {
                    harp_id: &row.task.harp_id,
                    project: project_label(&row.project),
                })
                .collect(),
        }
    }
}

/// A tag in use: Go's `TagCount`, the tag in its canonical spelling.
#[derive(Serialize)]
pub struct TagCountJson {
    tag: String,
    active: usize,
    total: usize,
}

impl TagCountJson {
    pub fn of(count: &TagCount) -> TagCountJson {
        TagCountJson {
            tag: count.tag.to_string(),
            active: count.active,
            total: count.total,
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

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
