//! The scheduler's ports.
//!
//! - [`Clock`]: the instant, UTC. The production adapter reads the system
//!   clock; the core reads none.
//! - Waiting is angzarr-embedded's own [`Wait`] (the kernel's timer port, with
//!   its jitter draw), re-exported: the loop waits through it so a test's wait
//!   returns at once.
//!
//! The fakes ([`FakeClock`], [`RecordingWait`]) are behind `test-support`.

use angzarr_embedded::Timestamp;

pub use angzarr_embedded::Wait;

/// The instant now, UTC.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

#[cfg(any(test, feature = "test-support"))]
mod fakes;
#[cfg(any(test, feature = "test-support"))]
pub use fakes::{FakeClock, RecordingWait};

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
