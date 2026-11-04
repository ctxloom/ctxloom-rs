//! An instant as the rows show it: pure calendar arithmetic on a
//! `Timestamp`, UTC (Go shows the machine's zone; the core knows none).

use angzarr_embedded::Timestamp;
use time::format_description::{well_known::Rfc3339, FormatItem};
use time::OffsetDateTime;

/// `at` as a date and time; the epoch for one no calendar holds.
fn datetime(at: Timestamp) -> OffsetDateTime {
    let seconds =
        OffsetDateTime::from_unix_timestamp(at.seconds).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let nanos = u32::try_from(at.nanos).unwrap_or(0);
    seconds.replace_nanosecond(nanos).unwrap_or(seconds)
}

/// `days` whole days before `now`: a retention cutoff.
pub fn days_before(now: Timestamp, days: u64) -> Timestamp {
    let seconds = i64::try_from(days.saturating_mul(86_400)).unwrap_or(i64::MAX);
    Timestamp {
        seconds: now.seconds.saturating_sub(seconds),
        nanos: now.nanos,
    }
}

/// RFC 3339 in UTC, the fraction only where there is one: Go's
/// `time.Time.MarshalJSON` (`2026-07-17T17:27:32Z`, `...32.481Z`).
pub fn rfc3339(at: Timestamp) -> String {
    // A date and time always format as RFC 3339; the empty text is only for
    // the impossible.
    datetime(at).format(&Rfc3339).unwrap_or_default()
}

/// `2006-01-02 15:04:05` in UTC: the compact line the text table and the
/// search haystack show (Go's `sessionTime.String`).
pub fn text(at: Timestamp) -> String {
    let parsed: Result<Vec<FormatItem<'_>>, _> = time::format_description::parse_borrowed::<2>(
        "[year]-[month]-[day] [hour]:[minute]:[second]",
    );
    match parsed {
        Ok(format) => datetime(at).format(&format).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
#[path = "stamp.test.rs"]
mod tests;
