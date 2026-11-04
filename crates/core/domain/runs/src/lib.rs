//! The runs domain. A run is one runner incarnation of a session: its stream's
//! root is UUIDv5(session, incarnation) (plan 3.3, D5 Q4), so a session's
//! second incarnation is a new stream and a late fact for an ended run can
//! never touch the resumed one. ONE run per incarnation, NOT per turn.

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
pub use state::{apply, RunState, RUN_DOMAIN};

/// The root of the run stream for `session`'s `incarnation`: UUIDv5 of the
/// incarnation's decimal text in the session's own UUID namespace. FIXED once
/// chosen: run ids are derived from it. A session that is not UUID text has
/// the namespace of its own text, so the root is still a pure function of both.
pub fn run_root(session: &str, incarnation: u32) -> angzarr_embedded::v1::Uuid {
    let namespace = uuid::Uuid::parse_str(session)
        .unwrap_or_else(|_| uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, session.as_bytes()));
    let root = uuid::Uuid::new_v5(&namespace, incarnation.to_string().as_bytes());
    angzarr_embedded::v1::Uuid::from_bytes(*root.as_bytes())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
