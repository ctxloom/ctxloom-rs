//! The driven adapter that implements `run_ports::Edge` over
//! `angzarr_embedded_host::App` (the owner approved the in-process slice; no
//! transport). The conversions are pure and tested here; the
//! whole path over a real `App` is `ctxloom-wiring`'s `core_link` test, because a
//! driven adapter sees ports and never a use case.
//!
//! angzarr-embedded stays reusable tooling: nothing of ctxloom (session, run,
//! bundle, approval) appears in this crate's signatures.

use std::sync::Arc;

use angzarr_embedded::v1::{CommandBook, Cover, EventBook, EventPage};
use angzarr_embedded::{Answered, Failure, Handled, Rejected, Sampled, StoreError};
use angzarr_embedded_host::stores::Stores;
use angzarr_embedded_host::App;
use run_ports::{Arrival, Edge, Unserved};

/// The host's arrival for the core's.
pub fn host_arrival(arrival: Arrival) -> angzarr_embedded_host::Arrival {
    match arrival {
        Arrival::Loopback => angzarr_embedded_host::Arrival::Loopback,
        Arrival::Ip => angzarr_embedded_host::Arrival::Ip,
    }
}

/// The core's `Unserved` for the host's.
pub fn unserved(unserved: angzarr_embedded_host::Unserved) -> Unserved {
    match unserved {
        angzarr_embedded_host::Unserved::Hidden { hidden, .. } => Unserved::Hidden {
            domain: hidden.domain,
        },
        angzarr_embedded_host::Unserved::Failed(failure) => Unserved::Failed(failure),
    }
}

/// What `Unserved`-free handling answers: the addressed stream's events (an
/// empty book when the chain wrote none to it) and the delivery outcome, as
/// the exposure seam answers.
fn answered(handled: Handled, cover: Option<Cover>) -> Answered {
    let events = handled
        .books()
        .into_iter()
        .find(|book| book.cover == cover)
        .unwrap_or(EventBook {
            cover,
            ..EventBook::default()
        });
    Answered {
        events: Some(events),
        outcome: Some(handled.outcome()),
    }
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
        self.app
            .serve(book, host_arrival(arrival), &(self.sample)())
            .await
            .map_err(unserved)
    }

    async fn handle(&self, book: CommandBook) -> Result<Answered, Failure<Rejected>> {
        let cover = book.cover.clone();
        let handled = self.app.handle(book, &(self.sample)()).await?;
        Ok(answered(handled, cover))
    }

    async fn events(&self, cover: &Cover, from: u64) -> Result<Vec<EventPage>, StoreError> {
        let pages = self.app.events(cover).await?;
        Ok(pages
            .into_iter()
            .filter(|page| page.sequence >= from)
            .collect())
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
