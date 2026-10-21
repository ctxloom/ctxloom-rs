//! Running one command through annals' kernel, and running it again when
//! annals says to.
//!
//! annals never retries; it answers [`Recovery::Retry`] for a store
//! conflict, a stale command or a store that could not say, and a retry is
//! safe because it re-runs the SAME command id in a fresh unit of work: if
//! the first attempt landed after all, the retry is answered DUPLICATE. The
//! waits between attempts are capped exponential backoff with full jitter
//! (each wait drawn from zero to its ceiling), so many contending processes
//! spread out instead of colliding again in lockstep.

use std::time::Duration;

use annals::next::v1::{CommandBook, Uuid};
use annals::next::{
    AsyncStore, Failure, Handled, Kernel, MemoryDeadLetters, Recovery, Registry, Routes, Sampled,
};
use annals_diagnostic::Report;
use ports::{Clock, Delay, Jitter};
use tasks::{TaskRejection, Tasks};

use crate::AppError;

/// How long to wait between attempts, and how many to make.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Backoff {
    /// The ceiling of the first wait; each later one doubles it.
    pub base: Duration,
    /// No wait's ceiling exceeds this.
    pub cap: Duration,
    /// Attempts in all, the first included.
    pub attempts: u32,
}

impl Backoff {
    /// 10ms doubling to a 1s cap, 10 attempts: the waits' ceilings sum to
    /// about 3.3s, on top of the store's own lock wait per attempt.
    pub const DEFAULT: Backoff = Backoff {
        base: Duration::from_millis(10),
        cap: Duration::from_secs(1),
        attempts: 10,
    };

    /// The ceiling of the wait after the `failed`th failed attempt (from
    /// 1): `base * 2^(failed - 1)`, at most `cap`.
    pub fn ceiling(&self, failed: u32) -> Duration {
        let doubling = 1u32
            .checked_shl(failed.saturating_sub(1))
            .unwrap_or(u32::MAX);
        self.base.saturating_mul(doubling).min(self.cap)
    }
}

/// What one command came to, short of an error.
pub(crate) enum Outcome {
    /// It committed, or had already. Which, only the tests read.
    Handled(#[cfg_attr(not(test), expect(dead_code, reason = "read by the tests"))] Handled),
    /// The domain refused it.
    Refused(TaskRejection),
}

/// What [`run`] needs besides the store and the command.
pub(crate) struct Runner<'a, C, D> {
    pub registry: &'a Registry,
    pub clock: &'a mut C,
    pub delay: &'a mut D,
    pub backoff: Backoff,
}

impl<C: Clock, D: Delay + Jitter> Runner<'_, C, D> {
    /// Run `book` under `command_id` on `store`, each attempt in a
    /// fresh unit of work, until it commits, is refused, fails in a way a
    /// retry will not mend, or the attempts run out.
    pub async fn run<S: AsyncStore>(
        &mut self,
        store: &mut S,
        book: &CommandBook,
        command_id: &Uuid,
    ) -> Result<Outcome, AppError> {
        let dead_letters = MemoryDeadLetters::new();
        let kernel = Kernel::new(self.registry, &dead_letters);
        let routes = Routes::new();
        let mut failed = 0;
        loop {
            let sampled = Sampled {
                now: self.clock.now(),
                command_id: command_id.clone(),
            };
            let unit = store.begin().await;
            let failure = match kernel
                .handle_async::<Tasks, _, _>(unit, &routes, book.clone(), &sampled, &mut ())
                .await
            {
                Ok(handled) => return Ok(Outcome::Handled(handled)),
                Err(Failure::Rejected(rejection)) => return Ok(Outcome::Refused(rejection)),
                Err(failure) => failure,
            };
            if failure.recovery() != Recovery::Retry {
                return Err(AppError::Failed(Report::of(&failure)));
            }
            failed += 1;
            if failed >= self.backoff.attempts {
                return Err(AppError::GaveUp {
                    attempts: failed,
                    error: Report::of(&failure),
                });
            }
            let ceiling = self.backoff.ceiling(failed);
            let wait = self.delay.up_to(ceiling).min(ceiling);
            self.delay.sleep(wait).await;
        }
    }
}

#[cfg(test)]
#[path = "retry.test.rs"]
mod tests;
