//! The session's inbox (the former mailbox, D5 Q4): queued, claimed,
//! delivered, withdrawn, failed. STUBS (S5 red).

use angzarr_embedded::markup::Record;

use crate::{
    ClaimMessage, DeliverMessage, FailMessage, RecordMessageArrived, SessionAggregate,
    SessionEvent, SessionRejection, SessionState, SteerWithdrawn, WithdrawSteer,
};

pub(crate) fn arrived(
    held: &SessionState,
    arrive: &RecordMessageArrived,
) -> Record<SessionAggregate> {
    let _ = (held, arrive);
    todo!("S5 stub")
}

pub(crate) fn claimed(held: &SessionState, claim: &ClaimMessage) -> Record<SessionAggregate> {
    let _ = (held, claim);
    todo!("S5 stub")
}

pub(crate) fn delivered(held: &SessionState, deliver: &DeliverMessage) -> Record<SessionAggregate> {
    let _ = (held, deliver);
    todo!("S5 stub")
}

pub(crate) fn failed(held: &SessionState, fail: &FailMessage) -> Record<SessionAggregate> {
    let _ = (held, fail);
    todo!("S5 stub")
}

pub(crate) fn withdrawn(
    held: &SessionState,
    withdraw: &WithdrawSteer,
) -> Result<SteerWithdrawn, SessionRejection> {
    let _ = (held, withdraw);
    todo!("S5 stub")
}

/// Fold an inbox event.
pub(crate) fn apply(state: &mut SessionState, event: &SessionEvent) {
    let _ = (state, event);
    todo!("S5 stub")
}

#[cfg(test)]
#[path = "inbox.test.rs"]
mod tests;
