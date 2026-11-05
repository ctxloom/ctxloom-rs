//! The scheduler aggregate on angzarr-embedded's markup: one stream per
//! target stream, holding that target's alarms.
//!
//! Arming and cancelling are commands, refusable (OD-3); the clock's
//! `AlarmDue` is a fact, never refused, and records nothing it cannot use.

use angzarr_embedded::markup::{Context, Failed, PageContext, Record};
use angzarr_embedded_macros::{aggregate, applies, handles_cmd, handles_fact};

use crate::{
    apply, instant, stream_for, stream_text, AlarmArmed, AlarmCancelled, AlarmDue, AlarmFired,
    ArmAlarm, CancelAlarm, SchedulerCommand, SchedulerEvent, SchedulerRejection, SchedulerState,
};

/// The scheduler aggregate: one stream's alarms, folded.
#[derive(Clone, Debug, Default)]
pub struct SchedulerAggregate {
    scheduler: SchedulerState,
}

impl SchedulerAggregate {
    /// The folded stream.
    pub fn state(&self) -> &SchedulerState {
        &self.scheduler
    }
}

#[aggregate(domain = "scheduler", command = SchedulerCommand, event = SchedulerEvent)]
impl SchedulerAggregate {
    /// Arm an alarm for the stream's target. Re-arming moves it. Nothing is
    /// recorded when the alarm is armed there already, or has been reached
    /// at that instant (the same alarm at the same instant is the same fact).
    #[handles_cmd(ArmAlarm)]
    fn arm(
        &self,
        cmd: ArmAlarm,
        ctx: &Context,
    ) -> Result<Option<AlarmArmed>, Failed<SchedulerRejection>> {
        if cmd.alarm_id.is_empty() {
            return Err(SchedulerRejection::AlarmIdEmpty.into());
        }
        let here = ctx.command.cover.as_ref();
        let own = Some(&stream_for(&cmd.target)) == here.and_then(|cover| cover.root.as_ref())
            && cmd.target.edition.as_deref().filter(|e| !e.is_empty())
                == here
                    .and_then(|cover| cover.edition.as_deref())
                    .filter(|e| !e.is_empty());
        if !own {
            return Err(SchedulerRejection::TargetMismatch {
                stream: here.map(crate::cover_text).unwrap_or_default(),
                target: stream_text(&cmd.target),
            }
            .into());
        }
        let alarm = self.scheduler.alarms.get(&cmd.alarm_id);
        let settled = alarm.is_some_and(|alarm| {
            alarm.armed.as_ref().map(instant) == Some(instant(&cmd.at))
                || alarm.reached.contains(&instant(&cmd.at))
        });
        Ok((!settled).then_some(AlarmArmed {
            target: cmd.target,
            alarm_id: cmd.alarm_id,
            at: cmd.at,
        }))
    }

    /// Withdraw an armed alarm; anything else records nothing.
    #[handles_cmd(CancelAlarm)]
    fn cancel(
        &self,
        cmd: CancelAlarm,
        ctx: &Context,
    ) -> Result<Option<AlarmCancelled>, Failed<SchedulerRejection>> {
        let _ = ctx;
        let armed = self
            .scheduler
            .alarms
            .get(&cmd.alarm_id)
            .is_some_and(|alarm| alarm.armed.is_some());
        Ok(armed.then_some(AlarmCancelled {
            alarm_id: cmd.alarm_id,
        }))
    }

    /// The clock says an instant is reached. Fires the alarm only if it is
    /// armed at exactly this instant; a moved, cancelled or fired alarm
    /// records nothing.
    #[handles_fact(AlarmDue)]
    fn due(&self, fact: AlarmDue, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        let alarm = self.scheduler.alarms.get(&fact.alarm_id);
        let armed =
            alarm.is_some_and(|alarm| alarm.armed.as_ref().map(instant) == Some(instant(&fact.at)));
        let fired = self.scheduler.target.clone().filter(|_| armed);
        Record::of(fired.map(|target| AlarmFired {
            target,
            alarm_id: fact.alarm_id,
            at: fact.at,
        }))
    }

    #[applies(AlarmArmed)]
    fn alarm_armed(&mut self, event: AlarmArmed, page: &PageContext) {
        let _ = page;
        apply(&mut self.scheduler, &event.into());
    }

    #[applies(AlarmCancelled)]
    fn alarm_cancelled(&mut self, event: AlarmCancelled, page: &PageContext) {
        let _ = page;
        apply(&mut self.scheduler, &event.into());
    }

    #[applies(AlarmFired)]
    fn alarm_fired(&mut self, event: AlarmFired, page: &PageContext) {
        let _ = page;
        apply(&mut self.scheduler, &event.into());
    }
}

#[cfg(test)]
#[path = "aggregate.test.rs"]
mod tests;
