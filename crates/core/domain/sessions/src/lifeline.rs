//! The session's run lifeline (plan 3.1): the incarnation it owes, whether one
//! is open, the relaunch budget (the SESSION's, per harp, not per run) and the
//! stop flag. STUBS (S5 red): every rule is `todo!`; `lifeline.test.rs` holds
//! the behaviour.

use angzarr_embedded::markup::Record;

use crate::{
    IncarnationOwed, RecordIncarnationEnded, SessionAggregate, SessionEvent, SessionRejection,
    SessionState, StopOrdered, StopSession,
};

/// How many relaunches a session spends before it gives up, reset only when
/// mail is consumed (a delivery), never by an attach. Go's `maxLaunchAttempts`
/// (relaunch_budget_test.go); the value is an open question in the review.
pub const RELAUNCH_BUDGET: u32 = 3;

/// The incarnation a mint owes: the first (owner ruling 2026-11-04, S5
/// review Q1 (b)).
pub(crate) fn first_owed() -> IncarnationOwed {
    IncarnationOwed {
        incarnation: 1,
        reason: "first".to_string(),
    }
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

/// Fold a lifeline event. Only the owed incarnation is folded (the mint's
/// ruling needs it so S4's sessions still replay); the rest are stubs.
pub(crate) fn apply(state: &mut SessionState, event: &SessionEvent) {
    match event {
        SessionEvent::IncarnationOwed(owed) => {
            state.incarnation = owed.incarnation;
            state.open_incarnation = Some(owed.incarnation);
        }
        _ => todo!("S5 stub"),
    }
}

#[cfg(test)]
#[path = "lifeline.test.rs"]
mod tests;
