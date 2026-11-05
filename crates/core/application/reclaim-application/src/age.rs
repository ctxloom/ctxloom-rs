//! `--older-than` and `--purge-older-than`: an age before now (`30d`, `1h30m`,
//! `1.5d`) or a date (`2026-01-01`, UTC midnight), as an instant. The grammar
//! is `duration-text`'s; this converts at the edge between the application's
//! [`Timestamp`] and the std instants it speaks. The core's purity rules ban
//! naming `SystemTime` (the clock's type); the instants here are pure values
//! derived from `UNIX_EPOCH` and the caller's `now`, never read from a clock,
//! so they are only inferred, never named.

use std::time::{Duration, UNIX_EPOCH};

use duration_text::{DurationError, DurationText};
use sessions_application::Timestamp;

use crate::ReclaimError;

/// `value`, the argument of `flag`, as an instant relative to `now`.
pub fn parse_age(flag: &str, value: &str, now: Timestamp) -> Result<Timestamp, ReclaimError> {
    let bad = |cause| ReclaimError::BadAge {
        flag: flag.to_string(),
        value: value.to_string(),
        cause,
    };
    let overflow = || DurationError::Overflow {
        text: value.trim().to_string(),
    };
    // `now` as an instant: whole seconds from (or, before the epoch, back
    // from) the epoch, then its nanoseconds forward.
    let whole = Duration::from_secs(now.seconds.unsigned_abs());
    let nanos = u64::try_from(now.nanos).ok().map(Duration::from_nanos);
    let now = nanos
        .zip(if now.seconds >= 0 {
            UNIX_EPOCH.checked_add(whole)
        } else {
            UNIX_EPOCH.checked_sub(whole)
        })
        .and_then(|(nanos, at)| at.checked_add(nanos))
        .ok_or_else(|| bad(overflow()))?;
    let cutoff = DurationText::new(now).cutoff(value).map_err(bad)?;
    // The cutoff as a `Timestamp`: nanoseconds always count forward from
    // whole seconds, as in protobuf, so an instant before the epoch rounds
    // its seconds down.
    let (before, span) = match cutoff.duration_since(UNIX_EPOCH) {
        Ok(after) => (false, after),
        Err(before) => (true, before.duration()),
    };
    let (seconds, nanos) = (
        i64::try_from(span.as_secs()).ok(),
        i32::try_from(span.subsec_nanos()).ok(),
    );
    seconds
        .zip(nanos)
        .map(|(seconds, nanos)| match (before, nanos) {
            (false, _) => (seconds, nanos),
            (true, 0) => (-seconds, 0),
            (true, _) => (-seconds - 1, 1_000_000_000 - nanos),
        })
        .map(|(seconds, nanos)| Timestamp { seconds, nanos })
        .ok_or_else(|| bad(overflow()))
}
