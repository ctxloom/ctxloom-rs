//! The backoff schedule, and running a command again when annals says to.

use std::time::Duration;

use annals_diagnostic::Diagnostic;

use super::{Backoff, Outcome, Runner};
use crate::fakes::{ready, FakeClock, FakeDelay, Fault, Flaky};
use crate::{book, registry, AppError};
use annals::next::AsyncEventStore;
use tasks::{AddTask, TaskRejection};

const MS: Duration = Duration::from_millis(1);

#[test]
fn test_each_ceiling_doubles_the_last_from_the_base() {
    let backoff = Backoff {
        base: 10 * MS,
        cap: Duration::from_secs(60),
        attempts: 10,
    };
    let ceilings: Vec<Duration> = (1..=5).map(|n| backoff.ceiling(n)).collect();
    assert_eq!(ceilings, [10 * MS, 20 * MS, 40 * MS, 80 * MS, 160 * MS]);
}

#[test]
fn test_no_ceiling_exceeds_the_cap() {
    let backoff = Backoff {
        base: 10 * MS,
        cap: 50 * MS,
        attempts: 10,
    };
    assert_eq!(backoff.ceiling(3), 40 * MS);
    assert_eq!(backoff.ceiling(4), 50 * MS);
    assert_eq!(
        backoff.ceiling(40),
        50 * MS,
        "a doubling past u32 saturates"
    );
    assert_eq!(backoff.ceiling(u32::MAX), 50 * MS);
}

#[test]
fn test_the_default_is_10ms_doubling_to_1s_over_10_attempts() {
    assert_eq!(
        Backoff::DEFAULT,
        Backoff {
            base: 10 * MS,
            cap: Duration::from_secs(1),
            attempts: 10,
        }
    );
}

/// Run an add of "swift-amber" on `store`, under the first command id.
fn run(store: &mut Flaky, delay: &mut FakeDelay, attempts: u32) -> Result<Outcome, AppError> {
    run_as(1, store, delay, attempts)
}

/// [`run`], under the command id of `id`s.
fn run_as(
    id: u8,
    store: &mut Flaky,
    delay: &mut FakeDelay,
    attempts: u32,
) -> Result<Outcome, AppError> {
    let registry = registry();
    let mut clock = FakeClock::default();
    let command_id = annals::next::v1::Uuid::from_bytes([id; 16]);
    let add = AddTask {
        harp_id: "swift-amber".to_string(),
        text: "Ship it".to_string(),
        status: String::new(),
        trigger: String::new(),
        tags: Vec::new(),
        origin_session: String::new(),
    };
    let book = book("swift-amber", add.into());
    let mut runner = Runner {
        registry: &registry,
        clock: &mut clock,
        delay,
        backoff: Backoff {
            base: 10 * MS,
            cap: 30 * MS,
            attempts,
        },
    };
    ready(runner.run(store, &book, &command_id))
}

fn events(store: &mut Flaky) -> usize {
    let cover = crate::cover("swift-amber");
    ready(store.read_from(&cover, 0))
        .unwrap()
        .map_or(0, |slice| slice.pages.len())
}

#[test]
fn test_a_command_that_commits_at_once_waits_for_nothing() {
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    let outcome = run(&mut store, &mut delay, 3).unwrap();
    assert!(matches!(outcome, Outcome::Committed));
    assert_eq!(store.begun, 1);
    assert!(delay.slept.is_empty());
    assert_eq!(events(&mut store), 1);
}

#[test]
fn test_a_retryable_failure_is_run_again_after_a_growing_capped_wait() {
    let mut store = Flaky::failing(&[Fault::Unavailable, Fault::Conflict, Fault::Unavailable]);
    let mut delay = FakeDelay::default();
    let outcome = run(&mut store, &mut delay, 5).unwrap();
    assert!(matches!(outcome, Outcome::Committed));
    assert_eq!(store.begun, 4, "three failures, then the commit");
    assert_eq!(delay.ceilings, [10 * MS, 20 * MS, 30 * MS]);
    assert_eq!(delay.slept, [10 * MS, 20 * MS, 30 * MS]);
    assert_eq!(events(&mut store), 1, "landed once");
}

#[test]
fn test_each_wait_is_the_jitter_drawn_under_its_ceiling() {
    let mut store = Flaky::failing(&[Fault::Unavailable, Fault::Unavailable]);
    let mut delay = FakeDelay {
        permille: 500,
        ..FakeDelay::default()
    };
    run(&mut store, &mut delay, 5).unwrap();
    assert_eq!(delay.slept, [5 * MS, 10 * MS]);
}

#[test]
fn test_a_jitter_past_its_ceiling_is_held_to_it() {
    let mut store = Flaky::failing(&[Fault::Unavailable]);
    let mut delay = FakeDelay {
        over: Some(Duration::from_secs(9)),
        ..FakeDelay::default()
    };
    run(&mut store, &mut delay, 5).unwrap();
    assert_eq!(delay.slept, [10 * MS]);
}

#[test]
fn test_a_retry_after_a_lost_answer_is_the_duplicate_of_what_landed() {
    let mut store = Flaky::failing(&[Fault::LandsThenFails]);
    let mut delay = FakeDelay::default();
    let outcome = run(&mut store, &mut delay, 5).unwrap();
    assert!(matches!(outcome, Outcome::Committed));
    assert_eq!(store.begun, 2, "retried once, and found on the stream");
    assert_eq!(events(&mut store), 1, "not added twice");
}

#[test]
fn test_the_attempts_run_out_and_it_gives_up_with_the_last_failure() {
    let mut store = Flaky::failing(&[Fault::Unavailable; 3]);
    let mut delay = FakeDelay::default();
    let Err(AppError::GaveUp { attempts, error }) = run(&mut store, &mut delay, 3) else {
        panic!("gave up");
    };
    assert_eq!(attempts, 3);
    assert_eq!(store.begun, 3);
    assert_eq!(delay.slept.len(), 2, "no wait after the last attempt");
    assert_eq!(error.code(), "annals-next-store-unavailable");
    assert_eq!(events(&mut store), 0);
}

#[test]
fn test_a_failure_no_retry_mends_is_not_retried() {
    let mut store = Flaky::failing(&[Fault::Discontinuous]);
    let mut delay = FakeDelay::default();
    let Err(AppError::Failed(error)) = run(&mut store, &mut delay, 5) else {
        panic!("failed");
    };
    assert_eq!(error.code(), "annals-next-store-discontinuous");
    assert_eq!(store.begun, 1);
    assert!(delay.slept.is_empty());
}

#[test]
fn test_a_rejection_is_answered_not_retried() {
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    run(&mut store, &mut delay, 5).unwrap();
    let Ok(Outcome::Refused(rejection)) = run_as(2, &mut store, &mut delay, 5) else {
        panic!("refused");
    };
    assert_eq!(
        rejection,
        TaskRejection::TaskExists {
            harp_id: "swift-amber".to_string()
        }
    );
    assert_eq!(store.begun, 2);
    assert!(delay.slept.is_empty());
}
