//! The use cases over the real `App::serve_in`, in process. RED (S5): the
//! wiring is a stub. These are the in-process Facts / AgentRequests / Feeds
//! checks of the task (scope 5) on the real exposure seam; the finer admission
//! rules are `runs-application`'s, over a kernel double.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use angzarr_embedded::testing::{Fault, FaultyStore, MemoryStore};
use angzarr_embedded::v1::{Cover, Uuid};
use angzarr_embedded::{CommandBookExt, Failure, KernelError, Malformed};
use embedded_edge::EmbeddedEdge;
use run_ports::{Arrival, Edge, Token, TokenSource, Unserved};
use runs_application::{
    Admission, AgentFact, AppError, Credential, EnqueueRequest, Enqueuer, Facts, Gate, Identity,
    Recorded, RunsGate, Why,
};

use crate::{run_id, InProcessCore};

const SESSION: &str = "5f0c7a52-1111-5111-8111-000000000001";

#[derive(Default)]
struct Counting(AtomicU32);

impl TokenSource for Counting {
    fn mint(&self) -> Token {
        let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Token::from_bytes([n as u8; 32])
    }
}

type Core = InProcessCore<FaultyStore<MemoryStore>>;
type TheEdge = EmbeddedEdge<angzarr_embedded_host::stores::OneStore<FaultyStore<MemoryStore>>>;

struct World {
    core: Core,
    store: FaultyStore<MemoryStore>,
    /// The runner principal's credential and identity.
    credential: Credential,
    identity: Identity,
    /// The model principal's (owner ruling 2026-11-04, item 16).
    model_credential: Credential,
}

impl World {
    /// One session, minted, with its first run enqueued, through the real App.
    async fn new() -> World {
        let store = FaultyStore::<MemoryStore>::default();
        let core = InProcessCore::start(store.clone()).await;
        let edge = core.edge();
        let mint = sessions::MintSession {
            harp: "amber-swift-owl".to_string(),
            project_id: "calm-otter".to_string(),
            project_dir: "/w".to_string(),
            output_dir: "/o".to_string(),
            engine: "claude-code".to_string(),
            origin: sessions::Origin::Session,
            started_at: None,
            depth: 0,
        };
        let cover = Cover {
            domain: sessions::SESSION_DOMAIN.to_string(),
            root: Some(Uuid::from_bytes(
                *uuid::Uuid::parse_str(SESSION).unwrap().as_bytes(),
            )),
            edition: None,
        };
        edge.handle(CommandBook::command_of(
            cover,
            &sessions::SessionCommand::from(mint),
        ))
        .await
        .unwrap();
        let enqueued = Enqueuer::new(Arc::clone(&edge), Arc::new(Counting::default()))
            .enqueue(EnqueueRequest {
                session: SESSION.to_string(),
                incarnation: 1,
                mode: runs::RunMode::Interactive,
            })
            .await
            .unwrap();
        let credential = Credential {
            token: enqueued.runner_token,
            run: enqueued.run.clone(),
        };
        let model_credential = Credential {
            token: enqueued.model_token,
            run: enqueued.run,
        };
        let identity = RunsGate::new(Arc::clone(&edge))
            .identify(&credential)
            .await
            .unwrap();
        World {
            core,
            store,
            credential,
            identity,
            model_credential,
        }
    }

    fn edge(&self) -> Arc<TheEdge> {
        self.core.edge()
    }
}

use angzarr_embedded::v1::CommandBook;

fn exited() -> AgentFact {
    AgentFact::RunExited(runs::RunExited { code: 0, signal: 0 })
}

// NEW-DESIGN (plan 6.2 "one code path": run_id in S4 and the run domain's
// root agree): the run domain's root is the id `ctxloom_lib::run_id` derives.
#[test]
fn the_run_domains_root_is_the_id_the_library_derives() {
    let derived = run_id(SESSION, 3).unwrap();
    assert_eq!(
        runs::run_root(SESSION, 3).value,
        derived.as_bytes().to_vec()
    );
}

// PORTED-FROM-GO: S5 plan red test "a fact on the caller's own run is APPLIED,
// and the same fact again is DUPLICATE" -- over the real `App::serve_in`.
#[tokio::test]
async fn a_fact_is_applied_and_again_duplicate_over_app_serve_in() {
    let world = World::new().await;
    let facts = Facts::new(world.edge());
    assert_eq!(
        facts.record(&world.identity, exited()).await,
        Ok(Recorded::Applied)
    );
    assert_eq!(
        facts.record(&world.identity, exited()).await,
        Ok(Recorded::Duplicate)
    );
}

// NEW-DESIGN (exposure seam, `Unserved::Hidden`): a domain not exposed to the
// arrival is answered exactly as a domain that is not registered is.
#[tokio::test]
async fn an_unknown_domain_is_hidden_and_answered_like_a_missing_one() {
    let world = World::new().await;
    let edge = world.edge();
    let book = |domain: &str| {
        let cover = Cover {
            domain: domain.to_string(),
            root: Some(world.identity.run.clone()),
            edition: None,
        };
        CommandBook::command_of(
            cover,
            &sessions::SessionCommand::from(sessions::ForgetSession {}),
        )
    };
    let hidden = edge.serve(book("trees"), Arrival::Ip).await.unwrap_err();
    let missing = edge.serve(book("nowhere"), Arrival::Ip).await.unwrap_err();
    assert!(matches!(hidden, Unserved::Hidden { .. }));
    let unknown = |domain: &str| {
        Failure::Kernel(KernelError::Malformed(Malformed::UnknownDomain {
            domain: domain.to_string(),
        }))
    };
    assert_eq!(hidden.answer(), unknown("trees"));
    assert_eq!(missing.answer(), unknown("nowhere"));
}

// PORTED-FROM-GO: S5 plan red test "a fact at the race cap is PARKED, and the
// client stops retrying it" -- over the real App with a store that loses every
// race; nothing is persisted.
#[tokio::test]
async fn a_fact_at_the_race_cap_is_parked_over_app_serve_in() {
    let world = World::new().await;
    let facts = Facts::new(world.edge());
    let before = world
        .edge()
        .events(&world.identity.run_cover(), 0)
        .await
        .unwrap()
        .len();
    world.store.fail(std::iter::repeat_n(Fault::Conflict, 64));
    assert_eq!(
        facts.record(&world.identity, exited()).await,
        Ok(Recorded::Parked)
    );
    assert_eq!(
        world
            .edge()
            .events(&world.identity.run_cover(), 0)
            .await
            .unwrap()
            .len(),
        before
    );
}

// NEW-DESIGN (D29; item 16): the run's end, recorded by the fact, revokes BOTH
// principals' tokens over the real App: the next request with either is refused.
#[tokio::test]
async fn the_token_is_refused_once_the_runs_end_is_recorded() {
    let world = World::new().await;
    let facts = Facts::new(world.edge());
    facts.record(&world.identity, exited()).await.unwrap();
    let admission = Admission::new(world.edge());
    let book = exited().book(&world.identity);
    for credential in [&world.credential, &world.model_credential] {
        let refused = admission
            .admit(Arrival::Ip, credential, book.clone())
            .await
            .unwrap_err();
        assert_eq!(refused, AppError::Unauthenticated(Why::Revoked));
    }
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16, fail closed): over the real
// App, the model's token cannot record the runner's exit, and the run lives on.
#[tokio::test]
async fn the_models_token_cannot_record_an_exit_over_app_serve_in() {
    let world = World::new().await;
    let admission = Admission::new(world.edge());
    let before = world
        .edge()
        .events(&world.identity.run_cover(), 0)
        .await
        .unwrap()
        .len();
    let denied = admission
        .admit(
            Arrival::Ip,
            &world.model_credential,
            exited().book(&world.identity),
        )
        .await
        .unwrap_err();
    assert!(matches!(denied, AppError::Denied(Unserved::Hidden { .. })));
    assert_eq!(
        world
            .edge()
            .events(&world.identity.run_cover(), 0)
            .await
            .unwrap()
            .len(),
        before
    );
}
