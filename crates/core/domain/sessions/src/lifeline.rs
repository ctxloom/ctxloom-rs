//! The session's run lifeline (plan 3.1): the incarnation it owes, whether one
//! is open, the relaunch budget (the SESSION's, per harp, not per run) and the
//! stop flag. STUBS (S5 red): every rule is `todo!`; `lifeline.test.rs` holds
//! the behaviour.

use angzarr_embedded::markup::Record;

use crate::{
    IncarnationOwed, OweIncarnation, RecordIncarnationEnded, SessionAggregate, SessionEvent,
    SessionRejection, SessionState, StopOrdered, StopSession,
};

/// How many relaunches a session spends before it gives up, reset only when
/// mail is consumed (a delivery), never by an attach. Go's `maxLaunchAttempts`
/// (relaunch_budget_test.go); the value is an open question in the review.
pub const RELAUNCH_BUDGET: u32 = 3;

/// What an owed incarnation records.
pub(crate) fn owed(
    held: &SessionState,
    owe: &OweIncarnation,
) -> Result<IncarnationOwed, SessionRejection> {
    let _ = (held, owe);
    todo!("S5 stub")
}

/// What a stop order records: refused from a child (Go's I2).
pub(crate) fn stop_ordered(
    held: &SessionState,
    stop: &StopSession,
) -> Result<Option<StopOrdered>, SessionRejection> {
    let _ = (held, stop);
    todo!("S5 stub")
}

/// What a run's end records: the end, then, BY LEVEL, a relaunch owed or the
/// parent noticed.
pub(crate) fn ended(held: &SessionState, end: &RecordIncarnationEnded) -> Record<SessionAggregate> {
    let _ = (held, end);
    todo!("S5 stub")
}

/// Fold a lifeline event.
pub(crate) fn apply(state: &mut SessionState, event: &SessionEvent) {
    let _ = (state, event);
    todo!("S5 stub")
}

#[cfg(test)]
#[path = "lifeline.test.rs"]
mod tests;
