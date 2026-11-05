use std::time::Duration;

use angzarr_embedded::testing::block_on_ready;
use angzarr_embedded::Timestamp;

use crate::{Clock, FakeClock, RecordingWait, Wait};

#[test]
fn a_fake_clock_reads_what_was_set_and_moves_only_when_told() {
    let clock = FakeClock::at(100);
    assert_eq!(
        clock.now(),
        Timestamp {
            seconds: 100,
            nanos: 0
        }
    );
    assert_eq!(clock.now(), clock.now());
    clock.set(Timestamp {
        seconds: 5,
        nanos: 7,
    });
    assert_eq!(
        clock.now(),
        Timestamp {
            seconds: 5,
            nanos: 7
        }
    );
}

#[test]
fn advancing_carries_nanoseconds_into_seconds() {
    let clock = FakeClock::at(10);
    clock.set(Timestamp {
        seconds: 10,
        nanos: 900_000_000,
    });
    clock.advance(Duration::new(2, 300_000_000));
    assert_eq!(
        clock.now(),
        Timestamp {
            seconds: 13,
            nanos: 200_000_000
        }
    );
}

#[test]
fn a_recording_wait_returns_at_once_and_remembers_each_wait() {
    let wait = RecordingWait::new();
    block_on_ready(wait.wait(Duration::from_secs(3)));
    block_on_ready(wait.wait(Duration::from_millis(5)));
    assert_eq!(
        wait.waits(),
        vec![Duration::from_secs(3), Duration::from_millis(5)]
    );
    assert_eq!(wait.draw(), 0.0);
}

#[test]
fn an_advancing_wait_moves_its_clock_by_what_it_waited() {
    let clock = FakeClock::at(0);
    let wait = RecordingWait::advancing(&clock);
    block_on_ready(wait.wait(Duration::from_secs(30)));
    assert_eq!(clock.now().seconds, 30);
}
