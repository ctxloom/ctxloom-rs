//! The edge: the exposure seam as a port.

use std::future::Future;

use angzarr_embedded::v1::{CommandBook, Cover, EventPage};
use angzarr_embedded::{Answered, Failure, KernelError, Malformed, Rejected, StoreError};

/// Where a request arrived from (angzarr-embedded-host's `Arrival`, here as
/// the core's own): the host listener's loopback or the network (the
/// container bridge).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    Loopback,
    Ip,
}

/// Why a request was not served (the host's `Unserved`, as the core's own).
#[derive(Clone, Debug, PartialEq)]
pub enum Unserved {
    /// The domain is not exposed to where the request arrived from, or the
    /// identity does not own the stream: answered exactly as an unknown
    /// domain is. `domain` is the real one, for the host's own log.
    Hidden { domain: String },
    /// The kernel's own failure.
    Failed(Failure<Rejected>),
}

impl Unserved {
    /// What the CALLER is answered: for a hidden domain, the kernel's
    /// unknown-domain refusal, the same a missing domain gets.
    pub fn answer(&self) -> Failure<Rejected> {
        match self {
            Unserved::Hidden { domain } => {
                Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
                    domain: domain.clone(),
                }))
            }
            Unserved::Failed(failure) => failure.clone(),
        }
    }
}

/// The core as the agent listener reaches it. `serve` is the exposure seam
/// (`App::serve_in`); `handle` is the core's own unexposed path
/// (`App::handle_in`); `events` is a read of one stream from a sequence.
pub trait Edge: Send + Sync {
    /// One request to the listener, from `arrival`.
    fn serve(
        &self,
        book: CommandBook,
        arrival: Arrival,
    ) -> impl Future<Output = Result<Answered, Unserved>> + Send;

    /// One command of the core's own, not subject to exposure.
    fn handle(
        &self,
        book: CommandBook,
    ) -> impl Future<Output = Result<Answered, Failure<Rejected>>> + Send;

    /// The pages of `cover`'s stream from `from` on, oldest first; none for an
    /// absent stream.
    fn events(
        &self,
        cover: &Cover,
        from: u64,
    ) -> impl Future<Output = Result<Vec<EventPage>, StoreError>> + Send;
}
