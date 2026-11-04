//! The edge adapter's pure conversions. RED (S5): stubs.

use angzarr_embedded::{Failure, KernelError, Malformed};
use run_ports::Arrival;

use crate::{host_arrival, unserved};

// NEW-DESIGN (review question 6: the core's own Arrival, converted here).
#[test]
fn the_cores_arrival_is_the_hosts() {
    assert_eq!(
        host_arrival(Arrival::Loopback),
        angzarr_embedded_host::Arrival::Loopback
    );
    assert_eq!(
        host_arrival(Arrival::Ip),
        angzarr_embedded_host::Arrival::Ip
    );
}

// NEW-DESIGN: a hidden domain keeps its real name for the host's log, and its
// answer to the caller is the kernel's unknown domain.
#[test]
fn a_hidden_domain_is_hidden_and_answered_as_unknown() {
    let hidden = angzarr_embedded_host::Unserved::Hidden {
        answer: Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
            domain: "runs".to_string(),
        })),
        hidden: angzarr_embedded_host::NotExposed {
            domain: "runs".to_string(),
            arrival: angzarr_embedded_host::Arrival::Ip,
            expose: angzarr_embedded_host::config::Expose::None,
        },
    };
    let converted = unserved(hidden);
    assert_eq!(
        converted,
        run_ports::Unserved::Hidden {
            domain: "runs".to_string()
        }
    );
    assert_eq!(
        converted.answer(),
        Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
            domain: "runs".to_string()
        }))
    );
}

// NEW-DESIGN: the kernel's own failure is passed as it is.
#[test]
fn a_kernel_failure_is_passed_as_it_is() {
    let failure = Failure::Kernel(KernelError::ChainTooDeep { limit: 8 });
    let converted = unserved(angzarr_embedded_host::Unserved::Failed(failure.clone()));
    assert_eq!(converted, run_ports::Unserved::Failed(failure));
}

mod through_an_app {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;

    use angzarr_embedded::testing::MemoryStore;
    use angzarr_embedded::v1::{Cover, Uuid};
    use angzarr_embedded::{CommandBookExt, Registry, Sampled, Timestamp};
    use angzarr_embedded_host::config::Expose;
    use angzarr_embedded_host::stores::OneStore;
    use angzarr_embedded_host::App;
    use run_ports::{Arrival, Edge, Unserved};

    use crate::EmbeddedEdge;

    fn cover() -> Cover {
        Cover {
            domain: trees::TREE_DOMAIN.to_string(),
            root: Some(Uuid::from_bytes([9; 16])),
            edition: None,
        }
    }

    fn found() -> angzarr_embedded::v1::CommandBook {
        angzarr_embedded::v1::CommandBook::command_of(
            cover(),
            &trees::TreeCommand::from(trees::FoundTree {
                owner_session: "s".to_string(),
                ephemeral: false,
                concurrency: 1,
                depth: 1,
            }),
        )
    }

    fn occupy(session: &str) -> angzarr_embedded::v1::CommandBook {
        angzarr_embedded::v1::CommandBook::command_of(
            cover(),
            &trees::TreeCommand::from(trees::OccupyTree {
                session: session.to_string(),
            }),
        )
    }

    /// An edge over an application with trees exposed to loopback only.
    async fn edge() -> EmbeddedEdge<OneStore<MemoryStore>> {
        let mut registry = Registry::new();
        registry
            .aggregate::<trees::TreeAggregate>(trees::TreeAggregate::default)
            .unwrap();
        let app = App::builder(OneStore::new(MemoryStore::default()), registry)
            .exposed([(trees::TREE_DOMAIN.to_string(), Expose::Loopback)].into())
            .start()
            .await;
        let n = AtomicU8::new(0);
        EmbeddedEdge::new(Arc::new(app), move || {
            let n = n.fetch_add(1, Ordering::SeqCst) + 1;
            Sampled {
                now: Timestamp {
                    seconds: i64::from(n),
                    nanos: 0,
                },
                command_id: Uuid::from_bytes([0x70 | n; 16]),
            }
        })
    }

    // NEW-DESIGN: the core's own path answers the addressed stream's events
    // and the outcome.
    #[tokio::test]
    async fn handle_answers_the_addressed_streams_events() {
        let edge = edge().await;
        let answered = edge.handle(found()).await.unwrap();
        let book = answered.events.expect("events");
        assert_eq!(book.cover, Some(cover()));
        assert_eq!(book.pages.len(), 1);
        assert_eq!(
            answered.outcome.map(|o| o.delivery),
            Some(angzarr_embedded::v1::DeliveryOutcome::Applied as i32)
        );
    }

    // NEW-DESIGN: a command that records nothing is still answered for the
    // stream it addressed, with no events.
    #[tokio::test]
    async fn a_command_that_records_nothing_is_answered_with_its_stream_and_no_events() {
        let edge = edge().await;
        edge.handle(found()).await.unwrap();
        let answered = edge.handle(occupy("s")).await.unwrap();
        let book = answered.events.expect("events");
        assert_eq!(book.cover, Some(cover()));
        assert!(book.pages.is_empty());
    }

    // NEW-DESIGN: serve is the exposure seam: an arrival the domain is exposed
    // to is served as handle serves it; another is hidden.
    #[tokio::test]
    async fn serve_admits_by_arrival() {
        let edge = edge().await;
        let hidden = edge.serve(found(), Arrival::Ip).await.unwrap_err();
        assert_eq!(
            hidden,
            Unserved::Hidden {
                domain: trees::TREE_DOMAIN.to_string()
            }
        );
        let answered = edge.serve(found(), Arrival::Loopback).await.unwrap();
        assert_eq!(answered.events.expect("events").pages.len(), 1);
        assert!(edge.events(&cover(), 0).await.unwrap().len() == 1);
    }

    // NEW-DESIGN: events reads a stream from a sequence: oldest first, none
    // for an absent stream.
    #[tokio::test]
    async fn events_reads_from_a_sequence() {
        let edge = edge().await;
        assert!(edge.events(&cover(), 0).await.unwrap().is_empty());
        edge.handle(found()).await.unwrap();
        edge.handle(occupy("next")).await.unwrap();
        let sequences = |pages: Vec<angzarr_embedded::v1::EventPage>| {
            pages.into_iter().map(|p| p.sequence).collect::<Vec<_>>()
        };
        assert_eq!(sequences(edge.events(&cover(), 0).await.unwrap()), [0, 1]);
        assert_eq!(sequences(edge.events(&cover(), 1).await.unwrap()), [1]);
        assert!(edge.events(&cover(), 2).await.unwrap().is_empty());
    }
}
