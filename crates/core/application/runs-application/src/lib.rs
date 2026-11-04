//! The runs use cases (S5, in-process): what an agent may do to the core, and
//! only that.
//!
//! - [`Facts`]: the inbound fact port, narrow by construction (plan 4.2). The
//!   closed [`RunnerFact`] and [`ModelFact`] have no command in them; every
//!   variant addresses the caller's own run or session, and its `external_id` is derived from its
//!   cause (plan 4.3).
//! - [`AgentRequests`]: the agent's refusable asks, equally closed and
//!   identity-stamped (D7). The MODEL principal's only.
//!
//! A run has TWO principals (owner ruling 2026-11-04, item 16): the runner,
//! which records authoritative observations, and the model, which records only
//! advisory CLAIM facts and sends the closed requests. Each has its own token
//! and allowlist; a decider role (S10, S14) turns a claim into a transition.
//! - [`Feeds`]: per-stream catch-up from a resume point, no cross-stream
//!   position anywhere (plan 6).
//! - [`Admission`]: credential to [`Identity`], the allowlist, `owns`,
//!   then the edge. The LOCAL shape of the gate the upstream listener will
//!   provide (embedded-listener.plan.md section 4); a stopgap SERVER is not
//!   here and is not authorized.
//! - [`Enqueuer`]: the run's enqueue, which mints the credential (D29).
//!
//! RED (S5, design-by-test): every body is a stub; the tests propose the
//! signatures and the behaviour.

mod admission;
mod enqueue;
mod error;
mod facts;
mod feeds;
mod gate;
mod identity;
mod requests;

pub use admission::Admission;
pub use enqueue::{EnqueueRequest, Enqueued, Enqueuer};
#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{AppError, Why, EN_US};
pub use facts::{Fact, Facts, ModelFact, Recorded, RunnerFact};
pub use feeds::{Cursor, Feeds, Follow};
pub use gate::{owns, Allowlist, Credential, Gate, RunsGate};
pub use identity::{Identity, Principal, Role};
pub use requests::{AgentRequest, AgentRequests, Requested};

#[cfg(test)]
#[path = "fakes.test.rs"]
mod fakes;
