//! The session's inbox (the former mailbox, D5 Q4): queued, claimed,
//! delivered, withdrawn, failed.

use angzarr_embedded::markup::{Context, Record};

use crate::{
    lifeline, live, ClaimMessage, DeliverMessage, FailMessage, InboxEntry, InboxStatus,
    IncarnationOwed, MessageArrived, MessageClaimed, MessageDelivered, MessageFailed,
    RecordMessageArrived, SessionAggregate, SessionEvent, SessionRejection, SessionState,
    SteerWithdrawn, WithdrawSteer,
};

/// What mail arriving records: the arrival, then, by continuation on the state
/// after it (like a relaunch), `IncarnationOwed{n+1, "mail"}` when no
/// incarnation is open and the session is not stopped (owner ruling
/// 2026-11-04, S5 review Q1). Mail under an id the inbox holds queues once;
/// a stream holding no live session records nothing.
pub(crate) fn arrived(
    held: &SessionState,
    arrive: &RecordMessageArrived,
) -> Record<SessionAggregate> {
    if live(held).is_err() || entry(held, &arrive.message_id).is_some() {
        return Record::of(None::<MessageArrived>);
    }
    Record::of(MessageArrived {
        message_id: arrive.message_id.clone(),
        from: arrive.from.clone(),
        kind: arrive.kind.clone(),
        body_ref: arrive.body_ref.clone(),
    })
    .then(|after: &SessionAggregate, _: &Context| {
        let session = after.state();
        let owed = session.open_incarnation.is_none() && !session.stopped;
        owed.then(|| IncarnationOwed {
            incarnation: session.incarnation + 1,
            reason: lifeline::MAIL.to_string(),
        })
    })
}

/// The inbox's entry for `message_id`.
fn entry<'a>(held: &'a SessionState, message_id: &str) -> Option<&'a InboxEntry> {
    held.inbox
        .iter()
        .find(|entry| entry.message_id == message_id)
}

/// Whether `message_id` is in the inbox and not yet settled (queued or
/// claimed): the only mail a claim, a delivery or a failure moves.
fn unsettled(held: &SessionState, message_id: &str) -> bool {
    entry(held, message_id)
        .is_some_and(|entry| matches!(entry.status, InboxStatus::Queued | InboxStatus::Claimed))
}

/// The first claimant wins: a claim on queued mail only.
pub(crate) fn claimed(held: &SessionState, claim: &ClaimMessage) -> Record<SessionAggregate> {
    let queued =
        entry(held, &claim.message_id).is_some_and(|entry| entry.status == InboxStatus::Queued);
    Record::of((queued && live(held).is_ok()).then(|| MessageClaimed {
        message_id: claim.message_id.clone(),
        claimant: claim.claimant.clone(),
    }))
}

/// Delivered once: a second delivery of the same mail records nothing.
pub(crate) fn delivered(held: &SessionState, deliver: &DeliverMessage) -> Record<SessionAggregate> {
    Record::of(
        (unsettled(held, &deliver.message_id) && live(held).is_ok()).then(|| MessageDelivered {
            message_id: deliver.message_id.clone(),
            at_turn: deliver.at_turn,
        }),
    )
}

pub(crate) fn failed(held: &SessionState, fail: &FailMessage) -> Record<SessionAggregate> {
    Record::of(
        (unsettled(held, &fail.message_id) && live(held).is_ok()).then(|| MessageFailed {
            message_id: fail.message_id.clone(),
            reason: fail.reason.clone(),
        }),
    )
}

/// A steer withdrawn while it is still queued or claimed; refused once it was
/// delivered, or when the inbox never held it. Withdrawing what is already
/// withdrawn or failed records nothing.
pub(crate) fn withdrawn(
    held: &SessionState,
    withdraw: &WithdrawSteer,
) -> Result<Option<SteerWithdrawn>, SessionRejection> {
    let held = live(held)?;
    let message_id = withdraw.message_id.clone();
    match entry(held, &message_id).map(|entry| entry.status) {
        None => Err(SessionRejection::UnknownMessage { message_id }),
        Some(InboxStatus::Delivered) => Err(SessionRejection::AlreadyDelivered { message_id }),
        Some(InboxStatus::Queued | InboxStatus::Claimed) => Ok(Some(SteerWithdrawn { message_id })),
        Some(InboxStatus::Withdrawn | InboxStatus::Failed) => Ok(None),
    }
}

/// The inbox's entry for `message_id`, to change.
fn entry_mut<'a>(state: &'a mut SessionState, message_id: &str) -> Option<&'a mut InboxEntry> {
    state
        .inbox
        .iter_mut()
        .find(|entry| entry.message_id == message_id)
}

/// Fold an inbox event. A delivery is progress: it earns a fresh relaunch budget.
pub(crate) fn apply(state: &mut SessionState, event: &SessionEvent) {
    match event {
        SessionEvent::MessageArrived(arrived) => state.inbox.push(InboxEntry {
            message_id: arrived.message_id.clone(),
            from: arrived.from.clone(),
            kind: arrived.kind.clone(),
            body_ref: arrived.body_ref.clone(),
            status: InboxStatus::Queued,
            claimant: String::new(),
            at_turn: None,
        }),
        SessionEvent::MessageClaimed(claimed) => {
            if let Some(entry) = entry_mut(state, &claimed.message_id) {
                entry.status = InboxStatus::Claimed;
                entry.claimant = claimed.claimant.clone();
            }
        }
        SessionEvent::MessageDelivered(delivered) => {
            if let Some(entry) = entry_mut(state, &delivered.message_id) {
                entry.status = InboxStatus::Delivered;
                entry.at_turn = Some(delivered.at_turn);
            }
            state.relaunches = 0;
        }
        SessionEvent::MessageFailed(failed) => {
            if let Some(entry) = entry_mut(state, &failed.message_id) {
                entry.status = InboxStatus::Failed;
            }
        }
        SessionEvent::SteerWithdrawn(withdrawn) => {
            if let Some(entry) = entry_mut(state, &withdrawn.message_id) {
                entry.status = InboxStatus::Withdrawn;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "inbox.test.rs"]
mod tests;
