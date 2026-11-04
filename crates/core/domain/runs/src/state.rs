//! The run, folded from its events.

use crate::{EndCause, RunEvent, RunMode};

/// The runs domain's name: every run stream's `Cover.domain`.
pub const RUN_DOMAIN: &str = "runs";

/// The run, folded from its events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunState {
    pub session: String,
    pub incarnation: u32,
    pub mode: Option<RunMode>,
    /// SHA-256 of the RUNNER principal's token; the token itself is never
    /// journaled (D29).
    pub runner_credential_hash: Vec<u8>,
    /// SHA-256 of the MODEL principal's token. Always different from the
    /// runner's, so a token names exactly one principal (owner ruling
    /// 2026-11-04, item 16).
    pub model_credential_hash: Vec<u8>,
    /// Set by the fold of `RunEnded`, for every token, the owner's included,
    /// whatever the cause (D29).
    pub credential_revoked: bool,
    pub runner_attached: bool,
    /// The sequence of the `TurnQueued` event of the turn in flight (plan
    /// 4.3: the turn number is a per-stream position, the cause).
    pub turn_in_flight: Option<u64>,
    pub stop_requested: bool,
    /// How many summary claims the model has recorded: advisory evidence, no
    /// transition (owner ruling 2026-11-04, item 16).
    pub summaries: u32,
    /// The turn of the model's final claim, if it made one. A CLAIM: the run
    /// stays live, its credentials valid and its next turn queueable.
    pub final_reported: Option<u64>,
    pub ended: Option<EndCause>,
}

/// Fold `event`, the page at `sequence` of the run's stream, into `state`:
/// what the aggregate's appliers do, one place, so a reader of the stream
/// (the credential check) folds as the aggregate does. Pure.
pub fn apply(state: &mut RunState, event: &RunEvent, sequence: u64) {
    match event {
        RunEvent::RunEnqueued(enqueued) => {
            state.session = enqueued.session.clone();
            state.incarnation = enqueued.incarnation;
            state.mode = Some(enqueued.mode);
            state.runner_credential_hash = enqueued.runner_credential_hash.clone();
            state.model_credential_hash = enqueued.model_credential_hash.clone();
        }
        RunEvent::RunnerAttachedRecorded(_) => state.runner_attached = true,
        // The turn in flight is the sequence of its `TurnQueued` event (plan 4.3).
        RunEvent::TurnQueued(_) => state.turn_in_flight = Some(sequence),
        RunEvent::TurnStartedRecorded(_) => {}
        RunEvent::TurnCompletedRecorded(completed) => free_the_gate(state, completed.turn),
        RunEvent::TurnFailedRecorded(failed) => free_the_gate(state, failed.turn),
        RunEvent::StopRequested(_) => state.stop_requested = true,
        RunEvent::SummaryReportedRecorded(_) => state.summaries += 1,
        RunEvent::FinalReportedRecorded(reported) => state.final_reported = Some(reported.turn),
        // The end revokes every credential, the owner's included, whatever the
        // cause (D29).
        RunEvent::RunEnded(ended) => {
            state.ended = Some(ended.cause);
            state.credential_revoked = true;
        }
    }
}

/// A turn that completed or failed frees the gate, if it is the one in flight.
fn free_the_gate(state: &mut RunState, turn: u64) {
    if state.turn_in_flight == Some(turn) {
        state.turn_in_flight = None;
    }
}
