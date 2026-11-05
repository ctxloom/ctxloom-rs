//! The scheduler stream, folded from its events: the target it belongs to and
//! its alarms.

use std::collections::{BTreeMap, BTreeSet};

use angzarr_embedded::Timestamp;

use crate::{SchedulerEvent, Stream};

/// The scheduler domain's name: every scheduler stream's `Cover.domain`.
pub const SCHEDULER_DOMAIN: &str = "scheduler";

/// An instant as an ordered key: prost's `Timestamp` has no ordering.
pub fn instant(at: &Timestamp) -> (i64, i32) {
    (at.seconds, at.nanos)
}

/// One alarm of the stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlarmState {
    /// The instant it is armed at now; `None` once cancelled or fired.
    pub armed: Option<Timestamp>,
    /// Every instant it has been reached at. The clock's fact for an
    /// (alarm, instant) is recorded once, so arming an instant already reached
    /// records nothing (scheduler.plan.md OD-4).
    pub reached: BTreeSet<(i64, i32)>,
}

/// The scheduler stream, folded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulerState {
    /// The stream every alarm here is for, fixed by the first arming.
    pub target: Option<Stream>,
    pub alarms: BTreeMap<String, AlarmState>,
}

/// Fold `event` into `state`: what the aggregate's appliers do, one place.
/// Pure.
pub fn apply(state: &mut SchedulerState, event: &SchedulerEvent) {
    match event {
        SchedulerEvent::AlarmArmed(armed) => {
            state.target.get_or_insert_with(|| armed.target.clone());
            state
                .alarms
                .entry(armed.alarm_id.clone())
                .or_default()
                .armed = Some(armed.at);
        }
        SchedulerEvent::AlarmCancelled(cancelled) => {
            if let Some(alarm) = state.alarms.get_mut(&cancelled.alarm_id) {
                alarm.armed = None;
            }
        }
        SchedulerEvent::AlarmFired(fired) => {
            let alarm = state.alarms.entry(fired.alarm_id.clone()).or_default();
            alarm.armed = None;
            alarm.reached.insert(instant(&fired.at));
        }
    }
}
