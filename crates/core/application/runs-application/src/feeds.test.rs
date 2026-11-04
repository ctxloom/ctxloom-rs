//! Feeds: per-stream catch-up from `(cover, seq)`, no gap and no duplicate
//! (plan 6; listener plan 5.2). RED (S5): stubs.

use std::sync::Arc;

use run_ports::Edge;

use crate::fakes::{fact_book, World};
use crate::{AppError, Cursor, Feeds, Why};

async fn fact(world: &World, id: &str, turn: u64) {
    let book = fact_book(
        world.me.identity.run_cover(),
        runs::TurnStarted { turn },
        id,
    );
    world.edge.handle(book).await.unwrap();
}

// NEW-DESIGN: a follow reads the pages at or above its cursor, and the cursor
// moves past them.
#[tokio::test]
async fn a_follow_reads_from_its_cursor_and_moves_past_what_it_read() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let mut follow = feeds.follow(
        world.me.credential.clone(),
        Cursor::start(&world.me.identity.run_cover()),
    );
    let first = follow.next().await.unwrap();
    assert_eq!(first.len(), 1, "the RunEnqueued");
    assert_eq!(first[0].sequence, 0);
    assert_eq!(follow.cursor().next_sequence, 1);
    assert!(follow.next().await.unwrap().is_empty(), "nothing new");
    assert_eq!(follow.cursor().next_sequence, 1);
}

// NEW-DESIGN (S5 plan red test: "Feeds resume from (cover, seq) after a dropped
// connection with no gap and no duplicate"; listener plan C-0824/C-0825): drop
// the follow, append, resume from the saved cursor: exactly the new pages.
#[tokio::test]
async fn a_dropped_follow_resumes_from_its_cursor_with_no_gap_and_no_duplicate() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let mut follow = feeds.follow(
        world.me.credential.clone(),
        Cursor::start(&world.me.identity.run_cover()),
    );
    fact(&world, "t1", 1).await;
    let seen = follow.next().await.unwrap();
    assert_eq!(seen.iter().map(|p| p.sequence).collect::<Vec<_>>(), [0, 1]);
    let saved = follow.cursor();
    drop(follow);

    fact(&world, "t2", 2).await;
    fact(&world, "t3", 3).await;
    let mut resumed = feeds.follow(world.me.credential.clone(), saved);
    let more = resumed.next().await.unwrap();
    assert_eq!(more.iter().map(|p| p.sequence).collect::<Vec<_>>(), [2, 3]);
    assert!(resumed.next().await.unwrap().is_empty());
}

// NEW-DESIGN (D29; listener plan 4.1 `still_valid`): a follow re-identifies on
// every poll, so a run's end ends its feed (a token dies with its run).
#[tokio::test]
async fn a_follow_ends_unauthenticated_once_its_run_has_ended() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let mut follow = feeds.follow(
        world.me.credential.clone(),
        Cursor::start(&world.me.identity.run_cover()),
    );
    follow.next().await.unwrap();
    world.end(&world.me).await;
    assert_eq!(
        follow.next().await.unwrap_err(),
        AppError::Unauthenticated(Why::Revoked)
    );
}

// NEW-DESIGN (listener plan 4.3, old Q6: "denied reads answered as an absent
// stream"): reading another run's stream is an empty read, not an error, so
// existence cannot be probed.
#[tokio::test]
async fn reading_a_stream_the_caller_does_not_own_is_an_empty_read() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let theirs = world.other.identity.run_cover();
    assert!(
        !world.edge.pages(&theirs).await.is_empty(),
        "precondition: it exists"
    );
    let read = feeds.read(&world.me.identity, &theirs, 0).await.unwrap();
    assert!(read.is_empty());
}

// NEW-DESIGN: a follow of a stream the caller does not own is empty too.
#[tokio::test]
async fn following_a_stream_the_caller_does_not_own_yields_nothing() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let mut follow = feeds.follow(
        world.me.credential.clone(),
        Cursor::start(&world.other.identity.run_cover()),
    );
    assert!(follow.next().await.unwrap().is_empty());
}

// NEW-DESIGN (S5 plan red test: "Mail from outside the run's workflow is
// caught by the session's inbox catch-up (D10; to verify here)"). Mail is a
// MessageArrived on the SESSION stream, caused by someone else's workflow, so
// the run's own feed never carries it; the inbox catch-up does, from the
// session's own cursor.
#[tokio::test]
async fn mail_from_outside_the_runs_workflow_is_caught_by_the_inbox_catch_up() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let who = &world.me.identity;
    let mut run_feed = feeds.follow(world.me.credential.clone(), Cursor::start(&who.run_cover()));
    run_feed.next().await.unwrap();

    let arrival = fact_book(
        who.session_cover(),
        sessions::RecordMessageArrived {
            message_id: "m1".to_string(),
            from: "peer".to_string(),
            kind: "note".to_string(),
            body_ref: "body:m1".to_string(),
        },
        "arrive:fixture:m1",
    );
    world.edge.handle(arrival).await.unwrap();

    assert!(
        run_feed.next().await.unwrap().is_empty(),
        "not the run's workflow"
    );
    let inbox = feeds.inbox(who, 0).await.unwrap();
    assert!(
        inbox.iter().any(|page| page
            .payload
            .as_ref()
            .is_some_and(|any| any.type_url.ends_with("MessageArrived"))),
        "the inbox catch-up finds it"
    );
}

// NEW-DESIGN (plan 6: "No watch carries or needs a position across streams"):
// the cursor names one stream and one sequence, nothing more.
#[test]
fn a_cursor_is_one_streams_position_and_nothing_wider() {
    let cursor = Cursor {
        domain: "runs".to_string(),
        root: angzarr_embedded::v1::Uuid::from_bytes([1; 16]),
        next_sequence: 5,
    };
    assert_eq!(cursor.cover().domain, "runs");
    assert_eq!(Cursor::start(&cursor.cover()).next_sequence, 0);
}

// NEW-DESIGN, owner ruling 2026-11-04 (item 16): reads are not facts and carry
// no authority, so the model principal reads its own run like the runner does
// (roster-style calls are reads, no event), and still only its own.
#[tokio::test]
async fn the_model_principal_reads_its_own_stream_and_no_other() {
    let world = World::new().await;
    let feeds = Feeds::new(Arc::clone(&world.edge));
    let mine = world.me.model_identity.run_cover();
    let theirs = world.other.identity.run_cover();
    assert_eq!(
        feeds
            .read(&world.me.model_identity, &mine, 0)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(feeds
        .read(&world.me.model_identity, &theirs, 0)
        .await
        .unwrap()
        .is_empty());
    let mut follow = feeds.follow(world.me.model_credential.clone(), Cursor::start(&mine));
    assert_eq!(follow.next().await.unwrap().len(), 1);
}
