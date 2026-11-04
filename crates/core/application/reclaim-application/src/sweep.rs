//! `ctxloom session sweep`: every session of a project, turned into a row by
//! a fixed table, in order:
//!
//! | the session                                   | the row                     |
//! |-----------------------------------------------|-----------------------------|
//! | running                                       | skipped whole               |
//! | liveness unprovable                           | skipped whole; the manual route is named |
//! | marked keep                                   | held; `session keep --undo` is named |
//! | nothing left to purge                         | untouched                   |
//! | no purge age                                  | held (purges have no default) |
//! | last active after the purge age               | left                        |
//! | its transcript within the retention           | held, saying why            |
//! | otherwise                                     | purged (planned without `yes`) |
//!
//! Nothing is decided by a model: the same sessions give the same report.
//! The purge acts through `session purge`, so every refusal it makes is the
//! sweep's.

use std::collections::BTreeMap;

use isolation_application::WorktreeUse;
use serde::Serialize;
use session_index::stamp;
use sessions_application::{
    AppError, ListQuery, Refusal, RemoveRequest, Scope, SessionService, State, Timestamp,
};

use crate::ReclaimError;

/// What a sweep is asked.
#[derive(Clone, Debug, Default)]
pub struct SweepRequest {
    /// Every project's sessions, not only the current one's.
    pub all_projects: bool,
    /// Sessions last active before this are purged; none holds every purge.
    pub purge_cutoff: Option<Timestamp>,
    /// Act on the plan (`--yes`); without it only report.
    pub yes: bool,
}

/// What became of a session.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Done,
    Planned,
    Held,
    Left,
    Skipped,
    Failed,
}

impl Verdict {
    fn name(self) -> &'static str {
        match self {
            Verdict::Done => "done",
            Verdict::Planned => "planned",
            Verdict::Held => "held",
            Verdict::Left => "left",
            Verdict::Skipped => "skipped",
            Verdict::Failed => "failed",
        }
    }
}

/// One session's row.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct SweepRow {
    pub harp: String,
    pub action: &'static str,
    pub verdict: Verdict,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
    /// The command that does what the row could not, or will.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub bytes: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// The sweep's answer.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct SweepReport {
    /// When purging begins, as a date; none when purges are held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purge_cutoff: Option<String>,
    pub rows: Vec<SweepRow>,
    pub counts: BTreeMap<&'static str, u64>,
    /// Sessions with nothing to do.
    pub untouched: u64,
}

fn row(
    harp: &str,
    action: &'static str,
    verdict: Verdict,
    reason: &str,
    command: String,
) -> SweepRow {
    SweepRow {
        harp: harp.to_string(),
        action,
        verdict,
        reason: reason.to_string(),
        command,
        bytes: 0,
    }
}

/// Sweep the project's sessions (or every project's).
pub async fn sweep<S: SessionService + WorktreeUse>(
    service: &mut S,
    request: &SweepRequest,
) -> Result<SweepReport, ReclaimError> {
    let views = service
        .list(ListQuery {
            project: None,
            all: request.all_projects,
            state: None,
        })
        .await?;
    let mut rows = Vec::new();
    let mut untouched = 0;
    for view in &views {
        let harp = view.record.harp.as_str();
        let purge = format!("ctxloom session purge {harp} --yes");
        let decided = match view.state {
            State::Live => Some(row(
                harp,
                "skip",
                Verdict::Skipped,
                "it is running",
                String::new(),
            )),
            State::Unknown => Some(row(
                harp,
                "skip",
                Verdict::Skipped,
                "its liveness cannot be proven",
                format!("ctxloom session purge {harp} --even-if-live"),
            )),
            State::Ended if view.record.kept => Some(row(
                harp,
                "keep",
                Verdict::Held,
                "it is marked keep: its transcript is kept at any age",
                format!("ctxloom session keep {harp} --undo"),
            )),
            State::Ended if view.captured.is_none() && view.record.purged_at.is_some() => {
                untouched += 1;
                continue;
            }
            State::Ended => None,
        };
        if let Some(decided) = decided {
            rows.push(decided);
            continue;
        }
        let Some(cutoff) = request.purge_cutoff else {
            rows.push(row(
                harp,
                "purge",
                Verdict::Held,
                "no --purge-older-than or session_purge_age is set, so nothing is purged",
                String::new(),
            ));
            continue;
        };
        let active = view.end().unwrap_or(view.record.started_at);
        if (active.seconds, active.nanos) > (cutoff.seconds, cutoff.nanos) {
            rows.push(row(
                harp,
                "purge",
                Verdict::Left,
                "it was active after the purge age",
                String::new(),
            ));
            continue;
        }
        rows.push(purge_row(service, harp, &purge, request.yes).await?);
    }
    let mut counts: BTreeMap<&'static str, u64> = [
        Verdict::Done,
        Verdict::Planned,
        Verdict::Held,
        Verdict::Left,
        Verdict::Skipped,
        Verdict::Failed,
    ]
    .iter()
    .map(|v| (v.name(), 0))
    .collect();
    for r in &rows {
        *counts.entry(r.verdict.name()).or_default() += 1;
    }
    Ok(SweepReport {
        purge_cutoff: request
            .purge_cutoff
            .map(|at| stamp::rfc3339(at)[..10].to_string()),
        rows,
        counts,
        untouched,
    })
}

/// The purge of one session, planned and, on `yes`, applied.
async fn purge_row<S: SessionService>(
    service: &mut S,
    harp: &str,
    command: &str,
    yes: bool,
) -> Result<SweepRow, ReclaimError> {
    let ask = |yes| RemoveRequest {
        scope: Scope::Purge,
        harp: harp.to_string(),
        yes,
        ..RemoveRequest::default()
    };
    let planned = match service.remove(ask(false)).await {
        Ok(planned) => planned,
        Err(error) => return Ok(failed(harp, &error)),
    };
    if let Some(refusal) = &planned.refusal {
        let (verdict, reason, command) = match refusal {
            Refusal::Retained { detail, .. } => (
                Verdict::Held,
                format!("its transcript is within the retention period ({detail})"),
                String::new(),
            ),
            Refusal::Kept { .. } => (
                Verdict::Held,
                "it is marked keep: its transcript is kept at any age".to_string(),
                format!("ctxloom session keep {harp} --undo"),
            ),
            Refusal::NotProvenDead { reason, .. } => (
                Verdict::Skipped,
                reason.clone(),
                format!("ctxloom session purge {harp} --even-if-live"),
            ),
        };
        return Ok(row(harp, "purge", verdict, &reason, command));
    }
    let bytes: u64 = planned.files.destroy.iter().map(|i| i.bytes).sum();
    if planned.files.destroy.is_empty() {
        return Ok(row(
            harp,
            "purge",
            Verdict::Left,
            "nothing to purge",
            String::new(),
        ));
    }
    if !yes {
        let mut r = row(harp, "purge", Verdict::Planned, "", command.to_string());
        r.bytes = bytes;
        return Ok(r);
    }
    match service.remove(ask(true)).await {
        Ok(done) => {
            let mut r = row(harp, "purge", Verdict::Done, "", String::new());
            r.bytes = done.files.bytes_freed;
            Ok(r)
        }
        Err(error) => Ok(failed(harp, &error)),
    }
}

fn failed(harp: &str, error: &AppError) -> SweepRow {
    row(
        harp,
        "purge",
        Verdict::Failed,
        &error.to_string(),
        String::new(),
    )
}
