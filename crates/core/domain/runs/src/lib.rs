//! The runs domain. A run is one runner incarnation of a session: its stream's
//! root is UUIDv5(session, incarnation) (plan 3.3, D5 Q4), so a session's
//! second incarnation is a new stream and a late fact for an ended run can
//! never touch the resumed one. ONE run per incarnation, NOT per turn.
//!
//! RED (S5, design-by-test): the rules are stubs. The tests in
//! `aggregate.test.rs` and `lib.test.rs` propose the signatures and the
//! behaviour; they are committed red for review.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/ctxloom.runs.v1.domain.rs"));
}
mod aggregate;
mod rejection;
mod state;

pub use aggregate::RunAggregate;
pub use generated::*;
#[cfg(any(test, feature = "test-support"))]
pub use rejection::diagnostic_samples;
pub use rejection::{RunRejection, EN_US};
pub use state::{RunState, RUN_DOMAIN};

/// The root of the run stream for `session`'s `incarnation`: UUIDv5 of the
/// incarnation's decimal text in the session's own UUID namespace. FIXED once
/// chosen: run ids are derived from it (stub).
pub fn run_root(session: &str, incarnation: u32) -> angzarr_embedded::v1::Uuid {
    let _ = (session, incarnation);
    todo!("S5 stub: UUIDv5(session, incarnation)")
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
