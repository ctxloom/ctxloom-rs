//! Pruning: taking old finished tasks out of a project's store, as Go's
//! `taskloom prune` takes them out of its log.
//!
//! A task is selected when its status is one of those asked for, its
//! status last changed before the cut (an age back from now, or a date),
//! and it matches the tag query, if one is given. Without `apply` the
//! prune only reports what it would take. With it, each selected task is
//! removed (`TaskRemoved{PRUNED}`), so every list drops it at once, and
//! then its stream is purged, which frees its harp: a removal by name
//! keeps both. A surviving task that still names a pruned one (a tag
//! carrying its harp, such as `relates:<harp>`, or the harp as a word of
//! its text or trigger) is reported, and left as written.
//!
//! Each removal is decided in a unit of work that first reads the task
//! and declines when it no longer qualifies (moved, retagged or pruned
//! since the selection), and each purge in a unit that first checks the
//! stream still holds the removed task, so a task minted on the freed harp
//! meanwhile is never purged.
//!
//! A purge that failed for good after its removal landed leaves a prune
//! half done: the task removed, its stream and harp still held. Every
//! prune finishes one, whatever it selects by: it is reported, and purged
//! when applied.

use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;

use annals::next::{AsyncReadStore, Timestamp};
use annals_diagnostic::{Arg, Diagnostic, Report};
use ports::{
    Clock, CommandIds, Configuration, Delay, Jitter, ProjectIdentity, PurgingStore, TaskStores,
};
use task_lists::TaskList;
use tasks::{RemovalReason, RemoveTask, TaskRejection, TaskState, STATUS_CLASS_SIGIL};

use crate::retry::{Outcome, Runner};
use crate::{book, cover, failed, query, rejected, AppError, Project, Service, Shown, Task, DONE};

/// The age a prune takes by default: Go's `--older-than 7d`.
pub const DEFAULT_OLDER_THAN: &str = "7d";
/// The statuses a prune takes by default: Go's `--status Done`. Archived
/// is left out, so archiving is how a finished task is kept.
pub const DEFAULT_PRUNE_STATUSES: [&str; 1] = [DONE];

/// Which tasks to prune, and whether to.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct PruneRequest {
    pub project: Option<String>,
    /// The statuses that qualify a task; at least one, none starting with
    /// `@`.
    pub statuses: Vec<String>,
    /// How long ago a qualifying task's status last changed, at the
    /// least: an offset back from now (`30d`, `12w`, `720h`, Go's
    /// durations with days and weeks), or a date (`2026-01-01`, midnight
    /// UTC).
    pub older_than: String,
    /// A tag query, postfix or infix; empty for none.
    pub tag_query: String,
    /// Remove and purge what is selected; without it, only report.
    pub apply: bool,
    /// Configuration overrides, `<path>=<value>` (`--config-set`).
    pub config_set: Vec<String>,
}

/// One task a prune took, or without `apply` would take.
#[derive(Clone, PartialEq, Debug)]
pub struct Pruned {
    pub task: Task,
    /// When its status last changed: what put it past the cut.
    pub status_set_at: Timestamp,
}

/// Where a surviving task names a pruned one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Via {
    /// A tag carrying the harp, in its canonical spelling.
    Tag(String),
    /// The harp as a word of the task's text.
    Text,
    /// The harp as a word of the task's trigger.
    Trigger,
}

/// As Go spells it: `tag <tag>`, `text` or `trigger`.
impl fmt::Display for Via {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Via::Tag(tag) => write!(f, "tag {tag}"),
            Via::Text => f.write_str("text"),
            Via::Trigger => f.write_str("trigger"),
        }
    }
}

/// A surviving task still naming a pruned one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Dangling {
    /// The surviving task holding the reference.
    pub harp_id: String,
    /// The pruned harp it names.
    pub pruned: String,
    pub via: Via,
}

/// What a prune selected, and what taking it costs.
#[derive(Clone, PartialEq, Debug)]
pub struct Pruning {
    /// The project pruned; `None` when the work root has none yet.
    pub project: Option<Project>,
    /// The statuses selected by.
    pub statuses: Vec<String>,
    /// The cut: a selected task's status last changed before it.
    pub before: Timestamp,
    pub tag_query: String,
    /// Whether the tasks were taken, or only reported.
    pub applied: bool,
    /// The tasks taken (or that would be), in the order added.
    pub pruned: Vec<Pruned>,
    /// The events purged with them, the removal each prune records first
    /// included: as the store reports them, or as it would.
    pub events_removed: u64,
    /// The live tasks left.
    pub tasks_remaining: usize,
    /// Every place a task left names a pruned one.
    pub dangling: Vec<Dangling>,
    /// Which of the tasks' tags a person is shown.
    pub shown: Shown,
}

/// Why a prune was refused before it read anything.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PruneRefusal {
    /// No status to select by: an empty set is never read as every status.
    NoStatus,
    /// No age to select by.
    AgeRequired,
    /// An offset of zero or less.
    AgeNotPositive { age: String },
    /// Neither an offset nor a date.
    AgeUnreadable { age: String },
}

impl fmt::Display for PruneRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PruneRefusal::NoStatus => f.write_str(
                "a prune needs at least one status to select by: an empty status set is never read as every status",
            ),
            PruneRefusal::AgeRequired => {
                f.write_str("a prune needs an age: an offset like 30d, or a date like 2026-01-01")
            }
            PruneRefusal::AgeNotPositive { age } => {
                write!(f, "the age {age} is not a positive age")
            }
            PruneRefusal::AgeUnreadable { age } => write!(
                f,
                "the age {age} is neither an offset (30d, 12w, 720h) nor a date (2026-01-01)"
            ),
        }
    }
}

impl Diagnostic for PruneRefusal {
    fn code(&self) -> &'static str {
        match self {
            PruneRefusal::NoStatus => "tasks-application-prune-no-status",
            PruneRefusal::AgeRequired => "tasks-application-prune-age-required",
            PruneRefusal::AgeNotPositive { .. } => "tasks-application-prune-age-not-positive",
            PruneRefusal::AgeUnreadable { .. } => "tasks-application-prune-age-unreadable",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            PruneRefusal::AgeNotPositive { age } | PruneRefusal::AgeUnreadable { age } => {
                vec![("age", age.as_str().into())]
            }
            PruneRefusal::NoStatus | PruneRefusal::AgeRequired => Vec::new(),
        }
    }
}

/// One of every refusal, for the catalog test.
pub(crate) fn samples() -> Vec<PruneRefusal> {
    let age = || "0d".to_string();
    vec![
        PruneRefusal::NoStatus,
        PruneRefusal::AgeRequired,
        PruneRefusal::AgeNotPositive { age: age() },
        PruneRefusal::AgeUnreadable { age: age() },
    ]
}

/// The prune use case, as a driving adapter calls it: opt-in, since it
/// needs a store that purges ([`PurgingStore`]). Its future is `Send`, so
/// a server can spawn it.
pub trait PruneService {
    fn prune(
        &mut self,
        request: PruneRequest,
    ) -> impl Future<Output = Result<Pruning, AppError>> + Send;
}

impl<I, S, N, C, D, L, G> PruneService for Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity + Send,
    S: TaskStores + Send,
    S::Store: PurgingStore,
    N: Send,
    C: Clock + CommandIds + Send,
    D: Delay + Jitter + Send,
    L: Send,
    G: Configuration + Send,
{
    async fn prune(&mut self, request: PruneRequest) -> Result<Pruning, AppError> {
        query::check(&request.tag_query).map_err(|e| AppError::TagQuery(Report::of(&e)))?;
        let schema = self.schema(&request.config_set)?;
        let statuses = statuses(&request.statuses)?;
        let before = cut(&request.older_than, self.clock.now()).map_err(|e| refused(&e))?;
        let mut pruning = Pruning {
            project: None,
            statuses,
            before,
            tag_query: request.tag_query.clone(),
            applied: request.apply,
            pruned: Vec::new(),
            events_removed: 0,
            tasks_remaining: 0,
            dangling: Vec::new(),
            shown: Shown::of(&schema),
        };
        // A prune resolves its project as a read does: where there is none,
        // or no store yet, there is nothing to prune, and nothing is minted
        // or created to say so.
        let Some(project) = self
            .identity
            .for_read(request.project.as_deref())
            .map_err(AppError::Identity)?
        else {
            return Ok(pruning);
        };
        pruning.project = Some(project.clone());
        if self
            .stores
            .open_existing(&project)
            .await
            .map_err(failed)?
            .is_none()
        {
            return Ok(pruning);
        }
        let store = if request.apply {
            self.stores.open(&project).await.map_err(failed)?
        } else {
            self.stores
                .open_existing(&project)
                .await
                .map_err(failed)?
                .ok_or(AppError::NoProject)?
        };

        let (tasks, unpurged) = live_tasks(store, &self.registry).await?;
        let selection = Selection {
            statuses: &pruning.statuses,
            before,
            tag_query: &request.tag_query,
            schema: &schema,
        };
        let matched = query::select(
            &request.tag_query,
            tasks
                .iter()
                .map(|t| (t.harp_id.as_str(), t.tags.as_slice())),
            &schema,
        )
        .map_err(|e| AppError::TagQuery(Report::of(&e)))?;
        // A task a prune removed but left unpurged was selected by that
        // prune: this one finishes it, whatever it selects by.
        let mut selected: Vec<&TaskState> = tasks
            .iter()
            .filter(|t| matched.contains(&t.harp_id) && selection.dated(t))
            .chain(&unpurged)
            .collect();
        selected.sort_by(|a, b| added(a).cmp(&added(b)));

        let mut runner = Runner {
            registry: &self.registry,
            clock: &mut self.clock,
            delay: &mut self.delay,
            backoff: self.backoff,
        };
        for task in selected {
            let Some(events) = take(&mut runner, store, task, &selection, request.apply).await?
            else {
                continue;
            };
            pruning.events_removed += events;
            pruning.pruned.push(Pruned {
                task: Task::of(task),
                status_set_at: task.status_set_at,
            });
        }

        let gone: BTreeSet<&str> = pruning
            .pruned
            .iter()
            .map(|p| p.task.harp_id.as_str())
            .collect();
        let survivors: Vec<&TaskState> = tasks
            .iter()
            .filter(|t| !gone.contains(t.harp_id.as_str()))
            .collect();
        pruning.tasks_remaining = survivors.len();
        pruning.dangling = dangling(&survivors, &gone);
        Ok(pruning)
    }
}

/// Every live task in `store`, and every task a prune removed but left
/// unpurged, each in the order added: one moment of the store.
async fn live_tasks<S: AsyncReadStore>(
    store: &mut S,
    registry: &annals::next::Registry,
) -> Result<(Vec<TaskState>, Vec<TaskState>), AppError> {
    let mut list = TaskList::new(Box::new(|_| true));
    list.catch_up_async(store, registry)
        .await
        .map_err(|e| AppError::Failed(Report::of(&*e)))?;
    let in_order = |tasks: Vec<&TaskState>| {
        let mut tasks: Vec<TaskState> = tasks.into_iter().cloned().collect();
        tasks.sort_by(|a, b| added(a).cmp(&added(b)));
        tasks
    };
    Ok((in_order(list.entries()), in_order(list.unpurged())))
}

/// Where `task` falls in the order added: when it was, then its harp.
fn added(task: &TaskState) -> (i64, i32, &str) {
    (
        task.created_at.seconds,
        task.created_at.nanos,
        task.harp_id.as_str(),
    )
}

/// Take `task`, as selected: remove it and purge its stream when
/// `apply`, else only count what that would delete. A task already
/// removed is one a prune removed and left unpurged: it is only purged.
/// The events deleted, or `None` when the task was not taken after all:
/// it no longer qualifies, or another removed or purged it since it was
/// selected.
async fn take<S: PurgingStore, C: Clock + CommandIds, D: Delay + Jitter>(
    runner: &mut Runner<'_, C, D>,
    store: &mut S,
    task: &TaskState,
    selection: &Selection<'_>,
    apply: bool,
) -> Result<Option<u64>, AppError> {
    let stream = cover(&task.harp_id);
    if !apply {
        let held = store.read_from(&stream, 0).await.map_err(failed)?;
        // Its events, and the removal a prune records before it purges,
        // unless it holds that already.
        let removal = u64::from(!task.removed);
        return Ok(Some(
            held.map_or(0, |slice| slice.pages.len() as u64) + removal,
        ));
    }
    if task.removed {
        let deleted = runner.purge(store, &stream).await?;
        return Ok((deleted > 0).then_some(deleted));
    }
    let command_id = runner.clock.command_id();
    let removal = RemoveTask {
        reason: RemovalReason::Pruned,
    };
    let book = book(&task.harp_id, removal.into());
    // A task removed since is let through, for the kernel to answer: this
    // prune's own removal, landed on an attempt whose answer was lost, as
    // a duplicate; anyone else's as refused.
    let holds = |state: &TaskState| state.removed || selection.holds(state);
    match runner
        .run_where(store, &book, &command_id, Some(&holds))
        .await?
    {
        Some(Outcome::Committed) => runner.purge(store, &stream).await.map(Some),
        None | Some(Outcome::Refused(TaskRejection::Removed { .. } | TaskRejection::NoTask)) => {
            Ok(None)
        }
        Some(Outcome::Refused(rejection)) => Err(rejected(&rejection)),
    }
}

/// What a prune selects by.
struct Selection<'a> {
    statuses: &'a [String],
    before: Timestamp,
    tag_query: &'a str,
    schema: &'a crate::TagSchema,
}

impl Selection<'_> {
    /// Whether `task` is live, in one of the statuses, and last changed
    /// status before the cut.
    fn dated(&self, task: &TaskState) -> bool {
        let at = task.status_set_at;
        !task.removed
            && self.statuses.contains(&task.status)
            && (at.seconds, at.nanos) < (self.before.seconds, self.before.nanos)
    }

    /// Whether `task` qualifies whole: dated, and matching the tag query.
    fn holds(&self, task: &TaskState) -> bool {
        self.dated(task)
            && query::select(
                self.tag_query,
                [(task.harp_id.as_str(), task.tags.as_slice())],
                self.schema,
            )
            .is_ok_and(|matched| matched.contains(&task.harp_id))
    }
}

/// The statuses asked for, trimmed and empties dropped: at least one, and
/// none claiming the status-class sigil (`@open` and `@terminal` are not
/// taken yet, as in a list).
fn statuses(asked: &[String]) -> Result<Vec<String>, AppError> {
    let statuses: Vec<String> = asked
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if let Some(class) = statuses.iter().find(|s| s.starts_with(STATUS_CLASS_SIGIL)) {
        return Err(rejected(&TaskRejection::StatusReserved {
            status: class.clone(),
        }));
    }
    if statuses.is_empty() {
        return Err(refused(&PruneRefusal::NoStatus));
    }
    Ok(statuses)
}

fn refused(refusal: &PruneRefusal) -> AppError {
    AppError::Prune(Report::of(refusal))
}

/// Every place one of `survivors` names a harp in `gone`: a tag carrying it
/// as a word (its key or value, `relates:<harp>`), or the harp as a word
/// of the text or the trigger. Each harp once per tag, text or trigger.
fn dangling(survivors: &[&TaskState], gone: &BTreeSet<&str>) -> Vec<Dangling> {
    let mut found = Vec::new();
    for task in survivors {
        let mut note = |text: &str, via: Via| {
            let mut seen = BTreeSet::new();
            for word in harp_words(text) {
                if gone.contains(word) && seen.insert(word) {
                    found.push(Dangling {
                        harp_id: task.harp_id.clone(),
                        pruned: word.to_string(),
                        via: via.clone(),
                    });
                }
            }
        };
        for tag in &task.tags {
            let spelled = tag.to_string();
            note(&spelled, Via::Tag(spelled.clone()));
        }
        note(&task.text, Via::Text);
        note(&task.trigger, Via::Trigger);
    }
    found
}

/// The runs of the characters a harp is spelled with (lowercase letters,
/// digits and hyphens) in `text`, so a harp is matched as a whole word and
/// never as part of a longer hyphenated one: Go's `harpWords`.
fn harp_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
        .filter(|word| !word.is_empty())
}

const NANOS_PER_SECOND: i128 = 1_000_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// The instant `age` bounds, read at `now`: an offset back from now, or a
/// date's midnight, UTC. Go's `agebound.Parse`.
pub fn cut(age: &str, now: Timestamp) -> Result<Timestamp, PruneRefusal> {
    let trimmed = age.trim();
    if trimmed.is_empty() {
        return Err(PruneRefusal::AgeRequired);
    }
    if let Some(nanos) = offset(trimmed) {
        if nanos <= 0 {
            return Err(PruneRefusal::AgeNotPositive {
                age: age.to_string(),
            });
        }
        let at = i128::from(now.seconds) * NANOS_PER_SECOND + i128::from(now.nanos) - nanos;
        return Ok(Timestamp {
            seconds: i64::try_from(at.div_euclid(NANOS_PER_SECOND)).unwrap_or(i64::MIN),
            nanos: i32::try_from(at.rem_euclid(NANOS_PER_SECOND)).unwrap_or_default(),
        });
    }
    date(trimmed).ok_or_else(|| PruneRefusal::AgeUnreadable {
        age: age.to_string(),
    })
}

/// `text` as a span of time, in nanoseconds: a whole number of days (`d`)
/// or weeks (`w`), or Go's `time.ParseDuration` grammar (an optional sign,
/// then numbers with units, `1h30m`, `1.5h`, `720h`).
fn offset(text: &str) -> Option<i128> {
    for (suffix, days) in [('d', 1), ('w', 7)] {
        if let Some(count) = text.strip_suffix(suffix) {
            if let Ok(count) = count.parse::<i64>() {
                return Some(
                    i128::from(count) * i128::from(days * SECONDS_PER_DAY) * NANOS_PER_SECOND,
                );
            }
        }
    }
    duration(text)
}

/// Go's `time.ParseDuration`, in nanoseconds.
fn duration(text: &str) -> Option<i128> {
    let (sign, mut rest) = match text.as_bytes().first()? {
        b'-' => (-1, &text[1..]),
        b'+' => (1, &text[1..]),
        _ => (1, text),
    };
    if rest == "0" {
        return Some(0);
    }
    if rest.is_empty() {
        return None;
    }
    let mut total: i128 = 0;
    while !rest.is_empty() {
        let whole_len = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let (whole, after) = rest.split_at(whole_len);
        let (fraction, after) = match after.strip_prefix('.') {
            Some(after) => after.split_at(
                after
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(after.len()),
            ),
            None => ("", after),
        };
        if whole.is_empty() && fraction.is_empty() {
            return None;
        }
        let unit_len = after
            .find(|c: char| c.is_ascii_digit() || c == '.')
            .unwrap_or(after.len());
        let (unit, after) = after.split_at(unit_len);
        let scale: i128 = match unit {
            "ns" => 1,
            "us" | "\u{b5}s" | "\u{3bc}s" => 1_000,
            "ms" => 1_000_000,
            "s" => 1_000_000_000,
            "m" => 60_000_000_000,
            "h" => 3_600_000_000_000,
            _ => return None,
        };
        // Digits past the nanosecond are dropped, as Go drops them.
        let whole: i128 = if whole.is_empty() {
            0
        } else {
            whole.parse().ok()?
        };
        let mut part = 0;
        let mut place = scale;
        for digit in fraction.bytes() {
            place /= 10;
            part += i128::from(digit - b'0') * place;
        }
        total = total.checked_add(whole.checked_mul(scale)?.checked_add(part)?)?;
        // Go refuses a duration its int64 of nanoseconds cannot hold.
        if total > i128::from(i64::MAX) {
            return None;
        }
        rest = after;
    }
    Some(sign * total)
}

/// `text` as a date, `YYYY-MM-DD`, at midnight UTC: Go's `time.DateOnly`.
fn date(text: &str) -> Option<Timestamp> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = text.get(range)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let (year, month, day) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&day) {
        return None;
    }
    Some(Timestamp {
        seconds: days_from_civil(year, month, day) * SECONDS_PER_DAY,
        nanos: 0,
    })
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
#[path = "prune.test.rs"]
mod tests;
