//! Importing Go taskloom's task logs into each project's store.
//!
//! Each Go record becomes the command that does what it did, run at the
//! instant Go recorded it, so a task's creation, its status's last change
//! and its last deferral keep Go's times. The records are replayed in Go's
//! fold order (`eventLog.foldData`): a line repeating an earlier one byte
//! for byte is skipped, and the rest are sorted by time, stably, so the
//! file breaks ties. Go's fold decides which records count, as it did
//! for Go:
//!
//! - an add for a harp the log already issued is a displaced duplicate
//!   (two tasks minted the same harp). The first keeps it. A displaced add
//!   that a later add written by Go's `repair` names (`repair_of`) was
//!   re-added under a fresh harp, which imports as any add does; one no
//!   repair names is not imported, and is reported.
//! - a record to a harp with no live task (never added, or removed) is
//!   ignored, as Go's fold ignores it.
//! - a tag or untag that changes no tag records nothing.
//!
//! What the tasks domain refuses that Go's fold accepted (empty text, a
//! Deferred status with no trigger, a status starting with `@`) is not
//! imported, and is reported; so is every later record to a task whose add
//! was not imported. A Go tag that is not a tag in tagma's grammar is left
//! out of its record's command, and reported; the rest of the record is
//! imported. Go's fold still counts it, so a later record is judged as Go
//! judged it.
//!
//! Re-running an import duplicates nothing: a record's command id derives
//! from its line ([`record_command_id`]), so a record already imported is
//! found in its task's stream and skipped, and an import that stopped
//! part way resumes. A log Go has appended to since is imported up to its
//! new end.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::future::Future;

use annals::next::v1::Uuid;
use annals::next::{ids, AsyncEventStore, Timestamp};
use annals_diagnostic::{Arg, Diagnostic, Report};
use ports::{Clock, Delay, GoOp, GoRecord, GoTaskLogs, Jitter, ProjectIdentity, TaskStores};
use tasks::{
    AddTask, EditText, RemovalReason, RemoveTask, Retag, SetStatus, TaskCommand, TaskRejection,
};

use crate::tags::{self, Tag, TagRefusal};
use crate::{book, cover, retry, text_hash, AppError, Service};

/// What to import: every project Go keeps a log for, or the one named.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ImportRequest {
    pub project: Option<String>,
}

/// What an import did, project by project.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImportReport {
    pub projects: Vec<ProjectImport>,
}

impl ImportReport {
    /// Whether everything was brought over: no project failed and nothing
    /// was left behind.
    pub fn complete(&self) -> bool {
        self.projects
            .iter()
            .all(|p| p.failed.is_none() && p.lost.is_empty())
    }
}

/// What importing one project's log did.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ProjectImport {
    pub project_id: String,
    /// Why the import stopped: the log could not be read, the project id
    /// is not valid, or its store failed. What was imported before stays,
    /// and a re-run resumes from it.
    pub failed: Option<Report>,
    /// The log's records (its non-blank lines).
    pub records: usize,
    /// Lines repeating an earlier one byte for byte, which Go's fold skips.
    pub repeated: usize,
    /// Tasks this run added.
    pub tasks: usize,
    /// Records this run imported, adds included.
    pub applied: usize,
    /// Records an earlier run imported.
    pub already: usize,
    /// Tags and untags that change no tag, recording nothing in Go's fold
    /// or here.
    pub unchanged: usize,
    /// Records Go's fold ignores: to a harp with no live task.
    pub ignored: usize,
    /// Displaced duplicate adds that Go's `repair` re-added under a fresh
    /// harp: the re-add is imported, under that harp.
    pub repaired: usize,
    /// What could not be brought over.
    pub lost: Vec<Lost>,
}

/// A record, or a tag of one, that could not be brought over.
#[derive(Clone, PartialEq, Debug)]
pub struct Lost {
    /// Its line in the log.
    pub line: u32,
    pub op: GoOp,
    pub harp_id: String,
    /// Its text, if it carries any: an add's or an edit's.
    pub text: String,
    pub why: Why,
}

/// Why a record was not brought over.
#[derive(Clone, PartialEq, Debug)]
pub enum Why {
    /// An add for a harp another task already holds in Go's log, which no
    /// repair re-added: Go's fold keeps the first task. Run Go's
    /// `taskloom repair`, then import again.
    Displaced,
    /// The tasks domain refused it.
    Refused(Report),
    /// The store already holds a task under this harp, one not imported
    /// from this record.
    HarpTaken,
    /// A record to a task whose add was not imported.
    TaskNotImported,
    /// One of the record's tags, not a tag in tagma's grammar; the rest of
    /// the record was imported.
    Tag(TagRefusal),
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Why::Displaced => f.write_str(
                "another task holds this harp in Go's log: run Go's `taskloom repair`, then import again",
            ),
            Why::Refused(rejection) => rejection.fmt(f),
            Why::HarpTaken => f.write_str("a task not imported from this record already holds this harp"),
            Why::TaskNotImported => f.write_str("its task was not imported"),
            Why::Tag(refusal) => refusal.fmt(f),
        }
    }
}

impl Diagnostic for Why {
    fn code(&self) -> &'static str {
        match self {
            Why::Displaced => "tasks-application-import-displaced",
            Why::Refused(rejection) => rejection.code(),
            Why::HarpTaken => "tasks-application-import-harp-taken",
            Why::TaskNotImported => "tasks-application-import-task-not-imported",
            Why::Tag(refusal) => refusal.code(),
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            Why::Refused(rejection) => rejection.args(),
            Why::Tag(refusal) => refusal.args(),
            _ => Vec::new(),
        }
    }
}

/// Go's logs, as a service without any holds them: none.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct NoGoLogs;

/// The import use case, as a driving adapter calls it. Its future is
/// `Send`, so a server can spawn it.
pub trait ImportService {
    fn import(
        &mut self,
        request: ImportRequest,
    ) -> impl Future<Output = Result<ImportReport, AppError>> + Send;
}

impl<I, S, N, C, D, L, G> ImportService for Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity + Send,
    S: TaskStores + Send,
    N: Send,
    C: Send,
    D: Delay + Jitter + Send,
    L: GoTaskLogs + Send,
    G: Send,
{
    async fn import(&mut self, request: ImportRequest) -> Result<ImportReport, AppError> {
        let projects = match request.project {
            Some(project) => vec![project],
            None => self.go_logs.projects().map_err(AppError::Failed)?,
        };
        let mut report = ImportReport::default();
        for project_id in projects {
            let mut project = ProjectImport {
                project_id,
                ..ProjectImport::default()
            };
            if let Err(why) = self.import_project(&mut project).await {
                project.failed = Some(why);
            }
            report.projects.push(project);
        }
        Ok(report)
    }
}

impl<I, S, N, C, D, L, G> Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity + Send,
    S: TaskStores + Send,
    D: Delay + Jitter + Send,
    L: GoTaskLogs + Send,
{
    /// Import one project's log, counting into `report` as it goes.
    async fn import_project(&mut self, report: &mut ProjectImport) -> Result<(), Report> {
        let id = report.project_id.clone();
        let records = self.go_logs.records(&id)?.ok_or_else(|| {
            Report::of(&AppError::NoGoLog {
                project: id.clone(),
            })
        })?;
        report.records = records.len();
        let (records, repeated) = replay_order(records);
        report.repeated = repeated;
        if records.is_empty() {
            return Ok(());
        }
        // An explicit id resolves without binding any directory to it.
        let project = self.identity.for_write(Some(&id))?;
        let store = self
            .stores
            .open(&project)
            .await
            .map_err(|e| Report::of(&e))?;
        let mut fold = Fold::new(&records);
        for record in &records {
            let step = fold.step(record);
            for refusal in fold.unparsed.drain(..) {
                report.lost.push(lost(record, Why::Tag(refusal)));
            }
            let command = match step {
                Step::Run(command) => command,
                Step::NothingLeft => continue,
                Step::Unchanged => {
                    report.unchanged += 1;
                    continue;
                }
                Step::Ignored => {
                    report.ignored += 1;
                    continue;
                }
                Step::Repaired => {
                    report.repaired += 1;
                    continue;
                }
                Step::Lost(why) => {
                    report.lost.push(lost(record, why));
                    continue;
                }
            };
            let cover = cover(&record.task);
            let command_id = record_command_id(&record.raw);
            let found = store
                .find_command(&cover, &command_id)
                .await
                .map_err(|e| Report::of(&e))?;
            if !found.is_empty() {
                report.already += 1;
                continue;
            }
            let mut at = At(record.ts);
            let mut runner = retry::Runner {
                registry: &self.registry,
                clock: &mut at,
                delay: &mut self.delay,
                backoff: self.backoff,
            };
            let book = book(&record.task, command);
            match runner
                .run(store, &book, &command_id)
                .await
                .map_err(|e| Report::of(&e))?
            {
                retry::Outcome::Committed => {
                    report.applied += 1;
                    if record.op == GoOp::Add {
                        report.tasks += 1;
                    }
                }
                retry::Outcome::Refused(rejection) => {
                    if record.op == GoOp::Add {
                        fold.not_imported(&record.task);
                    }
                    let why = match rejection {
                        TaskRejection::TaskExists { .. } => Why::HarpTaken,
                        other => Why::Refused(Report::of(&other)),
                    };
                    report.lost.push(lost(record, why));
                }
            }
        }
        Ok(())
    }
}

fn lost(record: &GoRecord, why: Why) -> Lost {
    Lost {
        line: record.line,
        op: record.op,
        harp_id: record.task.clone(),
        text: record.text.clone(),
        why,
    }
}

/// A clock stopped at the instant a record was recorded.
struct At(Timestamp);

impl Clock for At {
    fn now(&mut self) -> Timestamp {
        self.0
    }
}

/// The records in Go's replay order, each line once, and how many lines
/// repeated an earlier one.
fn replay_order(records: Vec<GoRecord>) -> (Vec<GoRecord>, usize) {
    let total = records.len();
    let mut seen = BTreeSet::new();
    let mut once: Vec<GoRecord> = records
        .into_iter()
        .filter(|r| seen.insert(r.raw.clone()))
        .collect();
    // Stable: records at one instant keep the log's order.
    once.sort_by_key(|r| (r.ts.seconds, r.ts.nanos));
    let repeated = total - once.len();
    (once, repeated)
}

/// What one record comes to.
#[derive(Debug, PartialEq)]
enum Step {
    /// Send this command to the record's task.
    Run(TaskCommand),
    /// A retag that changes nothing.
    Unchanged,
    /// Go's fold ignores it.
    Ignored,
    /// A displaced add that a repair re-added.
    Repaired,
    /// It cannot be brought over.
    Lost(Why),
    /// A retag that changed Go's tags only by tags left out, unparseable.
    NothingLeft,
}

/// Go's fold, as far as it decides which records count: the harps the log
/// has issued, the live tasks' tags, and the repairs made.
struct Fold {
    issued: BTreeSet<String>,
    live: BTreeMap<String, BTreeSet<String>>,
    repaired: BTreeSet<String>,
    /// Harps whose add was not imported.
    missing: BTreeSet<String>,
    /// The tags of the last record stepped that are not tags in tagma's
    /// grammar, left out of its command.
    unparsed: Vec<TagRefusal>,
}

impl Fold {
    /// The fold of `records`, which must be in replay order; the repairs
    /// are read ahead, since Go counts a repair wherever it falls.
    fn new(records: &[GoRecord]) -> Fold {
        Fold {
            issued: BTreeSet::new(),
            live: BTreeMap::new(),
            repaired: records
                .iter()
                .filter(|r| r.op == GoOp::Add && !r.repair_of.is_empty())
                .map(|r| r.repair_of.clone())
                .collect(),
            missing: BTreeSet::new(),
            unparsed: Vec::new(),
        }
    }

    /// `spelled` as tags, those that do not parse left out and kept in
    /// [`Fold::unparsed`].
    fn parsed<'a>(&mut self, spelled: impl IntoIterator<Item = &'a String>) -> Vec<Tag> {
        let mut parsed = Vec::new();
        for result in spelled.into_iter().filter_map(|s| tags::parse(s)) {
            match result {
                Ok(tag) => parsed.push(tag),
                Err(refusal) => self.unparsed.push(refusal),
            }
        }
        parsed
    }

    /// Keep in [`Fold::unparsed`] those of `spelled` that do not parse;
    /// whether there were any.
    fn unparsed_of(&mut self, spelled: &BTreeSet<String>) -> bool {
        let before = self.unparsed.len();
        self.parsed(spelled);
        self.unparsed.len() > before
    }

    /// The task `harp_id`'s add was not imported.
    fn not_imported(&mut self, harp_id: &str) {
        self.missing.insert(harp_id.to_string());
    }

    fn step(&mut self, record: &GoRecord) -> Step {
        if record.op == GoOp::Add {
            return self.add(record);
        }
        let harp = &record.task;
        let Some(held) = self.live.get_mut(harp) else {
            return Step::Ignored;
        };
        // Go's fold changes as Go's did, whether or not the task was
        // imported, so a later record is judged as Go judged it.
        let command: Option<TaskCommand> = match record.op {
            GoOp::Tag | GoOp::Untag => {
                let given = normalized(&record.tags);
                let after: BTreeSet<String> = if record.op == GoOp::Tag {
                    held.union(&given).cloned().collect()
                } else {
                    held.difference(&given).cloned().collect()
                };
                // The task's tags are Go's, parsed: two Go spellings of
                // one tag (`k=v`, `k="v"`) are one tag here, so the retag
                // takes the task from Go's tags before to Go's tags after,
                // as tags, whatever the record named.
                let before = parsed_set(held);
                let now = parsed_set(&after);
                *held = after;
                let unparsed = self.unparsed_of(&given);
                let add: Vec<Tag> = now
                    .iter()
                    .filter(|(spelled, _)| !before.contains_key(*spelled))
                    .map(|(_, tag)| tag.clone())
                    .collect();
                let remove: Vec<Tag> = before
                    .iter()
                    .filter(|(spelled, _)| !now.contains_key(*spelled))
                    .map(|(_, tag)| tag.clone())
                    .collect();
                if add.is_empty() && remove.is_empty() {
                    let step = if unparsed {
                        Step::NothingLeft
                    } else {
                        Step::Unchanged
                    };
                    return self.unless_missing(harp, step);
                }
                Some(Retag { add, remove }.into())
            }
            GoOp::Status => Some(
                SetStatus {
                    status: record.status.clone(),
                    trigger: record.trigger.clone(),
                }
                .into(),
            ),
            GoOp::Text => Some(
                EditText {
                    text: record.text.clone(),
                }
                .into(),
            ),
            // An add was taken above.
            GoOp::Remove | GoOp::Add => {
                self.live.remove(harp);
                Some(
                    RemoveTask {
                        reason: RemovalReason::Removed,
                    }
                    .into(),
                )
            }
        };
        self.unless_missing(harp, command.map_or(Step::Unchanged, Step::Run))
    }

    /// `step`, unless `harp`'s add was not imported.
    fn unless_missing(&mut self, harp: &str, step: Step) -> Step {
        if self.missing.contains(harp) {
            self.unparsed.clear();
            return Step::Lost(Why::TaskNotImported);
        }
        step
    }

    /// An add: the task, unless the harp is already issued.
    fn add(&mut self, record: &GoRecord) -> Step {
        let harp = &record.task;
        if !self.issued.insert(harp.clone()) {
            return if self.repaired.contains(&anomaly_key(record)) {
                Step::Repaired
            } else {
                Step::Lost(Why::Displaced)
            };
        }
        self.live.insert(harp.clone(), normalized(&record.tags));
        let tags = self.parsed(&record.tags);
        Step::Run(
            AddTask {
                harp_id: harp.clone(),
                text: record.text.clone(),
                status: record.status.clone(),
                trigger: record.trigger.clone(),
                tags,
                origin_session: record.session.clone(),
            }
            .into(),
        )
    }
}

/// The tags of Go's `spelled` that parse, by their canonical spelling.
fn parsed_set(spelled: &BTreeSet<String>) -> BTreeMap<String, Tag> {
    spelled
        .iter()
        .filter_map(|s| tags::parse(s)?.ok())
        .map(|tag| (tag.to_string(), tag))
        .collect()
}

/// Tags trimmed, empties dropped, deduplicated and sorted, as Go
/// normalizes them.
fn normalized(tags: &[String]) -> BTreeSet<String> {
    tags.iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// What a command id derived from a Go record's line is prefixed with, so
/// it can equal no other id derived the same way.
const RECORD_ID_PREFIX: &str = "ctxloom-go-task-log:";

/// The command id a Go record imports under: a UUIDv5 of its line. A Go
/// record is an external fact, and this is the id annals gives a fact
/// (`ids::fact_command_id`) whose external id is the line, prefixed. Go's
/// fold counts a line once, so within a log the line is the record's
/// identity.
pub fn record_command_id(raw: &str) -> Uuid {
    ids::fact_command_id(&format!("{RECORD_ID_PREFIX}{raw}"))
}

/// The key Go's `repair` names a displaced add by (`anomalyKey`): the text
/// hash of its harp, text, session and instant, NUL-separated.
pub fn anomaly_key(record: &GoRecord) -> String {
    text_hash(&format!(
        "{}\0{}\0{}\0{}",
        record.task,
        record.text,
        record.session,
        rfc3339_nano(record.ts)
    ))
}

/// `at` in UTC as Go's `time.RFC3339Nano` writes it: the fraction's
/// trailing zeros dropped, and the fraction too when it is zero.
pub fn rfc3339_nano(at: Timestamp) -> String {
    let (year, month, day) = civil(at.seconds.div_euclid(86_400));
    let second = at.seconds.rem_euclid(86_400);
    let mut text = format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        second / 3600,
        second % 3600 / 60,
        second % 60
    );
    if at.nanos != 0 {
        let fraction = format!("{:09}", at.nanos);
        text.push('.');
        text.push_str(fraction.trim_end_matches('0'));
    }
    text.push('Z');
    text
}

/// The proleptic Gregorian date `days` after 1970-01-01: Howard Hinnant's
/// `civil_from_days`.
fn civil(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let of_era = days.rem_euclid(146_097);
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "import.test.rs"]
mod tests;
