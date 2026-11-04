//! Go: `TestSessionTime_TextVsJSON` (session_row_test.go): a standard RFC 3339
//! timestamp for json, a compact second-granularity line for text.

use angzarr_embedded::Timestamp;

use super::{days_before, rfc3339, text};

fn at(seconds: i64, nanos: i32) -> Timestamp {
    Timestamp { seconds, nanos }
}

// 2026-07-17T17:27:32Z.
const JULY_17: i64 = 1_784_309_252;

#[test]
fn json_is_rfc3339_in_utc() {
    assert_eq!(rfc3339(at(JULY_17, 0)), "2026-07-17T17:27:32Z");
    assert_eq!(rfc3339(at(0, 0)), "1970-01-01T00:00:00Z");
}

#[test]
fn json_carries_a_fraction_only_where_there_is_one() {
    assert_eq!(
        rfc3339(at(JULY_17, 481_000_000)),
        "2026-07-17T17:27:32.481Z"
    );
    assert_eq!(rfc3339(at(JULY_17, 1)), "2026-07-17T17:27:32.000000001Z");
}

#[test]
fn text_is_the_compact_second_granularity_line() {
    assert_eq!(text(at(JULY_17, 481_000_000)), "2026-07-17 17:27:32");
}

// Go: the retention cutoff is "now minus the session_transcript_retention age".
#[test]
fn a_cutoff_is_whole_days_before_now() {
    assert_eq!(
        days_before(at(JULY_17, 5), 30),
        at(JULY_17 - 30 * 86_400, 5)
    );
    assert_eq!(days_before(at(JULY_17, 0), 0), at(JULY_17, 0));
}
