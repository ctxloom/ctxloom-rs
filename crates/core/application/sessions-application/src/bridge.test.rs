//! `Port` hands the runner's three traits to the `ports` ones, unchanged.

use std::time::Duration;

use angzarr_embedded_runner::{Clock, Delay, Jitter};

use super::Port;
use crate::fakes::{ready, FakeClock, FakeDelay};

#[test]
fn the_clock_is_the_ports_clock() {
    let mut inner = FakeClock::default();
    let first = Port(&mut inner).now();
    let second = Port(&mut inner).now();
    assert!(second.seconds > first.seconds, "the fake clock advances");
    assert_eq!(first.seconds, 1_700_000_001);
}

#[test]
fn the_delay_is_slept_on_the_ports_delay() {
    let mut inner = FakeDelay::default();
    ready(Port(&mut inner).sleep(Duration::from_secs(3)));
    assert_eq!(inner.slept, [Duration::from_secs(3)]);
}

#[test]
fn the_jitter_is_drawn_from_the_ports_jitter() {
    let mut inner = FakeDelay {
        permille: 250,
        ..FakeDelay::default()
    };
    let drawn = Port(&mut inner).up_to(Duration::from_secs(8));
    assert_eq!(drawn, Duration::from_secs(2));
    assert_eq!(inner.ceilings, [Duration::from_secs(8)]);
}
