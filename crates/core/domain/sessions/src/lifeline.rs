//! The session's run lifeline (plan 3.1): the incarnation it owes, whether one
//! is open, the relaunch budget (the SESSION's, per harp, not per run) and the
//! stop flag.

use angzarr_embedded::markup::{Context, Record};

use crate::{
    live, InboxStatus, IncarnationEnded, IncarnationOwed, RecordIncarnationEnded, SessionAggregate,
    SessionEvent, SessionRejection, SessionState, StopOrdered, StopSession,
};

/// How many automatic relaunches a session spends before it gives up, reset
/// only when mail is consumed (a delivery), never by an attach. Go's
/// `defaultMaxLaunchAttempts = 4` (internal/core/coord/launchgate.go):
/// `nextRelaunch` authorises while `spent < max`, so FOUR relaunches, then it
/// refuses (owner fact check, S5 review item 11).
pub const RELAUNCH_BUDGET: u32 = 4;

/// The incarnation a mint owes: the first (owner ruling 2026-11-04, S5
/// review Q1 (b)).
pub(crate) fn first_owed() -> IncarnationOwed {
    IncarnationOwed {
        incarnation: 1,
        reason: "first".to_string(),
    }
}

/// What a stop order records: refused from a child (Go's I2); a second order
/// records nothing. A stop ordered after the end still counts: it keeps the
/// relaunch from being owed.
pub(crate) fn stop_ordered(
    held: &SessionState,
    stop: &StopSession,
) -> Result<Option<StopOrdered>, SessionRejection> {
    let held = live(held)?;
    if stop.requester_depth > 0 {
        return Err(SessionRejection::ChildCannotStop {
            depth: u64::from(stop.requester_depth),
        });
    }
    Ok((!held.stopped).then(|| StopOrdered {
        reason: stop.reason.clone(),
    }))
}

/// Go's idle reaper's terminal (`CauseIdleReaped`): an expected end that
/// leaves the session resumable and notices nobody.
const CAUSE_IDLE_REAPED: &str = "idle-reaped";

/// Go's agent stop (`CauseStopped`): never relaunched.
const CAUSE_STOPPED: &str = "stopped";

/// Whether the inbox holds mail still to be consumed: queued or claimed.
pub(crate) fn mail_pending(held: &SessionState) -> bool {
    held.inbox
        .iter()
        .any(|entry| matches!(entry.status, InboxStatus::Queued | InboxStatus::Claimed))
}

/// What a run's end records: the end, then, BY LEVEL (Go's gate), a relaunch
/// owed when mail is pending, the budget allows, the session is not stopped
/// and the cause is not `stopped`; otherwise the session gives up and the
/// parent is to be noticed (except for an idle reap, which is no death). An
/// end for an incarnation that is not the open one (already ended, or stale)
/// records nothing.
pub(crate) fn ended(held: &SessionState, end: &RecordIncarnationEnded) -> Record<SessionAggregate> {
    if live(held).is_err() || held.open_incarnation != Some(end.incarnation) {
        return Record::of(None::<IncarnationEnded>);
    }
    let relaunch = mail_pending(held)
        && held.relaunches < RELAUNCH_BUDGET
        && !held.stopped
        && end.cause != CAUSE_STOPPED;
    let ended = IncarnationEnded {
        incarnation: end.incarnation,
        cause: end.cause.clone(),
        notice_parent: !relaunch && end.cause != CAUSE_IDLE_REAPED,
    };
    Record::of(ended).then(move |after: &SessionAggregate, _: &Context| {
        relaunch.then(|| IncarnationOwed {
            incarnation: after.state().incarnation + 1,
            reason: RELAUNCH.to_string(),
        })
    })
}

/// The reason of an owed incarnation that spends the relaunch budget.
const RELAUNCH: &str = "relaunch";

/// The reason of an owed incarnation that mail asked for.
pub(crate) const MAIL: &str = "mail";

/// Fold a lifeline event: the incarnation owed (a relaunch spends the
/// budget), its end, and the stop flag.
pub(crate) fn apply(state: &mut SessionState, event: &SessionEvent) {
    match event {
        SessionEvent::IncarnationOwed(owed) => {
            state.incarnation = owed.incarnation;
            state.open_incarnation = Some(owed.incarnation);
            if owed.reason == RELAUNCH {
                state.relaunches += 1;
            }
        }
        SessionEvent::IncarnationEnded(_) => state.open_incarnation = None,
        SessionEvent::StopOrdered(_) => state.stopped = true,
        _ => {}
    }
}

#[cfg(test)]
#[path = "lifeline.test.rs"]
mod tests;
