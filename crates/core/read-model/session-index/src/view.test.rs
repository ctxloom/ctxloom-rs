//! The listing's row rules. Go: `session_row_test.go`, `session_query_test.go`
//! and `session_state_test.go` (internal/adapters/cli, release/0.7), and
//! session.feature ("The listing takes each session's state from its lock,
//! not its end stamp"; "--state refuses a word that is not a state").

use angzarr_embedded::Timestamp;

use super::{only_state, State, View, NO_SUMMARY, STATES};
use crate::SessionRecord;

fn at(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

// 2026-07-17T12:00:00Z and an hour on.
const STARTED: i64 = 1_784_289_600;

fn view(harp: &str, title: Option<&str>) -> View {
    View {
        record: SessionRecord {
            harp: harp.to_string(),
            started_at: at(STARTED),
            ..SessionRecord::default()
        },
        state: State::Ended,
        title: title.map(str::to_string),
        captured: None,
    }
}

// Go: TestNewSessionRow_SummaryFallbackAndBadges.
#[test]
fn a_summary_without_a_title_is_the_placeholder() {
    assert_eq!(NO_SUMMARY, "(no summary)");
    assert_eq!(view("swift-amber-falcon", None).summary(), "(no summary)");
}

#[test]
fn a_title_is_carried_through_untouched() {
    assert_eq!(view("h", Some("Fixed the bug")).summary(), "Fixed the bug");
}

// Go: the purged and kept marks ride the summary as badges, "on the row itself,
// so they show up in every --format". The emoji are Go's, byte for byte.
#[test]
fn a_purged_session_says_so_in_its_summary() {
    let mut v = view("h", Some("Fixed the bug"));
    v.record.purged_at = Some(at(STARTED));
    assert_eq!(
        v.summary(),
        "Fixed the bug  \u{1f5d1} purged (transcript destroyed)"
    );
}

#[test]
fn a_kept_session_says_so_in_its_summary() {
    let mut v = view("h", Some("Fixed the bug"));
    v.record.kept = true;
    assert_eq!(
        v.summary(),
        "Fixed the bug  \u{1f4cc} kept (transcript kept at any age)"
    );
    v.record.purged_at = Some(at(STARTED));
    assert_eq!(
        v.summary(),
        "Fixed the bug  \u{1f5d1} purged (transcript destroyed)  \u{1f4cc} kept (transcript kept at any age)",
        "purged first, then kept, as Go orders them"
    );
}

// Go: TestNewSessionRow_EndedAt, and session.feature: the lock wins. A live
// session's end is blank rather than a stamp from its previous run.
#[test]
fn a_live_sessions_end_is_blank_and_every_other_state_shows_the_stamp() {
    let mut v = view("h", None);
    assert_eq!(v.end(), None, "no end recorded");
    v.record.ended_at = Some(at(STARTED + 3600));
    v.state = State::Ended;
    assert_eq!(v.end(), Some(at(STARTED + 3600)));
    v.state = State::Unknown;
    assert_eq!(
        v.end(),
        Some(at(STARTED + 3600)),
        "unknown shows its stamp as recorded"
    );
    v.state = State::Live;
    assert_eq!(v.end(), None, "live: the stamp is the LAST run's");
}

// Go: TestSessionMetadataHaystack.
#[test]
fn the_haystack_is_the_harp_the_title_and_the_times_the_row_shows() {
    let mut v = view("swift-amber-falcon", Some("Fixed the harp bind race"));
    v.record.ended_at = Some(at(STARTED + 3600));
    let got = v.haystack();
    assert!(got.contains("swift-amber-falcon"), "{got}");
    assert!(
        got.contains("fixed the harp bind race"),
        "lowercased: {got}"
    );
    assert!(
        got.contains("2026-07-17 12:00:00"),
        "start as the row shows it: {got}"
    );
    assert!(
        got.contains("2026-07-17 13:00:00"),
        "end once the session has one: {got}"
    );
}

// Go: sessionMetadataHaystack "has no end while the session is live", and the
// session_query help text: "A live session's END is blank, so its previous
// run's end stamp is not searched either."
#[test]
fn a_live_sessions_previous_end_is_not_searched() {
    let mut v = view("h", None);
    v.record.ended_at = Some(at(STARTED + 3600));
    v.state = State::Live;
    assert!(!v.haystack().contains("2026-07-17 13:00:00"));
}

// Go: TestAllWordsMatch, TestSessionMatchesQuery_MetadataHit.
#[test]
fn every_word_must_match_case_insensitively() {
    let v = view("swift-amber-falcon", Some("fixed the harp bind race"));
    let words = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(v.matches(&words(&["harp"])));
    assert!(
        v.matches(&words(&["harp", "bind"])),
        "all given words present: a match"
    );
    assert!(v.matches(&words(&["HARP"])), "case-insensitive");
    assert!(
        !v.matches(&words(&["harp", "nonexistent"])),
        "one missing word: no match"
    );
    assert!(v.matches(&[]), "no words required: vacuously a match");
}

// Go: TestSessionMatchesQuery_MetadataOnly: a search reads the row's metadata
// and nothing else.
#[test]
fn a_word_that_is_not_in_the_metadata_does_not_match() {
    let v = view("plump-loose-sash", Some("Session wrap-up"));
    let words = |w: &str| vec![w.to_string()];
    assert!(v.matches(&words("wrap-up")));
    assert!(!v.matches(&words("backoff")));
}

// Go: addSessionStateFlag's words, checkSessionState.
#[test]
fn a_state_is_one_of_live_ended_unknown() {
    assert_eq!(STATES, ["live", "ended", "unknown"]);
    for (word, state) in [
        ("live", State::Live),
        ("ended", State::Ended),
        ("unknown", State::Unknown),
    ] {
        assert_eq!(State::parse(word), Some(state));
        assert_eq!(state.as_str(), word);
    }
    assert_eq!(State::parse("running"), None);
    assert_eq!(State::parse(""), None);
}

// Go: session.feature "--state keeps only the sessions in that state".
#[test]
fn only_state_keeps_the_sessions_in_that_state_in_order() {
    let mut a = view("amber-swift-owl", None);
    a.state = State::Live;
    let mut b = view("brisk-copper-moth", None);
    b.state = State::Ended;
    let mut c = view("calm-dusty-wren", None);
    c.state = State::Unknown;
    let all = vec![a.clone(), b.clone(), c.clone()];
    assert_eq!(only_state(all.clone(), Some(State::Live)), vec![a]);
    assert_eq!(only_state(all.clone(), Some(State::Ended)), vec![b]);
    assert_eq!(only_state(all.clone(), Some(State::Unknown)), vec![c]);
    assert_eq!(
        only_state(all.clone(), None),
        all,
        "no filter keeps them all"
    );
}
