//! The run, folded from its events.

use crate::{EndCause, RunMode};

/// The runs domain's name: every run stream's `Cover.domain`.
pub const RUN_DOMAIN: &str = "runs";

/// The run, folded from its events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunState {
    pub session: String,
    pub incarnation: u32,
    pub mode: Option<RunMode>,
    /// SHA-256 of the run's token; the token itself is never journaled (D29).
    pub credential_hash: Vec<u8>,
    /// Set by the fold of `RunEnded`, for every token, the owner's included,
    /// whatever the cause (D29).
    pub credential_revoked: bool,
    pub runner_attached: bool,
    /// The sequence of the `TurnQueued` event of the turn in flight (plan
    /// 4.3: the turn number is a per-stream position, the cause).
    pub turn_in_flight: Option<u64>,
    pub stop_requested: bool,
    pub ended: Option<EndCause>,
}
