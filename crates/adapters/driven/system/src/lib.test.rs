//! The system's time, ids, waits and draws.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ports::{Clock, CommandIds, Delay, Jitter};

use super::System;

fn unix_now() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}

#[test]
fn test_now_is_the_system_clocks_time() {
    let before = unix_now();
    let now = System.now();
    let after = unix_now();
    let read = Duration::new(
        u64::try_from(now.seconds).unwrap(),
        u32::try_from(now.nanos).unwrap(),
    );
    assert!(
        before <= read && read <= after,
        "{before:?} <= {read:?} <= {after:?}"
    );
}

#[test]
fn test_a_command_id_is_a_uuid_v7_and_each_sorts_after_the_last() {
    let ids: Vec<[u8; 16]> = (0..100)
        .map(|_| {
            let id = System.command_id();
            <[u8; 16]>::try_from(&id.value[..]).expect("16 bytes")
        })
        .collect();
    for id in &ids {
        assert_eq!(id[6] >> 4, 7, "version 7");
        assert_eq!(id[8] >> 6, 0b10, "the RFC 9562 variant");
    }
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "time-ordered");
}

#[test]
fn test_sleep_waits_at_least_the_duration() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let start = Instant::now();
    runtime.block_on(System.sleep(Duration::from_millis(30)));
    assert!(start.elapsed() >= Duration::from_millis(30));
}

#[test]
fn test_jitter_is_drawn_at_random_up_to_the_ceiling() {
    let ceiling = Duration::from_millis(5);
    let draws: Vec<Duration> = (0..200).map(|_| System.up_to(ceiling)).collect();
    assert!(draws.iter().all(|d| *d <= ceiling), "{draws:?}");
    let mut distinct = draws.clone();
    distinct.sort();
    distinct.dedup();
    assert!(distinct.len() > 10, "drawn, not fixed: {distinct:?}");
}

#[test]
fn test_jitter_under_a_zero_ceiling_is_zero_and_a_huge_one_saturates() {
    assert_eq!(System.up_to(Duration::ZERO), Duration::ZERO);
    let huge = System.up_to(Duration::MAX);
    assert!(huge <= Duration::from_nanos(u64::MAX));
}
