//! The scheduler domain: alarms, as events on a stream of their own.
//!
//! Each scheduler stream belongs to exactly one target stream, rooted
//! `compute_root("scheduler", <the target stream's text>)` ([`stream_for`]).
//! A target's own policy arms and cancels alarms on it with commands
//! ([`ArmAlarm`], [`CancelAlarm`]); a clock loop sends the private fact
//! [`AlarmDue`] under the deterministic id [`alarm_external_id`]; the
//! scheduler records [`AlarmFired`]; and a per-domain delivery policy
//! ([`delivery!`]) hands the target [`AlarmReached`], a fact it re-checks
//! against its own arming.
//!
//! Nothing here knows an application: the crate lifts into angzarr (core; embedded the first user)
//! as it is (scheduler.plan.md section 8).

mod generated {
    include!(concat!(
        env!("OUT_DIR"),
        "/io.angzarr.scheduler.v1.domain.rs"
    ));
}
mod aggregate;
mod delivery;
mod reached;
mod rejection;
mod state;

pub use aggregate::SchedulerAggregate;
pub use delivery::delivered;
pub use generated::*;
pub use reached::AlarmReached;
#[cfg(any(test, feature = "test-support"))]
pub use rejection::diagnostic_samples;
pub use rejection::{SchedulerRejection, EN_US};
pub use state::{apply, instant, AlarmState, SchedulerState, SCHEDULER_DOMAIN};

use angzarr_embedded::v1::{Cover, Uuid};
use angzarr_embedded::{ids, Timestamp};
use time::OffsetDateTime;

/// `domain/root`, then `@edition` outside the main timeline: the text a
/// stream is named by in a root derivation, an external id and an error.
pub fn stream_text(stream: &Stream) -> String {
    text_of(&stream.domain, &stream.root, stream.edition.as_deref())
}

/// [`stream_text`] of a stream's `Cover`.
pub fn cover_text(cover: &Cover) -> String {
    let root = cover.root.as_ref().map(|r| r.value.as_ref());
    text_of(
        &cover.domain,
        root.unwrap_or_default(),
        cover.edition.as_deref(),
    )
}

fn text_of(domain: &str, root: &[u8], edition: Option<&str>) -> String {
    let root = match uuid::Uuid::from_slice(root) {
        Ok(root) => root.to_string(),
        // Not 16 bytes: never a stream, but named so the error can say it.
        Err(_) => root.iter().map(|b| format!("{b:02x}")).collect(),
    };
    match edition.filter(|e| !e.is_empty()) {
        Some(edition) => format!("{domain}/{root}@{edition}"),
        None => format!("{domain}/{root}"),
    }
}

/// The root of the scheduler stream for `target`: `compute_root("scheduler",
/// <the target's text>)`. FIXED once chosen: every stored scheduler stream is
/// rooted by it.
pub fn stream_for(target: &Stream) -> Uuid {
    // "scheduler" holds no ':', the only thing a derivation refuses.
    ids::compute_root(SCHEDULER_DOMAIN, &stream_text(target)).unwrap_or_default()
}

/// `at` as RFC 3339 in UTC with exactly nine fractional digits and a trailing
/// `Z`: fixed width, so `alarm_id@at` is unambiguous even when an alarm id
/// contains `@`.
pub fn rfc3339_nanos(at: &Timestamp) -> String {
    let seconds =
        OffsetDateTime::from_unix_timestamp(at.seconds).unwrap_or(OffsetDateTime::UNIX_EPOCH);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        seconds.year(),
        u8::from(seconds.month()),
        seconds.day(),
        seconds.hour(),
        seconds.minute(),
        seconds.second(),
        at.nanos
    )
}

/// The external id of the clock's [`AlarmDue`] fact: `alarm:<stream>/<alarm_id>@<at>`
/// (scheduler.plan.md OD-1). Deterministic, so a re-sent fact is a duplicate.
pub fn alarm_external_id(target: &Stream, alarm_id: &str, at: &Timestamp) -> String {
    format!(
        "alarm:{}/{alarm_id}@{}",
        stream_text(target),
        rfc3339_nanos(at)
    )
}

/// What [`delivery!`] expands to, named through this crate so the invoking
/// crate depends on none of it directly.
#[doc(hidden)]
pub mod __private {
    pub use angzarr_diagnostic::Report;
    pub use angzarr_embedded::markup::{CommandTarget, Commands, Policy, PolicyContext, TypeName};
    pub use angzarr_embedded::Any;
    pub use scheduler_proto::v1::AlarmFired;
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
