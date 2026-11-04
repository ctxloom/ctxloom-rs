//! `--older-than` and `--purge-older-than`: an offset before now (`30d`,
//! `12w`, `720h`) or a date (`2026-01-01`, UTC midnight), as an instant.

use sessions_application::Timestamp;
use time::{Date, Month, Time};

use crate::ReclaimError;

/// `value`, the argument of `flag`, as an instant relative to `now`.
pub fn parse_age(flag: &str, value: &str, now: Timestamp) -> Result<Timestamp, ReclaimError> {
    let bad = || ReclaimError::BadAge {
        flag: flag.to_string(),
        value: value.to_string(),
    };
    let text = value.trim();
    if let Some(offset) = offset(text) {
        return Ok(Timestamp {
            seconds: now.seconds.saturating_sub(offset),
            nanos: now.nanos,
        });
    }
    let mut parts = text.splitn(3, '-');
    let (y, m, d) = (parts.next(), parts.next(), parts.next());
    let number = |part: Option<&str>| part.and_then(|p| p.parse::<u32>().ok());
    let date = number(y)
        .zip(number(m))
        .zip(number(d))
        .and_then(|((y, m), d)| {
            let month = Month::try_from(u8::try_from(m).ok()?).ok()?;
            Date::from_calendar_date(i32::try_from(y).ok()?, month, u8::try_from(d).ok()?).ok()
        })
        .ok_or_else(bad)?;
    Ok(Timestamp {
        seconds: date.with_time(Time::MIDNIGHT).assume_utc().unix_timestamp(),
        nanos: 0,
    })
}

/// The seconds an offset like `30d` spans.
fn offset(text: &str) -> Option<i64> {
    let unit = text.chars().last()?;
    let seconds = match unit {
        'd' => 86_400,
        'w' => 7 * 86_400,
        'h' => 3_600,
        _ => return None,
    };
    let count: i64 = text[..text.len() - 1].parse().ok().filter(|n| *n >= 0)?;
    count.checked_mul(seconds)
}
