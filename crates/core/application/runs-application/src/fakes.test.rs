//! The test doubles the use cases share: an [`Edge`] over the kernel and an
//! in-memory store (so a fact is APPLIED, DUPLICATE or PARKED by the kernel's
//! own rules, not the double's), and a counting token
//! source. The real edge over `App::serve_in` is `ctxloom-wiring`'s `core_link`
//! test.
//!
//! The doubles do NOT stand in for the domains: the world is built through the
//! real `sessions` and `runs` aggregates, so these tests are red until those
//! are implemented too.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use angzarr_embedded::angzarr_ports::{DeadLetter, DeadLetterStore};
use angzarr_embedded::markup::DomainMessage;
use angzarr_embedded::testing::{Fault, FaultyStore, MemoryStore, Samples};
use angzarr_embedded::v1::{
    command_page, Command, CommandBook, CommandPage, Cover, EventBook, EventPage, Fact, FactOrigin,
    Uuid,
};
use angzarr_embedded::{
    Answered, Any, Failure, Handled, Kernel, OnRetryable, Registry, Rejected, Sampled, StoreError,
    StreamRef,
};
use run_ports::{Arrival, Edge, Token, TokenSource, Unserved};
use sha2::{Digest, Sha256};

use crate::{Credential, Identity, Principal};

/// The kernel's race re-run cap in these tests, and so how many lost races
/// `lose_every_race` scripts: the kernel parks a fact at the cap by writing its
/// dead letter, and that write is a store write too, so it must find the
/// script spent (a script longer than the cap fails the letter's write, and the
/// fact is then an error, not parked). No backoff: nothing here waits.
const RACE_RUNS: u32 = 3;

/// How far a domain is exposed (the host's `Expose`); absent is none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reach {
    Loopback,
    Ip,
}

pub(crate) struct KernelEdge {
    registry: Registry,
    store: FaultyStore<MemoryStore>,
    samples: Mutex<Samples>,
    reach: BTreeMap<&'static str, Reach>,
    served: AtomicU32,
}

impl KernelEdge {
    /// `runs` and `sessions` exposed to the network (the container bridge
    /// reaches what the agent needs), `trees` to loopback only.
    pub(crate) fn new() -> KernelEdge {
        let mut registry = Registry::new();
        registry
            .aggregate::<sessions::SessionAggregate>(sessions::SessionAggregate::default)
            .unwrap();
        registry
            .aggregate::<runs::RunAggregate>(runs::RunAggregate::default)
            .unwrap();
        registry
            .aggregate::<trees::TreeAggregate>(trees::TreeAggregate::default)
            .unwrap();
        KernelEdge {
            registry,
            store: FaultyStore::default(),
            samples: Mutex::new(Samples::starting_at(1_000)),
            reach: BTreeMap::from([
                (runs::RUN_DOMAIN, Reach::Ip),
                (sessions::SESSION_DOMAIN, Reach::Ip),
                // Reached on this machine only, and by nothing an agent holds.
                (trees::TREE_DOMAIN, Reach::Loopback),
            ]),
            served: AtomicU32::new(0),
        }
    }

    /// Every write from now on loses its race to another writer.
    pub(crate) fn lose_every_race(&self) {
        self.store
            .fail(std::iter::repeat_n(Fault::Conflict, RACE_RUNS as usize));
    }

    /// How many requests reached the exposure seam.
    pub(crate) fn served(&self) -> u32 {
        self.served.load(Ordering::SeqCst)
    }

    /// Every page of a stream, as the store holds it.
    pub(crate) async fn pages(&self, cover: &Cover) -> Vec<EventPage> {
        self.events(cover, 0).await.unwrap()
    }

    /// The dead letters the kernel kept about `cover`'s stream, in the store.
    pub(crate) async fn dead_letters(&self, cover: &Cover) -> Vec<DeadLetter> {
        let stream = StreamRef::try_from(cover).unwrap();
        DeadLetterStore::dead_letters(&self.store, stream)
            .await
            .unwrap()
    }

    fn admits(&self, domain: &str, arrival: Arrival) -> bool {
        matches!(
            (self.reach.get(domain), arrival),
            (Some(_), Arrival::Loopback) | (Some(Reach::Ip), Arrival::Ip)
        )
    }

    fn sample(&self) -> Sampled {
        self.samples.lock().unwrap().next_sample()
    }

    async fn run(&self, book: CommandBook) -> Result<Answered, Failure<Rejected>> {
        let sampled = self.sample();
        let kernel = Kernel::new(&self.registry).on_retryable(OnRetryable {
            max_retries: 0,
            max_race_runs: RACE_RUNS,
            initial_backoff: std::time::Duration::ZERO,
            max_backoff: std::time::Duration::ZERO,
            ..OnRetryable::default()
        });
        let cover = book.cover.clone();
        let handled: Handled = kernel.handle_any(&self.store, book, &sampled).await?;
        let events = handled
            .books()
            .into_iter()
            .find(|book| book.cover == cover)
            .unwrap_or(EventBook {
                cover,
                ..EventBook::default()
            });
        Ok(Answered {
            events: Some(events),
            outcome: Some(handled.outcome()),
        })
    }
}

impl Edge for KernelEdge {
    async fn serve(&self, book: CommandBook, arrival: Arrival) -> Result<Answered, Unserved> {
        self.served.fetch_add(1, Ordering::SeqCst);
        let domain = book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_default();
        if !self.admits(&domain, arrival) {
            return Err(Unserved::Hidden { domain });
        }
        self.run(book).await.map_err(Unserved::Failed)
    }

    async fn handle(&self, book: CommandBook) -> Result<Answered, Failure<Rejected>> {
        self.run(book).await
    }

    async fn events(&self, cover: &Cover, from: u64) -> Result<Vec<EventPage>, StoreError> {
        let stream = StreamRef::try_from(cover)?;
        let slice = angzarr_embedded::angzarr_ports::EventStore::read(
            self.store.inner(),
            stream,
            from,
            None,
        )
        .await?;
        Ok(slice.pages)
    }
}

/// Tokens `[1; 32]`, `[2; 32]`, ...
#[derive(Default)]
pub(crate) struct CountingSource(AtomicU32);

impl TokenSource for CountingSource {
    fn mint(&self) -> Token {
        let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Token::from_bytes([n as u8; 32])
    }
}

pub(crate) const SESSION_ME: &str = "5f0c7a52-1111-5111-8111-000000000001";
pub(crate) const SESSION_OTHER: &str = "5f0c7a52-2222-5222-8222-000000000002";

pub(crate) fn uuid(text: &str) -> Uuid {
    Uuid::from_bytes(*uuid::Uuid::parse_str(text).unwrap().as_bytes())
}

/// A command as the core sends its own.
pub(crate) fn command_book(cover: Cover, message: impl DomainMessage) -> CommandBook {
    CommandBook {
        cover: Some(cover),
        page: Some(CommandPage {
            body: Some(command_page::Body::Command(Command {
                payload: Some(Any::from_msg(&message.to_wire()).unwrap()),
            })),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

/// A fact as the listener delivers one, built here and not by `RunnerFact`, so a
/// fixture never depends on the code under test.
pub(crate) fn fact_book(
    cover: Cover,
    message: impl DomainMessage,
    external_id: &str,
) -> CommandBook {
    CommandBook {
        cover: Some(cover),
        page: Some(CommandPage {
            body: Some(command_page::Body::Fact(Fact {
                payload: Some(Any::from_msg(&message.to_wire()).unwrap()),
            })),
            fact_origin: Some(FactOrigin {
                external_id: external_id.to_string(),
                description: String::new(),
            }),
            ..CommandPage::default()
        }),
        handling: None,
    }
}

/// One caller: a session with one live run and its TWO tokens (owner ruling
/// 2026-11-04, item 16). `identity`, `credential` and `token` are the RUNNER
/// principal's; the `model_*` are the model principal's.
pub(crate) struct Party {
    pub(crate) identity: Identity,
    pub(crate) credential: Credential,
    pub(crate) token: Token,
    pub(crate) model_identity: Identity,
    pub(crate) model_credential: Credential,
    pub(crate) model_token: Token,
}

pub(crate) struct World {
    pub(crate) edge: Arc<KernelEdge>,
    pub(crate) me: Party,
    pub(crate) other: Party,
}

impl World {
    /// Two sessions, each with its first run enqueued and its token minted: `me`
    /// an owner (depth 0), `other` another session's owner.
    pub(crate) async fn new() -> World {
        let edge = Arc::new(KernelEdge::new());
        let me = World::party(&edge, SESSION_ME, "amber-swift-owl", 0, 1).await;
        let other = World::party(&edge, SESSION_OTHER, "calm-otter-dawn", 0, 2).await;
        World { edge, me, other }
    }

    /// A child (depth 2) of `me`'s tree, with a run of its own.
    pub(crate) async fn child(&self) -> Party {
        World::party(
            &self.edge,
            "5f0c7a52-3333-5333-8333-000000000003",
            "lone-wolf-pup",
            2,
            3,
        )
        .await
    }

    async fn party(edge: &Arc<KernelEdge>, session: &str, harp: &str, depth: u32, n: u8) -> Party {
        let session_root = uuid(session);
        let session_cover = Cover {
            domain: sessions::SESSION_DOMAIN.to_string(),
            root: Some(session_root.clone()),
            edition: None,
        };
        edge.handle(command_book(
            session_cover,
            sessions::MintSession {
                harp: harp.to_string(),
                project_id: "calm-otter".to_string(),
                project_dir: "/work/proj".to_string(),
                output_dir: format!("/out/{harp}"),
                engine: "claude-code".to_string(),
                origin: sessions::Origin::Session,
                started_at: None,
                depth,
            },
        ))
        .await
        .unwrap();
        let token = Token::from_bytes([n; 32]);
        let model_token = Token::from_bytes([n + 100; 32]);
        let run_root = runs::run_root(session, 1);
        let run_cover = Cover {
            domain: runs::RUN_DOMAIN.to_string(),
            root: Some(run_root.clone()),
            edition: None,
        };
        edge.handle(command_book(
            run_cover,
            runs::EnqueueRun {
                session: session.to_string(),
                incarnation: 1,
                mode: runs::RunMode::Interactive,
                runner_credential_hash: Sha256::digest(token.as_bytes()).to_vec(),
                model_credential_hash: Sha256::digest(model_token.as_bytes()).to_vec(),
            },
        ))
        .await
        .unwrap();
        let identity = Identity {
            run: run_root.clone(),
            session: session_root,
            incarnation: 1,
            harp: harp.to_string(),
            depth,
            principal: Principal::Runner,
        };
        Party {
            model_identity: Identity {
                principal: Principal::Model,
                ..identity.clone()
            },
            identity,
            credential: Credential {
                token: token.clone(),
                run: run_root.clone(),
            },
            token,
            model_credential: Credential {
                token: model_token.clone(),
                run: run_root,
            },
            model_token,
        }
    }

    /// End `party`'s run, as its runner exiting would.
    pub(crate) async fn end(&self, party: &Party) {
        let book = fact_book(
            party.identity.run_cover(),
            runs::RunExited { code: 0, signal: 0 },
            "exit:fixture",
        );
        self.edge.handle(book).await.unwrap();
    }
}
