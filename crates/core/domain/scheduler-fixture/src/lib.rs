//! A test-only domain that consumes the scheduler. `dummy` records a deadline
//! ([`SetDeadline`] -> [`DeadlineSet`]); [`ArmsDeadline`], its policy, asks the
//! scheduler for an alarm at it and withdraws it on [`Closed`]; the
//! scheduler's delivery policy ([`DeliverToDummy`], one `delivery!` line)
//! hands it `AlarmReached`, which it re-checks against its own deadline and
//! records as [`DeadlinePassed`] only if it still matches.

use std::convert::Infallible;

use angzarr_embedded::markup::{Commands, PageContext, Record};
use angzarr_embedded::Timestamp;
use angzarr_embedded_macros::{aggregate, applies, handles_cmd, handles_fact, on_event, policy};
use scheduler::{stream_for, AlarmReached, ArmAlarm, CancelAlarm, SchedulerAggregate, Stream};

/// The generated wire types, as the domain layer names them. The anchor
/// message (`DummyState`) is a declaration only, never constructed.
#[allow(dead_code)]
mod wire {
    include!(concat!(
        env!("OUT_DIR"),
        "/angzarr.embedded.scheduler.fixture.v1.rs"
    ));
}
mod generated {
    include!(concat!(
        env!("OUT_DIR"),
        "/angzarr.embedded.scheduler.fixture.v1.domain.rs"
    ));
}
pub use generated::*;

/// The domain's name.
pub const DUMMY_DOMAIN: &str = "dummy";
/// The one alarm id the dummy arms.
pub const DEADLINE_ALARM: &str = "deadline";

/// The dummy, folded from its stream.
#[derive(Clone, Debug, Default)]
pub struct DummyAggregate {
    pub deadline: Option<Timestamp>,
    pub closed: bool,
    pub passed: bool,
}

#[aggregate(domain = "dummy", command = DummyCommand, event = DummyEvent)]
impl DummyAggregate {
    #[handles_cmd(SetDeadline)]
    fn set_deadline(&self, cmd: SetDeadline, ctx: &Context) -> Result<DeadlineSet, Infallible> {
        let _ = ctx;
        Ok(DeadlineSet {
            deadline: cmd.deadline,
        })
    }

    #[handles_cmd(Close)]
    fn close(&self, cmd: Close, ctx: &Context) -> Result<Closed, Infallible> {
        let _ = (cmd, ctx);
        Ok(Closed {})
    }

    /// The scheduler says the deadline is reached: re-checked against this
    /// dummy's own arming, recorded only if it is still the deadline and the
    /// dummy is still open and has not passed yet.
    #[handles_fact(AlarmReached)]
    fn reached(&self, fact: AlarmReached, ctx: &Context) -> Record<Self> {
        let _ = ctx;
        let live = !self.closed && !self.passed;
        let matches = fact.alarm_id == DEADLINE_ALARM && self.deadline == Some(fact.at);
        Record::of((live && matches).then_some(DeadlinePassed { at: fact.at }))
    }

    #[applies(DeadlineSet)]
    fn deadline_set(&mut self, event: DeadlineSet, page: &PageContext) {
        let _ = page;
        self.deadline = Some(event.deadline);
    }

    #[applies(Closed)]
    fn closed_(&mut self, event: Closed, page: &PageContext) {
        let _ = (event, page);
        self.closed = true;
    }

    #[applies(DeadlinePassed)]
    fn deadline_passed(&mut self, event: DeadlinePassed, page: &PageContext) {
        let _ = (event, page);
        self.passed = true;
    }
}

/// The dummy's policy towards the scheduler: a deadline set arms the alarm,
/// a close cancels it.
pub struct ArmsDeadline;

/// The scheduler stream's root and the target the dummy's events are for.
fn scheduler_of(ctx: &PolicyContext) -> (angzarr_embedded::v1::Uuid, Stream) {
    let trigger = ctx.trigger().stream.clone().unwrap_or_default();
    let target = Stream {
        domain: trigger.domain,
        root: ctx.source_root().value.to_vec(),
        edition: trigger.edition,
    };
    (stream_for(&target), target)
}

#[policy(name = "dummy-arms-deadline", source = DummyAggregate, target = SchedulerAggregate)]
impl ArmsDeadline {
    #[on_event(DeadlineSet)]
    fn set(
        &self,
        event: &DeadlineSet,
        ctx: &PolicyContext,
    ) -> Result<Commands<SchedulerAggregate>, Infallible> {
        let (root, target) = scheduler_of(ctx);
        Ok(Commands::none().to(
            root,
            ArmAlarm {
                target,
                alarm_id: DEADLINE_ALARM.to_string(),
                at: event.deadline,
            },
        ))
    }

    #[on_event(Closed)]
    fn closed(
        &self,
        event: &Closed,
        ctx: &PolicyContext,
    ) -> Result<Commands<SchedulerAggregate>, Infallible> {
        let _ = event;
        let (root, _) = scheduler_of(ctx);
        Ok(Commands::none().to(
            root,
            CancelAlarm {
                alarm_id: DEADLINE_ALARM.to_string(),
            },
        ))
    }
}

scheduler::delivery!(DeliverToDummy, DummyAggregate);

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
