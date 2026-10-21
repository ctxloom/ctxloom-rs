//! The tasks domain's shapes on the driving edge: what a task looks like
//! to a program, the same whichever adapter answers, so one jq filter
//! works on the command line's JSON and on the MCP tools' results alike.
//! presentation itself stays domain-free; this is the tasks domain's.

use serde::Serialize;
use tasks_application::{Tag, Task, Timestamp};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// A task as JSON: Go's `tasks.Task`, its fields in its order, each tag in
/// its canonical spelling. Every tag, hidden ones too: hiding is for what a
/// person reads, and a program reads this.
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

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
