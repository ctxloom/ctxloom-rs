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

// ------------------------------------------------------------ conditions and purges

/// A runner over `registry`, `clock` and `delay`, 5 attempts.
fn runner<'a>(
    registry: &'a annals::next::Registry,
    clock: &'a mut FakeClock,
    delay: &'a mut FakeDelay,
) -> Runner<'a, FakeClock, FakeDelay> {
    Runner {
        registry,
        clock,
        delay,
        backoff: Backoff {
            base: 10 * MS,
            cap: 30 * MS,
            attempts: 5,
        },
    }
}

fn removal() -> annals::next::v1::CommandBook {
    let remove = tasks::RemoveTask {
        reason: tasks::RemovalReason::Pruned,
    };
    book("swift-amber", remove.into())
}

#[test]
fn test_a_command_whose_condition_fails_on_the_task_read_in_its_unit_is_not_sent() {
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    run(&mut store, &mut delay, 3).unwrap();
    let registry = registry();
    let mut clock = FakeClock::default();
    let command_id = annals::next::v1::Uuid::from_bytes([9; 16]);
    let mut runner = runner(&registry, &mut clock, &mut delay);
    let other = |task: &tasks::TaskState| task.text == "Something else";
    let outcome = ready(runner.run_where(&mut store, &removal(), &command_id, Some(&other)));
    assert!(outcome.unwrap().is_none(), "declined");
    assert_eq!(events(&mut store), 1, "nothing sent");

    let this = |task: &tasks::TaskState| task.text == "Ship it";
    let outcome = ready(runner.run_where(&mut store, &removal(), &command_id, Some(&this)));
    assert!(matches!(outcome, Ok(Some(Outcome::Committed))));
    assert_eq!(events(&mut store), 2, "the removal landed");
}

#[test]
fn test_a_condition_reads_through_a_unit_that_can_fail_and_be_retried() {
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    run(&mut store, &mut delay, 3).unwrap();
    store.faults = [Fault::Unavailable].into();
    let registry = registry();
    let mut clock = FakeClock::default();
    let command_id = annals::next::v1::Uuid::from_bytes([9; 16]);
    let mut runner = runner(&registry, &mut clock, &mut delay);
    let any = |_: &tasks::TaskState| true;
    let outcome = ready(runner.run_where(&mut store, &removal(), &command_id, Some(&any)));
    assert!(matches!(outcome, Ok(Some(Outcome::Committed))));
    assert_eq!(
        store.begun, 3,
        "the add, then the failed attempt and its retry"
    );
}

#[test]
fn test_a_purge_deletes_a_removed_tasks_stream_and_says_how_many_events() {
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    run(&mut store, &mut delay, 3).unwrap();
    let registry = registry();
    let mut clock = FakeClock::default();
    let command_id = annals::next::v1::Uuid::from_bytes([9; 16]);
    let mut runner = runner(&registry, &mut clock, &mut delay);
    ready(runner.run(&mut store, &removal(), &command_id)).unwrap();
    let stream = crate::cover("swift-amber");
    assert_eq!(ready(runner.purge(&mut store, &stream)).unwrap(), 2);
    assert_eq!(events(&mut store), 0);
    assert_eq!(
        ready(runner.purge(&mut store, &stream)).unwrap(),
        0,
        "no such stream"
    );
}

#[test]
fn test_a_purge_leaves_a_live_task_alone() {
    // A task minted on the harp after the removal was purged by another.
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    run(&mut store, &mut delay, 3).unwrap();
    let registry = registry();
    let mut clock = FakeClock::default();
    let mut runner = runner(&registry, &mut clock, &mut delay);
    let stream = crate::cover("swift-amber");
    assert_eq!(ready(runner.purge(&mut store, &stream)).unwrap(), 0);
    assert_eq!(events(&mut store), 1, "kept");
}

#[test]
fn test_a_purge_gives_up_once_its_attempts_are_spent() {
    let mut store = Flaky::default();
    let mut delay = FakeDelay::default();
    run(&mut store, &mut delay, 3).unwrap();
    let registry = registry();
    let mut clock = FakeClock::default();
    let command_id = annals::next::v1::Uuid::from_bytes([9; 16]);
    let mut runner = runner(&registry, &mut clock, &mut delay);
    ready(runner.run(&mut store, &removal(), &command_id)).unwrap();
    runner.delay.slept.clear();
    store.faults = [Fault::Conflict; 5].into();
    let stream = crate::cover("swift-amber");
    let Err(AppError::GaveUp { attempts, error }) = ready(runner.purge(&mut store, &stream)) else {
        panic!("gave up");
    };
    assert_eq!(attempts, 5);
    assert_eq!(error.code(), "annals-next-store-conflict");
    assert_eq!(runner.delay.slept.len(), 4);
    assert_eq!(events(&mut store), 2, "nothing purged");
}
