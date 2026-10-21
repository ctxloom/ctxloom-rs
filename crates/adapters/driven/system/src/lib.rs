//! The system behind the ports: the clock, UUIDv7 command ids, tokio's
//! timer, and thread-local randomness.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use annals::next::v1::Uuid;
use annals::next::Timestamp;
use ports::{Clock, CommandIds, Delay, Jitter};

/// The system's clock, ids, timer and randomness. Its sleep needs a tokio
/// runtime with the timer enabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct System;

impl Clock for System {
    /// The wall clock, since the Unix epoch; the epoch itself if the clock
    /// is set before it.
    fn now(&mut self) -> Timestamp {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Timestamp {
            seconds: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            nanos: i32::try_from(since.subsec_nanos()).unwrap_or(0),
        }
    }
}

impl CommandIds for System {
    /// A UUIDv7, ordered after every one this process minted before.
    fn command_id(&mut self) -> Uuid {
        Uuid::from_bytes(*uuid::Uuid::now_v7().as_bytes())
    }
}

impl Delay for System {
    async fn sleep(&mut self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

impl Jitter for System {
    /// Uniform over whole nanoseconds; a ceiling past `u64::MAX`
    /// nanoseconds (some 584 years) draws up to that.
    fn up_to(&mut self, ceiling: Duration) -> Duration {
        let nanos = u64::try_from(ceiling.as_nanos()).unwrap_or(u64::MAX);
        Duration::from_nanos(rand::random_range(0..=nanos))
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
