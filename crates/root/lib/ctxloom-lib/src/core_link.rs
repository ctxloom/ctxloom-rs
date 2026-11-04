//! The core served in process (S5): the kernel's `App` over a store, the
//! exposure map, and the `embedded-edge` adapter over it, so `Facts`,
//! `AgentRequests`, `Feeds` and `Admission` run against the real
//! `App::serve_in`. No transport: UDS, TCP and gRPC are not here.
//! `core_link.test.rs` runs the wiring.

use std::sync::Arc;

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use angzarr_embedded::v1::Uuid as WireUuid;
use angzarr_embedded::{ListStreams, OnRetryable, Processed, Registry, Sampled, Store, Timestamp};
use angzarr_embedded_host::config::Expose;
use angzarr_embedded_host::stores::OneStore;
use angzarr_embedded_host::App;
use embedded_edge::EmbeddedEdge;
use runs::RunAggregate;
use sessions::{NameClaimAggregate, SessionAggregate};
use trees::TreeAggregate;

/// How many times the kernel runs a fact that keeps losing its race before it
/// parks it. In process nothing is far away, so no backoff separates the runs.
/// Parking writes the fact's dead letter, itself a store write: a store that
/// refuses every write parks nothing (the write of the letter fails too), and a
/// test that scripts losses scripts exactly this many.
pub(crate) const RACE_RUNS: u32 = 3;

/// The instant and the fresh command id each call is handled at: this is the
/// composition root, where the clock and the ids live.
fn sample() -> Sampled {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Sampled {
        now: Timestamp {
            seconds: since.as_secs() as i64,
            nanos: since.subsec_nanos() as i32,
        },
        command_id: WireUuid::from_bytes(*uuid::Uuid::now_v7().as_bytes()),
    }
}

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
        let mut registry = Registry::new();
        // The registrations are fixed here, so a failure is a bug of this
        // function, found by the first start.
        registry
            .aggregate::<SessionAggregate>(SessionAggregate::default)
            .expect("the sessions aggregate registers");
        registry
            .aggregate::<NameClaimAggregate>(NameClaimAggregate::default)
            .expect("the session names aggregate registers");
        registry
            .aggregate::<RunAggregate>(RunAggregate::default)
            .expect("the runs aggregate registers");
        registry
            .aggregate::<TreeAggregate>(TreeAggregate::default)
            .expect("the trees aggregate registers");
        let exposed = BTreeMap::from([
            (sessions::SESSION_DOMAIN.to_string(), Expose::Ip),
            (runs::RUN_DOMAIN.to_string(), Expose::Ip),
            (trees::TREE_DOMAIN.to_string(), Expose::Loopback),
        ]);
        let retries = OnRetryable {
            max_retries: 0,
            max_race_runs: RACE_RUNS,
            initial_backoff: Duration::ZERO,
            multiplier: 1.0,
            max_backoff: Duration::ZERO,
            jitter: 0.0,
        };
        let app = App::builder(OneStore::new(store), registry)
            .exposed(exposed)
            .on_retryable(retries)
            .start()
            .await;
        InProcessCore {
            edge: Arc::new(EmbeddedEdge::new(Arc::new(app), sample)),
        }
    }

    /// The edge the use cases take.
    pub fn edge(&self) -> Arc<EmbeddedEdge<OneStore<S>>> {
        Arc::clone(&self.edge)
    }
}

#[cfg(test)]
#[path = "core_link.test.rs"]
mod tests;
