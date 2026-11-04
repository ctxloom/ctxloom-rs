//! The sweep's table of rows, over a fake of the sessions use cases. Go:
//! j001300_closeout.feature's sweep scenarios and `operations.DecideSweep`
//! (release/0.7), minus the worktree rows (S11).

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use session_index::{Captured, SessionRecord, View};
use sessions_application::{
    AppError, BindRequest, Bound, FilePlan, KeepRequest, Kept, ListQuery, Refusal, Removal,
    RemoveRequest, RenameRequest, Renamed, Scope, SearchQuery, SessionService, ShowQuery, State,
    Timestamp, TranscriptQuery, TranscriptRow,
};

use crate::{sweep, SweepRequest, Verdict};

const NOW: i64 = 1_800_000_000;
const DAY: i64 = 86_400;

fn ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a fake never waits"),
    }
}

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn view(harp: &str, state: State, ended_days_ago: i64, kept: bool) -> View {
    View {
        record: SessionRecord {
            id: String::new(),
            harp: harp.to_string(),
            project_id: String::new(),
            project_dir: String::new(),
            output_dir: String::new(),
            engine: String::new(),
            origin: None,
            native_id: String::new(),
            transcript_path: String::new(),
            engine_version: String::new(),
            rotations: Vec::new(),
            kept,
            started_at: at(NOW - (ended_days_ago + 1) * DAY),
            ended_at: Some(at(NOW - ended_days_ago * DAY)),
            purged_at: None,
        },
        state,
        title: None,
        captured: Some(Captured {
            path: String::new(),
            bytes: 10,
        }),
    }
}

/// Lists its views; a purge answers what `refusal` says, and records what it
/// was asked.
#[derive(Default)]
struct Fake {
    views: Vec<View>,
    refusal: Option<Refusal>,
    asked: Vec<(String, bool, Scope)>,
}

fn nothing<T>() -> Result<T, AppError> {
    Err(AppError::NoSession {
        harp: String::new(),
    })
}

impl SessionService for Fake {
    async fn list(&mut self, _: ListQuery) -> Result<Vec<View>, AppError> {
        Ok(self.views.clone())
    }
    async fn search(&mut self, _: SearchQuery) -> Result<Vec<View>, AppError> {
        nothing()
    }
    async fn show(&mut self, _: ShowQuery) -> Result<View, AppError> {
        nothing()
    }
    async fn keep(&mut self, _: KeepRequest) -> Result<Kept, AppError> {
        nothing()
    }
    async fn remove(&mut self, request: RemoveRequest) -> Result<Removal, AppError> {
        self.asked
            .push((request.harp.clone(), request.yes, request.scope));
        let mut files = FilePlan::default();
        files.destroy.push(sessions_application::PlanItem {
            path: String::new(),
            rel: "transcripts/transcript.jsonl".to_string(),
            class: sessions_application::Class::Machine,
            bytes: 10,
            action: "destroy",
            reason: None,
        });
        files.bytes_freed = if request.yes { 10 } else { 0 };
        Ok(Removal {
            scope: request.scope,
            harp: request.harp,
            applied: request.yes && self.refusal.is_none(),
            index_entry_removed: false,
            files,
            refusal: self.refusal.clone(),
            dead_letters: Vec::new(),
        })
    }
    async fn rename(&mut self, _: RenameRequest) -> Result<Renamed, AppError> {
        nothing()
    }
    async fn transcripts(&mut self, _: TranscriptQuery) -> Result<Vec<TranscriptRow>, AppError> {
        nothing()
    }
    async fn transcript(&mut self, _: &str) -> Result<String, AppError> {
        nothing()
    }
    async fn bind(&mut self, _: BindRequest) -> Result<Bound, AppError> {
        nothing()
    }
}

fn request(yes: bool) -> SweepRequest {
    SweepRequest {
        all_projects: false,
        purge_cutoff: Some(at(NOW - 90 * DAY)),
        yes,
    }
}

fn verdict_of(report: &crate::SweepReport, harp: &str) -> Verdict {
    report
        .rows
        .iter()
        .find(|r| r.harp == harp)
        .unwrap_or_else(|| panic!("no row for {harp}"))
        .verdict
}

// j001300: "The sweep reports every session's fate before it changes anything".
#[test]
fn a_report_plans_each_old_session_and_purges_none() {
    let mut fake = Fake {
        views: vec![
            view("amber-quiet-heron", State::Ended, 120, false),
            view("brisk-copper-moth", State::Ended, 120, false),
        ],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(false))).unwrap();
    assert_eq!(verdict_of(&report, "amber-quiet-heron"), Verdict::Planned);
    assert_eq!(verdict_of(&report, "brisk-copper-moth"), Verdict::Planned);
    assert!(fake
        .asked
        .iter()
        .all(|(_, yes, scope)| !yes && *scope == Scope::Purge));
    assert_eq!(report.counts["planned"], 2);
    assert_eq!(
        report.rows[0].command,
        "ctxloom session purge amber-quiet-heron --yes"
    );
}

// j001300: "A running session and one whose liveness nothing can prove are
// skipped whole", with the manual route named.
#[test]
fn running_and_unproven_sessions_are_skipped_and_never_asked_about() {
    let mut fake = Fake {
        views: vec![
            view("teal-running-wren", State::Live, 120, false),
            view("slate-unproven-vole", State::Unknown, 120, false),
        ],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    assert!(fake.asked.is_empty());
    let running = &report.rows[0];
    assert_eq!(
        (running.verdict, running.reason.as_str()),
        (Verdict::Skipped, "it is running")
    );
    let unproven = &report.rows[1];
    assert_eq!(unproven.reason, "its liveness cannot be proven");
    assert_eq!(
        unproven.command,
        "ctxloom session purge slate-unproven-vole --even-if-live"
    );
}

// j001300: "a session marked keep is not [purged], and the report names the
// command that releases it".
#[test]
fn a_kept_session_is_held_and_the_release_is_named() {
    let mut fake = Fake {
        views: vec![view("brisk-copper-moth", State::Ended, 120, true)],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    assert_eq!(report.rows[0].verdict, Verdict::Held);
    assert_eq!(
        report.rows[0].command,
        "ctxloom session keep brisk-copper-moth --undo"
    );
    assert!(fake.asked.is_empty());
}

// session sweep --help: "Purging has NO default: without --purge-older-than or
// session_purge_age the purge rows are reported as held and never acted on."
#[test]
fn without_a_purge_age_every_purge_is_held() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 400, false)],
        ..Fake::default()
    };
    let none = SweepRequest {
        purge_cutoff: None,
        yes: true,
        ..SweepRequest::default()
    };
    let report = ready(sweep(&mut fake, &none)).unwrap();
    assert_eq!(report.rows[0].verdict, Verdict::Held);
    assert!(fake.asked.is_empty());
    assert_eq!(report.purge_cutoff, None);
}

// session sweep --help: "older than --purge-older-than".
#[test]
fn a_session_active_after_the_purge_age_is_left() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 10, false)],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    assert_eq!(report.rows[0].verdict, Verdict::Left);
    assert!(fake.asked.is_empty());
}

// session sweep --help: "ended within session_transcript_retention: never
// purged", the refusal the purge itself makes.
#[test]
fn a_refusal_of_the_purge_holds_the_session() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 120, false)],
        refusal: Some(Refusal::Retained {
            harp: "amber-quiet-heron".to_string(),
            detail: "it ended an hour ago".to_string(),
        }),
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    assert_eq!(report.rows[0].verdict, Verdict::Held);
    assert!(report.rows[0].reason.contains("retention"));
    assert!(fake.asked.iter().all(|(_, yes, _)| !yes));
}

// j001300: "The sweep removes only what it can prove is safe": on yes the
// plan is applied, and the bytes freed are reported.
#[test]
fn yes_purges_the_planned_session() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 120, false)],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    assert_eq!(report.rows[0].verdict, Verdict::Done);
    assert_eq!(report.rows[0].bytes, 10);
    assert_eq!(fake.asked.last().map(|a| a.1), Some(true));
}

// A purged session with nothing captured has nothing to do.
#[test]
fn a_session_already_emptied_is_untouched() {
    let mut emptied = view("amber-quiet-heron", State::Ended, 120, false);
    emptied.captured = None;
    emptied.record.purged_at = Some(at(NOW - 100 * DAY));
    let mut fake = Fake {
        views: vec![emptied],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    assert!(report.rows.is_empty());
    assert_eq!(report.untouched, 1);
}
