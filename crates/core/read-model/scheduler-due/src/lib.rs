//! The due model: a read model a projector folds from the scheduler streams.
//! For each stream it holds the target, and every alarm still armed with its
//! instant and the correlation of the event that armed it. It answers what is
//! due at an instant, and when to wake next.
//!
//! It is HELD in memory ([`catch_up`] is a `CatchUp::new`), rebuilt from each
//! stream's start by a process's first step. It must be held, not stored: a
//! stored position with a lost in-memory fold would skip alarms.
//!
//! No clock: `now` is an argument.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use angzarr_embedded::angzarr_router::error::CodedError;
use angzarr_embedded::v1::{EventBook, Projection, Uuid};
use angzarr_embedded::{CatchUp, Payload, PayloadError, ProjectorHandler, Timestamp};
use scheduler::{instant, stream_text, SchedulerEvent, Stream, SCHEDULER_DOMAIN};

/// An armed alarm.
#[derive(Clone, Debug, PartialEq)]
pub struct DueAlarm {
    /// The stream to tell.
    pub target: Stream,
    pub alarm_id: String,
    /// The instant armed.
    pub at: Timestamp,
    /// The correlation of the event that armed it, so the workflow's
    /// correlation runs through the timer (scheduler.plan.md OD-17).
    pub correlation: Option<Uuid>,
}

/// What one scheduler stream holds armed.
#[derive(Clone, Debug, Default)]
struct Armed {
    alarms: BTreeMap<String, (Timestamp, Option<Uuid>)>,
}

/// Every scheduler stream's armed alarms.
#[derive(Default)]
pub struct Due {
    /// By the scheduler stream's root.
    streams: BTreeMap<Vec<u8>, (Option<Stream>, Armed)>,
}

impl Due {
    /// An empty model.
    pub fn new() -> Due {
        Due::default()
    }

    /// Fold `book`, one scheduler stream's events after the last projected of
    /// it, as the projector port hands it: a book from sequence 0 starts the
    /// stream afresh; a book with no event and `next_sequence` 0 says the
    /// stream is gone (purged), and its alarms leave the model.
    pub fn project(&mut self, book: &EventBook) -> Result<(), PayloadError> {
        let root = book
            .cover
            .as_ref()
            .and_then(|cover| cover.root.as_ref())
            .map(|root| root.value.to_vec())
            .unwrap_or_default();
        let Some(first) = book.pages.first() else {
            if book.next_sequence == 0 {
                self.streams.remove(&root);
            }
            return Ok(());
        };
        let mut held = match first.sequence {
            0 => (None, Armed::default()),
            _ => self.streams.remove(&root).unwrap_or_default(),
        };
        for page in &book.pages {
            let event = page.payload.clone().unwrap_or_default();
            let correlation = page.meta.as_ref().and_then(|m| m.correlation_id.clone());
            fold(&mut held, SchedulerEvent::from_any(&event)?, correlation);
        }
        self.streams.insert(root, held);
        Ok(())
    }

    /// Every alarm armed at or before `now`, earliest instant first (a tie by
    /// target, then alarm id).
    pub fn due_at_or_before(&self, now: &Timestamp) -> Vec<DueAlarm> {
        let mut due: Vec<DueAlarm> = self
            .alarms()
            .filter(|alarm| instant(&alarm.at) <= instant(now))
            .collect();
        due.sort_by_key(sort_key);
        due
    }

    /// The earliest instant armed after `now`: when to wake next.
    pub fn next_after(&self, now: &Timestamp) -> Option<Timestamp> {
        self.alarms()
            .map(|alarm| alarm.at)
            .filter(|at| instant(at) > instant(now))
            .min_by_key(instant)
    }

    fn alarms(&self) -> impl Iterator<Item = DueAlarm> + '_ {
        self.streams.values().flat_map(|(target, armed)| {
            armed
                .alarms
                .iter()
                .filter_map(|(alarm_id, (at, correlation))| {
                    target.clone().map(|target| DueAlarm {
                        target,
                        alarm_id: alarm_id.clone(),
                        at: *at,
                        correlation: correlation.clone(),
                    })
                })
        })
    }
}

fn sort_key(alarm: &DueAlarm) -> ((i64, i32), String, String) {
    (
        instant(&alarm.at),
        stream_text(&alarm.target),
        alarm.alarm_id.clone(),
    )
}

/// Fold one event of a scheduler stream.
fn fold(held: &mut (Option<Stream>, Armed), event: SchedulerEvent, correlation: Option<Uuid>) {
    match event {
        SchedulerEvent::AlarmArmed(armed) => {
            held.0.get_or_insert(armed.target);
            held.1
                .alarms
                .insert(armed.alarm_id, (armed.at, correlation));
        }
        SchedulerEvent::AlarmCancelled(cancelled) => {
            held.1.alarms.remove(&cancelled.alarm_id);
        }
        SchedulerEvent::AlarmFired(fired) => {
            held.1.alarms.remove(&fired.alarm_id);
        }
    }
}

/// The due model on angzarr-embedded's projector port.
#[derive(Default)]
pub struct DueModel(Mutex<Due>);

impl DueModel {
    /// The model, locked.
    pub fn due(&self) -> MutexGuard<'_, Due> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl ProjectorHandler for DueModel {
    fn name(&self) -> &str {
        "scheduler-due"
    }

    async fn dispatch(&self, events: &EventBook) -> Result<Projection, CodedError> {
        self.due().project(events).map_err(|error| bug(&error))?;
        Ok(Projection {
            cover: events.cover.clone(),
            projector: self.name().to_string(),
            next_sequence: events.next_sequence,
            projection: None,
        })
    }
}

/// An empty due model on the projector port, held in memory, to catch up
/// from a store.
pub fn catch_up() -> CatchUp<DueModel> {
    CatchUp::new(&[SCHEDULER_DOMAIN], DueModel::default())
        .expect("a due model reads the scheduler domain")
}

/// `error`, an event the model cannot read, as the projector port's bug: its
/// code, and its text args (the type URL it names).
fn bug(error: &PayloadError) -> CodedError {
    use angzarr_embedded::angzarr_diagnostic::{Arg, Diagnostic as _};
    let args = error
        .args()
        .into_iter()
        .filter_map(|(name, arg)| match arg {
            Arg::Str(value) => Some((name.to_string(), value)),
            _ => None,
        });
    CodedError::bug(error.code(), args)
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
