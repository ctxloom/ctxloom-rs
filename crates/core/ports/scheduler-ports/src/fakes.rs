//! Test doubles for the ports: a clock a test sets, and a wait that records
//! what it was asked and returns at once, so nothing sleeps.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use angzarr_embedded::Timestamp;

use crate::{Clock, Wait};

/// A clock that reads what the test last set, and moves only when told.
#[derive(Debug, Default)]
pub struct FakeClock {
    now: Mutex<Timestamp>,
}

impl FakeClock {
    /// A clock reading `seconds` after the epoch.
    pub fn at(seconds: i64) -> FakeClock {
        FakeClock {
            now: Mutex::new(Timestamp { seconds, nanos: 0 }),
        }
    }

    /// Read `now` from here on.
    pub fn set(&self, now: Timestamp) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = now;
    }

    /// Move `by` forward.
    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        let nanos = i64::from(now.nanos) + i64::from(by.subsec_nanos());
        now.seconds += i64::try_from(by.as_secs()).unwrap_or(i64::MAX) + nanos / 1_000_000_000;
        now.nanos = i32::try_from(nanos % 1_000_000_000).unwrap_or(0);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A [`Wait`] that returns at once, recording each wait it was asked for; its
/// draw is fixed. Optionally it moves a [`FakeClock`] by each wait, so a
/// looping test sees time pass without sleeping.
#[derive(Debug, Default)]
pub struct RecordingWait<'c> {
    waits: Mutex<Vec<Duration>>,
    clock: Option<&'c FakeClock>,
}

impl<'c> RecordingWait<'c> {
    /// A wait that only records.
    pub fn new() -> RecordingWait<'c> {
        RecordingWait::default()
    }

    /// A wait that also advances `clock` by what it is asked to wait.
    pub fn advancing(clock: &'c FakeClock) -> RecordingWait<'c> {
        RecordingWait {
            waits: Mutex::default(),
            clock: Some(clock),
        }
    }

    /// Every wait asked for, in order.
    pub fn waits(&self) -> Vec<Duration> {
        self.waits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Wait for RecordingWait<'_> {
    fn wait(&self, backoff: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.waits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(backoff);
        if let Some(clock) = self.clock {
            clock.advance(backoff);
        }
        Box::pin(std::future::ready(()))
    }

    fn draw(&self) -> f64 {
        0.0
    }
}
