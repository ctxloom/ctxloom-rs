//! Ported from Go's `internal/core/displaysafe/displaysafe_test.go` and
//! `internal/shared/textutil` (860336c4e). The markers are U+27E8 `ESC`
//! U+27E9 and U+27E8 `U+XXXX` U+27E9, written as escapes here.

use super::*;

/// `⟨`, `⟩` and the markers.
fn m(code: &str) -> String {
    format!("\u{27e8}{code}\u{27e9}")
}

// displaysafe_test.go: TestText_Goldens
#[test]
fn test_every_dangerous_character_comes_out_as_a_visible_marker() {
    let cases: Vec<(&str, String, String)> = vec![
        (
            "plain text is untouched",
            "rm -rf build/ && make test".into(),
            "rm -rf build/ && make test".into(),
        ),
        (
            "a newline is a line break, kept",
            "a\nb".into(),
            "a\nb".into(),
        ),
        ("CRLF is one line break", "a\r\nb".into(), "a\nb".into()),
        (
            "a lone CR could overwrite the line: visible",
            "rm -rf /\rls".into(),
            format!("rm -rf /{}ls", m("U+000D")),
        ),
        ("a tab expands to spaces", "a\tb".into(), "a    b".into()),
        (
            "CSI clear screen",
            "echo\u{1b}[2Jhi".into(),
            format!("echo{}[2Jhi", m("ESC")),
        ),
        (
            "SGR colour",
            "\u{1b}[31mred\u{1b}[0m".into(),
            format!("{}[31mred{}[0m", m("ESC"), m("ESC")),
        ),
        (
            "OSC title terminated by BEL",
            "\u{1b}]0;pwned\u{7}".into(),
            format!("{}]0;pwned{}", m("ESC"), m("U+0007")),
        ),
        (
            "backspace erases on a terminal",
            "safe\u{8}\u{8}evil".into(),
            format!("safe{}{}evil", m("U+0008"), m("U+0008")),
        ),
        (
            "NUL and DEL",
            "a\u{0}b\u{7f}c".into(),
            format!("a{}b{}c", m("U+0000"), m("U+007F")),
        ),
        (
            "C1 CSI (U+009B) acts as ESC [ on some terminals",
            "a\u{9b}2Jb".into(),
            format!("a{}2Jb", m("U+009B")),
        ),
        ("C1 NEL", "a\u{85}b".into(), format!("a{}b", m("U+0085"))),
    ];
    for (what, input, want) in cases {
        assert_eq!(display_safe(&input, true), want, "{what}");
    }
}

// displaysafe_test.go: TestText_Goldens (bidi, zero-width, separators, tags)
#[test]
fn test_invisible_and_reordering_characters_come_out_as_markers() {
    let cases: Vec<(&str, String, String)> = vec![
        (
            "bidi override reorders what is shown",
            "ls \u{202e}gnp.exe".into(),
            format!("ls {}gnp.exe", m("U+202E")),
        ),
        (
            "bidi isolates",
            "a\u{2066}b\u{2069}c".into(),
            format!("a{}b{}c", m("U+2066"), m("U+2069")),
        ),
        (
            "LRM/RLM/ALM",
            "\u{200e}\u{200f}\u{61c}".into(),
            format!("{}{}{}", m("U+200E"), m("U+200F"), m("U+061C")),
        ),
        (
            "zero-width space hides an argument boundary",
            "rm\u{200b} -rf".into(),
            format!("rm{} -rf", m("U+200B")),
        ),
        (
            "ZWNJ, ZWJ, word joiner, BOM",
            "\u{200c}\u{200d}\u{2060}\u{feff}".into(),
            format!(
                "{}{}{}{}",
                m("U+200C"),
                m("U+200D"),
                m("U+2060"),
                m("U+FEFF")
            ),
        ),
        (
            "soft hyphen",
            "a\u{ad}b".into(),
            format!("a{}b", m("U+00AD")),
        ),
        (
            "tag characters smuggle invisible text",
            "hi\u{e0041}\u{e0042}".into(),
            format!("hi{}{}", m("U+E0041"), m("U+E0042")),
        ),
        (
            "line and paragraph separators",
            "a\u{2028}b\u{2029}c".into(),
            format!("a{}b{}c", m("U+2028"), m("U+2029")),
        ),
        (
            "wide and accented text is untouched",
            "\u{65e5}\u{672c}\u{8a9e} caf\u{e9}".into(),
            "\u{65e5}\u{672c}\u{8a9e} caf\u{e9}".into(),
        ),
    ];
    for (what, input, want) in cases {
        assert_eq!(display_safe(&input, true), want, "{what}");
    }
}

// displaysafe_test.go: TestText_OneLine
#[test]
fn test_without_line_breaks_the_text_cannot_leave_its_line() {
    assert_eq!(display_safe("a\nb", false), format!("a{}b", m("U+000A")));
    assert_eq!(
        display_safe("a\r\nb", false),
        format!("a{}{}b", m("U+000D"), m("U+000A"))
    );
    assert_eq!(display_safe("a\tb", false), "a    b");
    assert_eq!(display_safe("a\u{1b}b", false), format!("a{}b", m("ESC")));
    assert_eq!(display_safe("make test", false), "make test");
}

// textutil.go Ellipsize: a field is cut on a rune boundary with an ellipsis
#[test]
fn test_a_field_is_cut_with_an_ellipsis_on_a_rune_boundary() {
    assert_eq!(field("short", 256), "short");
    let long = "x".repeat(300);
    let cut = field(&long, 256);
    assert!(cut.len() <= 256, "{}", cut.len());
    assert!(cut.ends_with('\u{2026}'));
    let cjk = "\u{4e16}".repeat(100);
    let cut = field(&cjk, 20);
    assert!(cut.len() <= 20);
    assert!(cut.ends_with('\u{2026}'));
}

// bundle_list_view.go itemPreview: the first line, trimmed, cut to 70 bytes
#[test]
fn test_a_preview_is_the_first_line_trimmed_and_cut() {
    assert_eq!(
        preview("\n\n# Example Fragment\n\nAdd your content here."),
        "# Example Fragment"
    );
    assert_eq!(preview("  first  \nsecond"), "first");
    assert_eq!(preview(""), "");
    let long = format!("{}\nrest", "y".repeat(100));
    let cut = preview(&long);
    assert!(cut.len() <= 70, "{}", cut.len());
    assert!(cut.ends_with('\u{2026}'));
}

// inert_render.go bodyRenderer.Render / inertNotice: each line behind the
// indent, trailing line breaks dropped, runs of blank lines collapsed when
// asked, and a notice for what was altered.
#[test]
fn test_a_body_is_indented_collapsed_and_noted() {
    let plain = body("  ", "", true, "ref", "one\ntwo\n\n");
    assert_eq!(plain.text, "  one\n  two\n");
    assert!(plain.notice.is_empty());
    let blank = body("", "(none)", false, "ref", "\n\n");
    assert_eq!(blank.text, "(none)\n");
    let collapsed = body("", "", true, "demo#x", "a\n\n\n\n\n\n\nb");
    assert_eq!(collapsed.text, "a\n\n\n\nb\n");
    assert_eq!(
        collapsed.notice,
        "demo#x is publisher-authored and was rendered inert for this terminal (3 blank line(s) collapsed); use --format json for the raw bytes"
    );
    let loud = body("", "", false, "r", "x\u{1b}[2Jy");
    assert!(loud.text.contains("\u{27e8}ESC\u{27e9}"));
    assert!(loud.notice.contains("1 control character(s) escaped"));
    let huge = body("", "", false, "r", &"z".repeat((256 << 10) + 10));
    assert!(huge.notice.contains("truncated to 262144 of 262154 bytes"));
}

// inert_render.go renderBody: the count of escaped characters is what the
// text gained, so a bracket the author wrote is not counted.
#[test]
fn test_a_bracket_the_author_wrote_is_not_counted_as_escaped() {
    let rendered = body("", "", false, "ref", "\u{27e8}ok\u{27e9} and \u{1b}");
    assert!(rendered.notice.contains("(1 control character(s) escaped)"));
    let plain = body("", "", false, "ref", "\u{27e8}ok\u{27e9}");
    assert!(plain.notice.is_empty());
}

// inert_render.go: a body is cut at 256 KiB and says so; one of exactly that
// size is whole.
#[test]
fn test_a_body_is_cut_only_beyond_its_limit() {
    let exact = "a".repeat(256 << 10);
    let whole = body("", "", false, "ref", &exact);
    assert!(whole.notice.is_empty());
    assert_eq!(whole.text.len(), (256 << 10) + 1);
    let over = body("", "", false, "ref", &format!("{exact}b"));
    assert!(over.notice.contains("truncated to 262144 of 262145 bytes"));
    assert_eq!(over.text.len(), (256 << 10) + 1);
}
