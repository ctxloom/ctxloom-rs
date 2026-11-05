//! The `ports` crate's clock, delay and jitter, as the runner's own traits.
//!
//! `angzarr-embedded-runner` states [`Clock`], [`Delay`] and [`Jitter`]
//! itself; `ports` (taskloom's) states the same three, signature for
//! signature, as distinct traits, and its adapters (`system`, the fakes)
//! implement those. [`Port`] lends one of them to the runner for one call.

use std::future::Future;
use std::time::Duration;

use angzarr_embedded::Timestamp;

/// A `ports` clock, delay or jitter, lent to the runner.
pub(crate) struct Port<'p, T>(pub &'p mut T);

impl<T: ports::Clock> angzarr_embedded_runner::Clock for Port<'_, T> {
    fn now(&mut self) -> Timestamp {
        self.0.now()
    }
}

impl<T: ports::Delay> angzarr_embedded_runner::Delay for Port<'_, T> {
    fn sleep(&mut self, duration: Duration) -> impl Future<Output = ()> + Send {
        self.0.sleep(duration)
    }
}

impl<T: ports::Jitter> angzarr_embedded_runner::Jitter for Port<'_, T> {
    fn up_to(&mut self, ceiling: Duration) -> Duration {
        self.0.up_to(ceiling)
    }
}

#[cfg(test)]
#[path = "bridge.test.rs"]
mod tests;
