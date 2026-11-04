//! The driven adapter that implements `run_ports::Edge` over
//! `angzarr_embedded_host::App` (the owner approved the in-process slice; no
//! transport). RED (S5): stubs. The conversions are pure and tested here; the
//! whole path over a real `App` is `ctxloom-lib`'s `core_link` test, because a
//! driven adapter sees ports and never a use case.
//!
//! angzarr-embedded stays reusable tooling: nothing of ctxloom (session, run,
//! bundle, approval) appears in this crate's signatures.

use std::sync::Arc;

use angzarr_embedded::v1::{CommandBook, Cover, EventPage};
use angzarr_embedded::{Answered, Failure, Rejected, Sampled, StoreError};
use angzarr_embedded_host::stores::Stores;
use angzarr_embedded_host::App;
use run_ports::{Arrival, Edge, Unserved};

/// The host's arrival for the core's.
pub fn host_arrival(arrival: Arrival) -> angzarr_embedded_host::Arrival {
    let _ = arrival;
    todo!("S5 stub")
}

/// The core's `Unserved` for the host's.
pub fn unserved(unserved: angzarr_embedded_host::Unserved) -> Unserved {
    let _ = unserved;
    todo!("S5 stub")
}

/// An application's edge: `serve` is `App::serve_in`, `handle` is
/// `App::handle_in`, `events` is `App::events_in`, each on the one store key.
pub struct EmbeddedEdge<R: Stores<Key = ()>> {
    app: Arc<App<R>>,
    sample: Box<dyn Fn() -> Sampled + Send + Sync>,
}

impl<R: Stores<Key = ()>> EmbeddedEdge<R> {
    /// An edge over `app`; `sample` gives each call its instant and command id
    /// (the composition root's clock and ids).
    pub fn new(
        app: Arc<App<R>>,
        sample: impl Fn() -> Sampled + Send + Sync + 'static,
    ) -> EmbeddedEdge<R> {
        EmbeddedEdge {
            app,
            sample: Box::new(sample),
        }
    }
}

impl<R: Stores<Key = ()>> Edge for EmbeddedEdge<R> {
    async fn serve(&self, book: CommandBook, arrival: Arrival) -> Result<Answered, Unserved> {
        let _ = (&self.app, &self.sample, book, arrival);
        todo!("S5 stub")
    }

    async fn handle(&self, book: CommandBook) -> Result<Answered, Failure<Rejected>> {
        let _ = book;
        todo!("S5 stub")
    }

    async fn events(&self, cover: &Cover, from: u64) -> Result<Vec<EventPage>, StoreError> {
        let _ = (cover, from);
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
