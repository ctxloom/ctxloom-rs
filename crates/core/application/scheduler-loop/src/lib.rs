//! The scheduler's clock loop.
//!
//! [`SchedulerLoop::tick`] is the unit: one step with no sleeping. It catches
//! a HELD due model up over the scheduler streams, and for every alarm armed
//! at or before `now`, in instant order, sends the scheduler the private
//! fact `AlarmDue` under the deterministic id `alarm:<stream>/<alarm_id>@<at>`
//! and the arming event's correlation. The scheduler re-checks it and records
//! `AlarmFired`; the target's delivery policy then delivers `AlarmReached`,
//! all inside that one kernel call. [`SchedulerLoop::run`] repeats `tick`
//! over a [`Clock`] and a [`Wait`]; tests drive `tick`.
//!
//! Exactly once: a re-sent `AlarmDue` is a DUPLICATE by its id; the
//! scheduler's own re-check covers a moved or cancelled alarm; a fresh loop
//! over the same store rebuilds its model from the streams, in which a fired
//! alarm is gone and an alarm that came due while no loop ran is simply due
//! (it fires once, late, with the instant it was armed at).

mod error;

use std::time::Duration;

use angzarr_embedded::markup::pack_message;
use angzarr_embedded::v1::{command_page, CommandBook, CommandPage, Cover, Fact, FactOrigin};
use angzarr_embedded::{
    ids, CatchUp, Failure, Handled, Kernel, Payload, Processed, ReadStore, Rejected, Sampled,
    Store, Timestamp, Watch,
};
use scheduler::{alarm_external_id, stream_for, AlarmDue, SchedulerEvent, SCHEDULER_DOMAIN};
use scheduler_due::{catch_up, DueAlarm, DueModel};
use scheduler_ports::{Clock, Wait};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{LoopError, EN_US};

/// What the scheduler did with one alarm that was due.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sent {
    /// It recorded `AlarmFired` (and delivery ran).
    Fired,
    /// The fact's id already has events on the stream: sent before.
    Duplicate,
    /// Applied and recorded nothing, or parked by the kernel.
    Nothing,
}

/// What one tick did.
#[derive(Debug, Default)]
pub struct Tick {
    /// Each alarm that was due, earliest instant first, with what became of it.
    pub sent: Vec<(DueAlarm, Sent)>,
    /// Each alarm whose fact the kernel refused or failed, with the failure's
    /// code. The alarm stays due, so the next tick sends it again.
    pub failed: Vec<(DueAlarm, String)>,
    /// The streams the due model could not read; held where they were.
    pub unread: Vec<Cover>,
    /// The earliest armed instant after `now`: when to wake next.
    pub next: Option<Timestamp>,
}

impl Tick {
    /// The alarms that fired this tick.
    pub fn fired(&self) -> Vec<&DueAlarm> {
        self.sent
            .iter()
            .filter(|(_, sent)| *sent == Sent::Fired)
            .map(|(alarm, _)| alarm)
            .collect()
    }
}

/// The loop: a due model, held in memory, over a store.
pub struct SchedulerLoop {
    due: CatchUp<DueModel>,
}

impl Default for SchedulerLoop {
    fn default() -> Self {
        SchedulerLoop::new()
    }
}

impl SchedulerLoop {
    /// A loop with an empty model: its first tick rebuilds it from each
    /// stream's start.
    pub fn new() -> SchedulerLoop {
        SchedulerLoop { due: catch_up() }
    }

    /// One step: catch the model up, then fire every alarm armed at or before
    /// `now`. No sleeping and no clock: `now` is the caller's.
    pub async fn tick<S>(
        &mut self,
        kernel: &Kernel<'_>,
        store: &S,
        now: Timestamp,
    ) -> Result<Tick, LoopError>
    where
        S: Store + ReadStore + Processed + Watch,
    {
        let mut tick = Tick::default();
        let caught = self
            .due
            .step_if_changed(store, kernel.registry().catalog())
            .await
            .map_err(|error| LoopError::CatchUpFailed {
                reason: error.to_string(),
            })?;
        tick.unread = caught
            .map(|caught| caught.failures().iter().map(|(c, _)| c.clone()).collect())
            .unwrap_or_default();
        let due = self.due.projector().due().due_at_or_before(&now);
        for alarm in due {
            match fire(kernel, store, &alarm, now).await {
                Ok(sent) => tick.sent.push((alarm, sent)),
                Err(code) => tick.failed.push((alarm, code)),
            }
        }
        tick.next = self.due.projector().due().next_after(&now);
        Ok(tick)
    }

    /// Tick on `clock`, then wait `pause(now, next, every)` on `wait`, again
    /// and again while `more`, asked after each tick with what it did, says
    /// so. Returns when it does not, or at an error.
    pub async fn run<S>(
        &mut self,
        kernel: &Kernel<'_>,
        store: &S,
        clock: &dyn Clock,
        wait: &dyn Wait,
        every: Duration,
        mut more: impl FnMut(&Tick) -> bool,
    ) -> Result<(), LoopError>
    where
        S: Store + ReadStore + Processed + Watch,
    {
        loop {
            let now = clock.now();
            let tick = self.tick(kernel, store, now).await?;
            if !more(&tick) {
                return Ok(());
            }
            wait.wait(pause(&now, tick.next.as_ref(), every)).await;
        }
    }
}

/// How long to wait after a tick at `now`: until the next armed instant, but
/// never longer than `every`, the poll that bounds how late a newly armed,
/// earlier alarm is noticed.
pub fn pause(now: &Timestamp, next: Option<&Timestamp>, every: Duration) -> Duration {
    let Some(next) = next else {
        return every;
    };
    let seconds = u64::try_from(next.seconds.saturating_sub(now.seconds)).unwrap_or(0);
    let nanos = i64::from(next.nanos) - i64::from(now.nanos);
    let until = Duration::from_secs(seconds)
        .saturating_add(Duration::from_nanos(nanos.max(0).unsigned_abs()))
        .saturating_sub(Duration::from_nanos(nanos.min(0).unsigned_abs()));
    until.min(every)
}

/// The clock's fact for `alarm`, sent to the kernel; what it came to, or the
/// failure's code.
async fn fire<S: Store>(
    kernel: &Kernel<'_>,
    store: &S,
    alarm: &DueAlarm,
    now: Timestamp,
) -> Result<Sent, String> {
    let external_id = alarm_external_id(&alarm.target, &alarm.alarm_id, &alarm.at);
    let cover = Cover {
        domain: SCHEDULER_DOMAIN.to_string(),
        root: Some(stream_for(&alarm.target)),
        edition: alarm.target.edition.clone(),
    };
    let due = AlarmDue {
        alarm_id: alarm.alarm_id.clone(),
        at: alarm.at,
    };
    let book = CommandBook {
        cover: Some(cover.clone()),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(Fact {
                payload: Some(pack_message(&due)),
            })),
            fact_origin: Some(FactOrigin {
                external_id: external_id.clone(),
                description: String::new(),
            }),
            correlation_id: alarm.correlation.clone(),
            ..CommandPage::default()
        }),
        handling: None,
    };
    // A fact's command id is the kernel's derivation from its external id; the
    // sample's is unused for it.
    let sampled = Sampled {
        now,
        command_id: ids::fact_command_id(&external_id),
    };
    match kernel.handle_any(store, book, &sampled).await {
        Ok(Handled::Applied(recorded)) => {
            let root = cover.root.as_ref();
            let fired = recorded
                .books()
                .iter()
                .filter(|book| {
                    book.cover.as_ref().map(|c| (&c.domain, c.root.as_ref()))
                        == Some((&cover.domain, root))
                })
                .flat_map(|book| &book.pages)
                .filter_map(|page| page.payload.as_ref())
                .any(|payload| {
                    matches!(
                        SchedulerEvent::from_any(payload),
                        Ok(SchedulerEvent::AlarmFired(_))
                    )
                });
            Ok(if fired { Sent::Fired } else { Sent::Nothing })
        }
        Ok(Handled::Duplicate(..)) => Ok(Sent::Duplicate),
        Ok(Handled::Parked(_)) => Ok(Sent::Nothing),
        Err(failure) => Err(failure_code(&failure)),
    }
}

fn failure_code(failure: &Failure<Rejected>) -> String {
    use angzarr_diagnostic::Diagnostic as _;
    failure.code().to_string()
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
