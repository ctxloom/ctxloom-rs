//! `prune`'s answer: Go taskloom's prune report, as a person reads it and
//! as a program does (one JSON document), and the command that applies a
//! report.

use std::io::{self, Write};

use presentation::{text, Render};
use serde::Serialize;
use tasks_application::{Pruning, Timestamp};
use tasks_presentation::{rfc3339, spelled};

use crate::render::{headline, project_label};
use crate::PROG;

/// A prune's report: JSON Go's `pruneReport`, its fields in its order;
/// text what was selected by, each task taken (or that would be), the
/// tasks left, and every reference a survivor still holds to one taken.
pub struct PruneReport<'a>(pub &'a Pruning);

#[derive(Serialize)]
struct ReportJson<'a> {
    project_id: &'a str,
    project_dir: &'a str,
    statuses: &'a [String],
    before: String,
    tag_query: &'a str,
    applied: bool,
    pruned: Vec<PrunedJson<'a>>,
    tasks_pruned: usize,
    events_removed: u64,
    tasks_remaining: usize,
    dangling_references: Vec<DanglingJson<'a>>,
}

/// A task taken: Go's `PrunedTask`, every tag (hidden ones too).
#[derive(Serialize)]
struct PrunedJson<'a> {
    harp_id: &'a str,
    status: &'a str,
    tags: Vec<String>,
    headline: String,
    last_status_change: String,
}

/// A reference a survivor still holds: Go's `DanglingReference`.
#[derive(Serialize)]
struct DanglingJson<'a> {
    harp_id: &'a str,
    pruned: &'a str,
    via: String,
}

impl Render for PruneReport<'_> {
    fn json(&self) -> impl Serialize + '_ {
        let pruning = self.0;
        let project = pruning.project.as_ref();
        ReportJson {
            project_id: project.map_or("", |p| p.id.as_str()),
            project_dir: project.map_or("", |p| p.root.as_str()),
            statuses: &pruning.statuses,
            before: rfc3339(pruning.before),
            tag_query: &pruning.tag_query,
            applied: pruning.applied,
            pruned: pruning
                .pruned
                .iter()
                .map(|p| PrunedJson {
                    harp_id: &p.task.harp_id,
                    status: &p.task.status,
                    tags: spelled(&p.task.tags),
                    headline: headline(&p.task.text),
                    last_status_change: rfc3339(p.status_set_at),
                })
                .collect(),
            tasks_pruned: pruning.pruned.len(),
            events_removed: pruning.events_removed,
            tasks_remaining: pruning.tasks_remaining,
            dangling_references: pruning
                .dangling
                .iter()
                .map(|d| DanglingJson {
                    harp_id: &d.harp_id,
                    pruned: &d.pruned,
                    via: d.via.to_string(),
                })
                .collect(),
        }
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        let pruning = self.0;
        if let Some(project) = &pruning.project {
            writeln!(out, "Project: {}\n", project_label(project))?;
        }
        let mut criteria = format!(
            "status {}, last status change before {}",
            pruning.statuses.join(" or "),
            minute(pruning.before)
        );
        if !pruning.tag_query.is_empty() {
            criteria = format!("{criteria}, tag query {}", pruning.tag_query);
        }
        if pruning.pruned.is_empty() {
            return writeln!(out, "Nothing to prune ({criteria}).");
        }
        let verb = if pruning.applied {
            "Pruned"
        } else {
            "Would prune"
        };
        writeln!(
            out,
            "{verb} {} task(s), {} event(s) \u{2014} {criteria}:\n",
            pruning.pruned.len(),
            pruning.events_removed
        )?;
        let widest = |of: fn(&tasks_application::Pruned) -> &str| {
            pruning
                .pruned
                .iter()
                .map(|p| text::width(of(p)))
                .max()
                .unwrap_or(0)
        };
        let id_width = widest(|p| &p.task.harp_id);
        let status_width = widest(|p| &p.task.status);
        for pruned in &pruning.pruned {
            writeln!(
                out,
                "  {}  {}  {}  {}",
                text::pad(&pruned.task.harp_id, id_width),
                text::pad(&pruned.task.status, status_width),
                day(pruned.status_set_at),
                headline(&pruned.task.text)
            )?;
        }
        writeln!(out, "\n{} task(s) remain.", pruning.tasks_remaining)?;
        if !pruning.dangling.is_empty() {
            writeln!(out, "\nStill named by a surviving task (left as written):")?;
            for dangling in &pruning.dangling {
                writeln!(
                    out,
                    "  {} -> {}  ({})",
                    dangling.harp_id, dangling.pruned, dangling.via
                )?;
            }
        }
        Ok(())
    }
}

/// `at`'s date, UTC: `2026-01-01`.
fn day(at: Timestamp) -> String {
    rfc3339(at).chars().take(10).collect()
}

/// `at` to the minute, UTC, as Go prints a prune's cut:
/// `2026-01-01 00:00 UTC`.
fn minute(at: Timestamp) -> String {
    let spelled = rfc3339(at);
    let time: String = spelled.chars().skip(11).take(5).collect();
    format!("{} {time} UTC", day(at))
}

/// The command that applies exactly the prune a report showed: its age,
/// statuses and tag query as given, then `--yes`, each quoted for a POSIX
/// shell where it must be.
pub fn apply_command(older_than: &str, statuses: &[String], tag_query: &str) -> String {
    let mut words = vec![
        PROG.to_string(),
        "prune".to_string(),
        "--older-than".to_string(),
        shell_word(older_than),
    ];
    for status in statuses {
        words.push("--status".to_string());
        words.push(shell_word(status));
    }
    if !tag_query.is_empty() {
        words.push("--tag-query".to_string());
        words.push(shell_word(tag_query));
    }
    words.push("--yes".to_string());
    words.join(" ")
}

/// `word` as a POSIX shell reads it back: bare when every character is one
/// a shell passes through literally, single-quoted otherwise (Go's
/// `shellWord`), so `To Do` or `triage:level<=2` survives being pasted.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@%+,-./:=_".contains(c));
    if plain {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

#[cfg(test)]
#[path = "prune.test.rs"]
mod tests;
