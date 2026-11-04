//! The retention rule. Go: `retention_test.go` (release/0.7).

use angzarr_embedded::Timestamp;
use sessions::Origin;

use super::{ended, Retained, Retention};
use crate::SessionRecord;

const HOUR: i64 = 3600;

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

/// 2026-09-01T00:00:00Z.
const CUTOFF: i64 = 1_788_220_800;

fn rule() -> Retention {
    Retention {
        cutoff: Some(at(CUTOFF)),
    }
}

fn record(ended_at: Option<i64>, kept: bool, origin: Origin) -> SessionRecord {
    SessionRecord {
        kept,
        ended_at: ended_at.map(at),
        origin: Some(origin),
        ..SessionRecord::default()
    }
}

// Go: TestRetention_ReclaimableOnlyOnceEndedPastTheCutoffAndNotKept.
#[test]
fn a_transcript_is_reclaimable_only_once_ended_past_the_cutoff_and_not_kept() {
    let (before, after) = (CUTOFF - HOUR, CUTOFF + HOUR);
    let r = rule();
    assert_eq!(
        r.retains(
            Some(&record(Some(before), false, Origin::Session)),
            Some(at(before))
        ),
        None,
        "ended before the cutoff: reclaimable"
    );
    assert_eq!(
        r.retains(
            Some(&record(Some(CUTOFF), false, Origin::Session)),
            Some(at(CUTOFF))
        ),
        None,
        "ended exactly at the cutoff: reclaimable"
    );
    assert_eq!(
        r.retains(
            Some(&record(Some(after), false, Origin::Session)),
            Some(at(after))
        ),
        Some(Retained::Recent {
            ended: Some(at(after))
        })
    );
    assert_eq!(
        r.retains(
            Some(&record(Some(before), true, Origin::Session)),
            Some(at(before))
        ),
        Some(Retained::Kept),
        "keep wins over age"
    );
}

// Go: TestRetention_EndIsTheLaterOfRecordedEndAndLastActivity.
#[test]
fn the_end_is_the_later_of_the_recorded_end_and_the_last_activity() {
    let (old, recent) = (CUTOFF - 48 * HOUR, CUTOFF + HOUR);
    let r = rule();
    assert_eq!(
        r.retains(
            Some(&record(Some(old), false, Origin::Session)),
            Some(at(recent))
        ),
        Some(Retained::Recent {
            ended: Some(at(recent))
        }),
        "worked on after its recorded end: it ended no earlier than that"
    );
    assert_eq!(
        r.retains(Some(&record(None, false, Origin::Session)), Some(at(old))),
        None,
        "no recorded end: its last activity is its end"
    );
    assert_eq!(
        r.retains(Some(&record(None, false, Origin::Session)), None),
        Some(Retained::Recent { ended: None }),
        "no recorded end and no activity: nothing proves it ended, so it is kept"
    );
}

// Go: TestRetention_OneShotIsReclaimableAtAnyAgeUnlessKept.
#[test]
fn a_one_shot_is_reclaimable_at_any_age_unless_kept() {
    let recent = CUTOFF + HOUR;
    let r = rule();
    assert_eq!(
        r.retains(
            Some(&record(Some(recent), false, Origin::Oneshot)),
            Some(at(recent))
        ),
        None
    );
    assert_eq!(
        r.retains(
            Some(&record(Some(recent), true, Origin::Oneshot)),
            Some(at(recent))
        ),
        Some(Retained::Kept)
    );
    assert_eq!(
        r.retains(
            Some(&record(Some(recent), false, Origin::Agent)),
            Some(at(recent))
        ),
        Some(Retained::Recent {
            ended: Some(at(recent))
        }),
        "a delegated child's session is a record like a human's"
    );
}

// Go: TestRetention_ZeroCutoffRefuses.
#[test]
fn no_cutoff_is_no_rule() {
    let old = at(1_577_836_800);
    assert_eq!(
        Retention::default().retains(
            Some(&record(Some(1_577_836_800), false, Origin::Session)),
            Some(old)
        ),
        Some(Retained::NoBound)
    );
}

// Go: TestRetention_NilEntryIsJudgedByActivityAlone.
#[test]
fn a_session_with_no_record_is_judged_by_its_activity_alone() {
    let (old, recent) = (CUTOFF - HOUR, CUTOFF + HOUR);
    let r = rule();
    assert_eq!(ended(None, Some(at(old))), Some(at(old)));
    assert_eq!(r.retains(None, Some(at(old))), None);
    assert_eq!(
        r.retains(None, Some(at(recent))),
        Some(Retained::Recent {
            ended: Some(at(recent))
        })
    );
    assert_eq!(
        r.retains(None, None),
        Some(Retained::Recent { ended: None })
    );
}
