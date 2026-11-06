//! The sweep's table of rows, over a fake of the sessions use cases. Go:
//! j001300_closeout.feature's sweep scenarios and `operations.DecideSweep`
//! (release/0.7), minus the worktree rows (S11).

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use isolation_application::{IsolationError, Report, Row, Verdict as Worktree, WorktreeUse};
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
    /// The scratch checkouts the sessions left, with their verdicts.
    scratch: Vec<Row>,
    /// What `worktrees` was asked: the session and whether to apply.
    worktrees_asked: Vec<(Option<String>, bool)>,
}

impl WorktreeUse for Fake {
    fn worktrees(&mut self, harp: Option<&str>, apply: bool) -> Result<Report, IsolationError> {
        self.worktrees_asked.push((harp.map(str::to_string), apply));
        let rows: Vec<Row> = self
            .scratch
            .iter()
            .filter(|r| harp.is_none_or(|h| h == r.harp))
            .cloned()
            .collect();
        let count = |v: Worktree| rows.iter().filter(|r| r.verdict == v).count() as u64;
        Ok(Report {
            reaped: count(Worktree::Reaped),
            spared: count(Worktree::Spared),
            skipped: count(Worktree::Skipped),
            worktrees: rows,
            applied: apply,
        })
    }
}

fn scratch(harp: &str, name: &str, verdict: Worktree, reason: &str) -> Row {
    Row {
        harp: harp.to_string(),
        worktree: format!("ctxloom-wt-{name}"),
        path: format!("/home/.ctxloom/sessions/{harp}/work/ctxloom-wt-{name}"),
        owner_state: "dead",
        verdict,
        reason: reason.to_string(),
    }
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

// j001300 "The sweep removes only what it can prove is safe": the scratch
// worktrees are reclaimed through the worktree leaf, which the sweep asks
// with its own apply decision, and each outcome is a row of the report.
#[test]
fn the_sweep_asks_the_worktree_leaf_for_each_session_with_its_own_decision() {
    let mut fake = Fake {
        views: vec![
            view("olive-dirty-finch", State::Ended, 120, false),
            view("teal-running-wren", State::Live, 0, false),
        ],
        ..Fake::default()
    };
    ready(sweep(&mut fake, &request(false))).unwrap();
    assert_eq!(
        fake.worktrees_asked,
        vec![
            (Some("olive-dirty-finch".to_string()), false),
            (Some("teal-running-wren".to_string()), false)
        ]
    );
    fake.worktrees_asked.clear();
    ready(sweep(&mut fake, &request(true))).unwrap();
    assert!(fake.worktrees_asked.iter().all(|(_, apply)| *apply));
}

#[test]
fn each_scratch_checkout_is_a_row_with_its_verdict_and_reason() {
    let mut fake = Fake {
        views: vec![
            view("olive-dirty-finch", State::Ended, 120, false),
            view("teal-running-wren", State::Live, 0, false),
        ],
        scratch: vec![
            scratch("olive-dirty-finch", "clean", Worktree::Reaped, ""),
            scratch(
                "olive-dirty-finch",
                "wip",
                Worktree::Spared,
                "session olive-dirty-finch has ended, but the worktree has uncommitted changes (or unknown state)",
            ),
            scratch(
                "teal-running-wren",
                "own",
                Worktree::Skipped,
                "its session is running: its lock is held",
            ),
        ],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(true))).unwrap();
    let rows: Vec<_> = report
        .rows
        .iter()
        .filter(|r| r.action == "worktree")
        .collect();
    assert_eq!(rows.len(), 3);
    let by = |name: &str| {
        rows.iter()
            .find(|r| r.reason.contains(name) || r.command.contains(name))
    };
    assert_eq!(rows[0].verdict, Verdict::Done);
    assert_eq!(rows[1].verdict, Verdict::Held);
    assert!(rows[1].reason.contains("uncommitted changes"));
    assert_eq!(rows[2].verdict, Verdict::Skipped);
    assert!(by("running").is_some());
}

// Go's DecideSweep: a scratch worktree that must be preserved spares its
// session from purge (little-pamphlet), so its transcript is never asked of
// the purge, and a row says which worktree is why.
#[test]
fn a_session_with_a_preserved_worktree_is_spared_from_purge() {
    for verdict in [Worktree::Spared, Worktree::Skipped] {
        let mut fake = Fake {
            views: vec![
                view("olive-dirty-finch", State::Ended, 120, false),
                view("amber-quiet-heron", State::Ended, 120, false),
            ],
            scratch: vec![
                scratch("olive-dirty-finch", "clean", Worktree::Reaped, ""),
                scratch("olive-dirty-finch", "wip", verdict, "uncommitted changes"),
            ],
            ..Fake::default()
        };
        let report = ready(sweep(&mut fake, &request(true))).unwrap();
        assert_eq!(
            fake.asked,
            vec![
                ("amber-quiet-heron".to_string(), false, Scope::Purge),
                ("amber-quiet-heron".to_string(), true, Scope::Purge)
            ],
            "{verdict:?}: only the session without a preserved worktree is purged"
        );
        let spare: Vec<_> = report
            .rows
            .iter()
            .filter(|r| r.harp == "olive-dirty-finch" && r.action == "spare")
            .collect();
        assert_eq!(spare.len(), 1);
        assert_eq!(spare[0].verdict, Verdict::Left);
        assert!(spare[0].reason.contains("ctxloom-wt-wip"));
        assert!(spare[0].reason.contains("spared from reclaim and purge"));
        assert!(report
            .rows
            .iter()
            .all(|r| !(r.harp == "olive-dirty-finch" && r.action == "purge")));
    }
}

// A reaped or reapable checkout is no reason to spare the session.
#[test]
fn a_reaped_or_reapable_worktree_does_not_spare_its_session() {
    for verdict in [Worktree::Reaped, Worktree::Reapable] {
        let mut fake = Fake {
            views: vec![view("amber-quiet-heron", State::Ended, 120, false)],
            scratch: vec![scratch("amber-quiet-heron", "clean", verdict, "")],
            ..Fake::default()
        };
        let report = ready(sweep(&mut fake, &request(false))).unwrap();
        assert!(report.rows.iter().any(|r| r.action == "purge"));
        assert!(report.rows.iter().all(|r| r.action != "spare"));
    }
}

// A plan reaps nothing: a reapable checkout is planned, with the command.
#[test]
fn a_reapable_checkout_is_planned_naming_the_leaf_that_removes_it() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 120, false)],
        scratch: vec![scratch(
            "amber-quiet-heron",
            "clean",
            Worktree::Reapable,
            "",
        )],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(false))).unwrap();
    let row = report.rows.iter().find(|r| r.action == "worktree").unwrap();
    assert_eq!(row.verdict, Verdict::Planned);
    assert_eq!(
        row.command,
        "ctxloom session worktrees purge amber-quiet-heron --yes"
    );
}

// The purge is asked of the session the row is for, in the purge scope.
#[test]
fn the_purge_names_the_session_and_its_scope() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 120, false)],
        ..Fake::default()
    };
    ready(sweep(&mut fake, &request(true))).unwrap();
    assert_eq!(
        fake.asked,
        vec![
            ("amber-quiet-heron".to_string(), false, Scope::Purge),
            ("amber-quiet-heron".to_string(), true, Scope::Purge),
        ]
    );
}

// Untouched needs both: nothing captured AND already purged.
#[test]
fn a_session_with_only_one_of_the_emptied_marks_is_still_decided() {
    let mut uncaptured = view("amber-quiet-heron", State::Ended, 120, false);
    uncaptured.captured = None;
    let mut purged = view("brisk-quiet-heron", State::Ended, 120, false);
    purged.record.purged_at = Some(at(NOW - 100 * DAY));
    let mut fake = Fake {
        views: vec![uncaptured, purged],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(false))).unwrap();
    assert_eq!(report.untouched, 0);
    assert_eq!(report.rows.len(), 2);
}

// A session that ended exactly at the purge age is old enough.
#[test]
fn a_session_active_exactly_at_the_purge_age_is_purged() {
    let mut fake = Fake {
        views: vec![view("amber-quiet-heron", State::Ended, 90, false)],
        ..Fake::default()
    };
    let report = ready(sweep(&mut fake, &request(false))).unwrap();
    assert_eq!(report.rows[0].verdict, Verdict::Planned);
}

// The bytes of a row are written only when there are some.
#[test]
fn zero_bytes_are_skipped_and_others_are_not() {
    assert!(crate::sweep::is_zero(&0));
    assert!(!crate::sweep::is_zero(&1));
    assert!(!crate::sweep::is_zero(&10));
}
