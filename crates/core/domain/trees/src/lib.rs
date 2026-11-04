//! The trees domain: the shrunk tree of plan 3.2 (D5 Q4). It keeps founding
//! and the delegation limits, ephemeral, and the current occupant; routing,
//! grants, credentials and spawn refusals have left it.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/ctxloom.runs.v1.domain.rs"));
}
mod aggregate;
mod rejection;
mod state;

pub use aggregate::TreeAggregate;
pub use generated::*;
#[cfg(any(test, feature = "test-support"))]
pub use rejection::diagnostic_samples;
pub use rejection::{TreeRejection, EN_US};
pub use state::{TreeState, TREE_DOMAIN};

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
