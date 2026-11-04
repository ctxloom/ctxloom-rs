//! The age bounds. Go: `agebound.Parse` (release/0.7).

use sessions_application::Timestamp;

use crate::{parse_age, ReclaimError};

const NOW: Timestamp = Timestamp {
    seconds: 1_800_000_000,
    nanos: 7,
};

#[test]
fn an_offset_is_that_long_before_now() {
    for (text, seconds) in [
        ("30d", 30 * 86_400),
        ("12w", 12 * 7 * 86_400),
        ("720h", 720 * 3_600),
    ] {
        let at = parse_age("--older-than", text, NOW).unwrap();
        assert_eq!(at.seconds, NOW.seconds - seconds, "{text}");
        assert_eq!(at.nanos, NOW.nanos);
    }
}

#[test]
fn a_date_is_its_midnight_in_utc() {
    let at = parse_age("--purge-older-than", "2026-01-01", NOW).unwrap();
    assert_eq!(at.seconds, 1_767_225_600);
}

#[test]
fn anything_else_is_refused_naming_the_flag() {
    for text in ["", "soon", "30", "-3d", "2026-13-40", "d"] {
        match parse_age("--older-than", text, NOW) {
            Err(ReclaimError::BadAge { flag, value }) => {
                assert_eq!((flag.as_str(), value.as_str()), ("--older-than", text));
            }
            other => panic!("{text}: {other:?}"),
        }
    }
}
