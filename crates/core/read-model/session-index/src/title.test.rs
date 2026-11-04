//! A session's title. Go: `operations.SessionTitle` and
//! `TestViewSession_TitleIsTheFirstUserPrompt` /
//! `TestViewSession_NoUserPromptIsNoTitle` (session_title_test.go),
//! `sessions.FirstLineSummary` (sidecar.go), `textutil.TruncateBytes`.

use super::{first_line_summary, title_of, TITLE_BYTES};

/// One canonical record (docs/transcript.schema.json): an entry of `kind`
/// `entry` with `content`, a main-thread one unless `sidechain`.
fn entry(seq: u32, kind: &str, content: &str, sidechain: bool) -> String {
    let content = serde_json::to_string(content).unwrap();
    let sidechain = if sidechain {
        r#","sidechain":true"#
    } else {
        ""
    };
    format!(
        r#"{{"v":1,"harp":"amber-swift-owl","engine":"claude-code","seq":{seq},"ts":"2026-01-01T00:00:00Z","kind":"entry","entry":{{"type":"{kind}","content":{content}{sidechain}}}}}"#
    )
}

fn transcript(lines: &[String]) -> String {
    lines.join("\n") + "\n"
}

// Go: the title is the first main-thread user prompt: what came before it (an
// assistant greeting) and what a subagent was asked (a sidechain prompt) are
// not the user's words; a blank prompt says nothing; it is cut to its first
// non-empty line and 80 characters.
#[test]
fn the_title_is_the_first_main_thread_user_prompt_cut_to_one_line() {
    let long = "y".repeat(120);
    let text = transcript(&[
        entry(0, "assistant", "hello, how can I help?", false),
        entry(1, "user", "subagent brief", true),
        entry(2, "user", "   ", false),
        entry(
            3,
            "user",
            &format!("\n  fix the flaky purge test {long}\nsecond line"),
            false,
        ),
        entry(4, "user", "a later prompt", false),
    ]);
    let got = title_of(&text).expect("a title");
    assert_eq!(
        got,
        first_line_summary(&format!("fix the flaky purge test {long}"))
    );
    assert!(got.starts_with("fix the flaky purge test "), "{got}");
    assert_eq!(got.len(), TITLE_BYTES);
}

// Go: the fixture's title, read by the listing, the search and `show`
// (session_seed_fixture.go: "Seeded prompt for <harp>.").
#[test]
fn the_seeded_prompt_is_the_title() {
    let text = transcript(&[entry(
        0,
        "user",
        "Seeded prompt for amber-swift-owl.",
        false,
    )]);
    assert_eq!(
        title_of(&text).as_deref(),
        Some("Seeded prompt for amber-swift-owl.")
    );
}

// Go: no canonical transcript, or one with no user prompt in it, is no title.
#[test]
fn no_user_prompt_is_no_title() {
    assert_eq!(title_of(""), None);
    assert_eq!(
        title_of(&transcript(&[entry(0, "assistant", "only me", false)])),
        None
    );
}

// Go: an unreadable transcript has no title (`ParseTranscriptFile` errs and
// SessionTitle answers ""); a record of a version this reader does not know
// "must fail loud, never guess at the shape".
#[test]
fn an_unreadable_transcript_is_no_title() {
    let good = entry(0, "user", "hello", false);
    assert_eq!(title_of(&format!("{good}\nnot json\n")), None);
    let future = good.replace(r#""v":1"#, r#""v":2"#);
    assert_eq!(title_of(&format!("{future}\n")), None);
}

// Go: the other record kinds (session, complete, denied, raw) carry no prompt.
#[test]
fn records_that_are_not_entries_are_skipped() {
    let session = r#"{"v":1,"harp":"h","engine":"claude-code","seq":0,"ts":"2026-01-01T00:00:00Z","kind":"session","session":{"session_id":"s"}}"#;
    let text = format!("{session}\n{}\n", entry(1, "user", "the question", false));
    assert_eq!(title_of(&text).as_deref(), Some("the question"));
}

// Go: FirstLineSummary trims, takes the first line, caps at 80 bytes.
#[test]
fn a_summary_is_the_first_line_trimmed_and_capped() {
    assert_eq!(first_line_summary("  one\ntwo "), "one");
    assert_eq!(first_line_summary("short"), "short");
    assert_eq!(first_line_summary(""), "");
    assert_eq!(first_line_summary(&"a".repeat(100)).len(), 80);
}

// Go: TruncateBytes backs off to the nearest rune boundary so a multibyte
// character is never split.
#[test]
fn a_cap_never_splits_a_character() {
    // 79 ASCII bytes then a 3-byte character straddling the 80-byte cap.
    let text = format!("{}\u{20ac}tail", "a".repeat(79));
    let got = first_line_summary(&text);
    assert_eq!(got, "a".repeat(79));
    assert!(got.len() <= 80);
    // One byte more room and the character fits whole.
    let text = format!("{}\u{20ac}", "a".repeat(77));
    assert_eq!(first_line_summary(&text), text);
}
