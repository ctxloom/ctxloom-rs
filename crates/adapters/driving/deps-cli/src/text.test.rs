//! The text renderings' helpers.

use super::ellipsize;

#[test]
fn text_within_the_limit_is_whole() {
    assert_eq!(ellipsize("short", 5), "short");
    assert_eq!(ellipsize("", 0), "");
}

#[test]
fn longer_text_is_cut_to_the_limit_with_an_ellipsis() {
    // The mark is three bytes: the cut leaves room for it.
    assert_eq!(ellipsize("abcdefghij", 6), "abc\u{2026}");
}

#[test]
fn a_cut_never_splits_a_character() {
    // Each e-acute is two bytes; byte 3 of "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}" is inside the second one, so the
    // cut backs up to the boundary before it.
    assert_eq!(
        ellipsize("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}", 6),
        "\u{e9}\u{2026}"
    );
    assert_eq!(
        ellipsize("\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}", 7),
        "\u{e9}\u{e9}\u{2026}"
    );
}
