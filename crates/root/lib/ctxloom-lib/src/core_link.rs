//! The core served in process (S5): the kernel's `App` over a store, the
//! exposure map, and the `embedded-edge` adapter over it, so `Facts`,
//! `AgentRequests`, `Feeds` and `Admission` run against the real
//! `App::serve_in`. No transport: UDS, TCP and gRPC are not here. RED (S5): a
//! stub; `core_link.test.rs` proposes the wiring.

use std::sync::Arc;

use angzarr_embedded::{ListStreams, Processed, Store};
use angzarr_embedded_host::stores::OneStore;
use embedded_edge::EmbeddedEdge;

/// The in-process core over one store.
pub struct InProcessCore<S>
where
    S: Store + Processed + ListStreams + Send + Sync,
{
    edge: Arc<EmbeddedEdge<OneStore<S>>>,
}

impl<S> InProcessCore<S>
where
    S: Store + Processed + ListStreams + Send + Sync + 'static,
{
    /// The application over `store`: the sessions, runs and trees aggregates
    /// registered; sessions and runs exposed to the network (the container
    /// bridge reaches them) and trees to loopback; the kernel's retries off,
    /// so a fact at the race cap is parked at once.
    pub async fn start(store: S) -> InProcessCore<S> {
        let _ = store;
        todo!("S5 stub")
    }

    /// The edge the use cases take.
    pub fn edge(&self) -> Arc<EmbeddedEdge<OneStore<S>>> {
        Arc::clone(&self.edge)
    }
}

#[cfg(test)]
#[path = "core_link.test.rs"]
mod tests;
