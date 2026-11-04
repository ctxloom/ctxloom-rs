//! The inbound fact port (plan 4.2): narrow by construction. STUBS (S5 red).
//!
//! S5 carries the facts whose aggregates exist: the run's (attached, turn
//! started / completed / failed, exited) and the session inbox's (claimed,
//! delivered, failed). The plan's other variants (`RunPaused`, `RunResumed`,
//! `ArtifactPublished`, `ApprovalAsked`) arrive with their aggregates (S6, S9,
//! S10). `EngineSessionBound` is in: S4's bind became a fact (item 8, RULED).

use std::sync::Arc;

use angzarr_embedded::v1::{CommandBook, Cover};
use run_ports::Edge;

use crate::error::AppError;
use crate::identity::Identity;

/// What an agent may report. Closed: no command in it.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentFact {
    RunnerAttached(runs::RunnerAttached),
    TurnStarted(runs::TurnStarted),
    TurnCompleted(runs::TurnCompleted),
    TurnFailed(runs::TurnFailed),
    RunExited(runs::RunExited),
    MessageClaimed(sessions::ClaimMessage),
    MessageDelivered(sessions::DeliverMessage),
    MessageFailed(sessions::FailMessage),
    /// The SessionStart hook's bind, a fact since the owner ruling of
    /// 2026-11-04 (S5 review item 8): `bind:<session>:<native_id>`.
    EngineSessionBound(sessions::BindEngineSession),
}

impl AgentFact {
    /// The type URL of the fact's wire message.
    pub fn type_url(&self) -> String {
        todo!("S5 stub")
    }

    /// The `external_id`, derived from the fact's cause, never from a counter
    /// (plan 4.3): the run and session are `who`'s, in hyphenated lowercase.
    pub fn external_id(&self, who: &Identity) -> String {
        let _ = who;
        todo!("S5 stub")
    }

    /// The one stream this fact may address: `who`'s own run or session.
    pub fn cover(&self, who: &Identity) -> Cover {
        let _ = who;
        todo!("S5 stub")
    }

    /// The fact as the `CommandBook` the edge takes: a Fact body with its
    /// `fact_origin`.
    pub fn book(&self, who: &Identity) -> CommandBook {
        let _ = who;
        todo!("S5 stub")
    }

    /// One fact of every variant (a test's way to enumerate the allowlist).
    #[cfg(any(test, feature = "test-support"))]
    pub fn samples() -> Vec<AgentFact> {
        todo!("S5 stub")
    }
}

/// What the kernel did with a fact (the spec's `DeliveryOutcome`). Not the
/// host's `Recorded`/`Handled`: the two never meet in one module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorded {
    Applied,
    Duplicate,
    /// At the race cap (D13): dead-lettered, alerted, nothing persisted. The
    /// caller treats it as FINAL for that fact and never retries it.
    Parked,
}

/// Records facts on the caller's own streams.
pub struct Facts<E: Edge> {
    edge: Arc<E>,
}

impl<E: Edge> Facts<E> {
    pub fn new(edge: Arc<E>) -> Facts<E> {
        Facts { edge }
    }

    /// Record `fact` on a stream `who` owns. The answer is the kernel's: a fact it
    /// dead-letters (a handler bug, a race lost to the cap, or a rejection, which
    /// the kernel since 7873207 treats as its handler's bug) is `Parked`, and the
    /// letter is the kernel's, kept in the store; this crate files none.
    pub async fn record(&self, who: &Identity, fact: AgentFact) -> Result<Recorded, AppError> {
        let _ = (&self.edge, who, fact);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "facts.test.rs"]
mod tests;
