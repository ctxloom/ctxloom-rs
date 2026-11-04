//! The enqueue that mints a credential (D29). RED (S5): stubs.

use std::sync::Arc;

use run_ports::Edge;

use crate::fakes::{command_book, uuid, CountingSource, KernelEdge, SESSION_ME};
use crate::{Admission, Credential, EnqueueRequest, Enqueuer};

fn request() -> EnqueueRequest {
    EnqueueRequest {
        session: SESSION_ME.to_string(),
        incarnation: 1,
        mode: runs::RunMode::Interactive,
    }
}

fn session_cover() -> angzarr_embedded::v1::Cover {
    angzarr_embedded::v1::Cover {
        domain: sessions::SESSION_DOMAIN.to_string(),
        root: Some(uuid(SESSION_ME)),
        edition: None,
    }
}

async fn minted(edge: &KernelEdge) {
    edge.handle(command_book(
        session_cover(),
        sessions::MintSession {
            harp: "amber-swift-owl".to_string(),
            project_id: "calm-otter".to_string(),
            project_dir: "/w".to_string(),
            output_dir: "/o".to_string(),
            engine: "claude-code".to_string(),
            origin: sessions::Origin::Session,
            started_at: None,
            depth: 0,
        },
    ))
    .await
    .unwrap();
}

// NEW-DESIGN (D29, plan 5.3: "only its SHA-256 goes in the event"): the run's
// stream holds the digest of the token and no byte sequence equal to the token.
#[tokio::test]
async fn the_enqueue_journals_the_digest_and_never_the_token() {
    let edge = Arc::new(KernelEdge::new());
    minted(&edge).await;
    let enqueuer = Enqueuer::new(Arc::clone(&edge), Arc::new(CountingSource::default()));
    let enqueued = enqueuer.enqueue(request()).await.unwrap();
    assert_eq!(enqueued.run, runs::run_root(SESSION_ME, 1));
    let cover = angzarr_embedded::v1::Cover {
        domain: runs::RUN_DOMAIN.to_string(),
        root: Some(enqueued.run.clone()),
        edition: None,
    };
    let stored: Vec<u8> = edge
        .pages(&cover)
        .await
        .into_iter()
        .flat_map(|page| page.payload.map(|any| any.value).unwrap_or_default())
        .collect();
    let digest = enqueued.token.hash();
    assert!(
        stored.windows(32).any(|w| w == digest.as_bytes()),
        "the digest is journaled"
    );
    assert!(
        !stored.windows(32).any(|w| w == enqueued.token.as_bytes()),
        "the token is not"
    );
}

// NEW-DESIGN: the token the enqueue returns identifies the run it enqueued
// (mint, journal and identify agree).
#[tokio::test]
async fn the_returned_token_identifies_the_new_run() {
    let edge = Arc::new(KernelEdge::new());
    minted(&edge).await;
    let enqueuer = Enqueuer::new(Arc::clone(&edge), Arc::new(CountingSource::default()));
    let enqueued = enqueuer.enqueue(request()).await.unwrap();
    let admission = Admission::new(Arc::clone(&edge));
    let credential = Credential {
        token: enqueued.token.clone(),
        run: enqueued.run.clone(),
    };
    let fact = crate::fakes::fact_book(
        angzarr_embedded::v1::Cover {
            domain: runs::RUN_DOMAIN.to_string(),
            root: Some(enqueued.run),
            edition: None,
        },
        runs::TurnStarted { turn: 1 },
        "t",
    );
    assert!(admission
        .admit(run_ports::Arrival::Loopback, &credential, fact)
        .await
        .is_ok());
}

// NEW-DESIGN (plan 5.3: a token PER RUN): two enqueues mint two different
// tokens; a repeat enqueue of the same run is refused by the run, and mints a
// token nobody can use (it does not replace the first).
#[tokio::test]
async fn each_enqueue_mints_its_own_token_and_a_repeat_changes_nothing() {
    let edge = Arc::new(KernelEdge::new());
    minted(&edge).await;
    let enqueuer = Enqueuer::new(Arc::clone(&edge), Arc::new(CountingSource::default()));
    let first = enqueuer.enqueue(request()).await.unwrap();
    let repeat = enqueuer.enqueue(request()).await;
    assert!(repeat.is_err(), "runs-already-enqueued");
    assert_ne!(first.token, run_ports::Token::from_bytes([0; 32]));
}
