//! Ported from Go's `internal/shared/tokens/tokens_test.go` (860336c4e).

use super::*;

// TestEstimate
#[test]
fn test_estimate() {
    for (text, want) in [("", 0), ("abcd", 1), ("abcdefghi", 3)] {
        assert_eq!(estimate(text), want, "{text:?}");
    }
}

// TestEstimate_CountsBytesNotCharacters: four CJK characters are twelve bytes
// in UTF-8: three tokens on the byte reading, one on the character reading.
#[test]
fn test_estimate_counts_bytes_not_characters() {
    let four_cjk = "\u{4e16}\u{754c}\u{4e4b}\u{5fc3}";
    assert_eq!(four_cjk.len(), 12);
    assert_eq!(estimate(four_cjk), 12 / BYTES_PER_TOKEN);
}

// TestEstimate_NonEmptyTextIsNeverZeroTokens
#[test]
fn test_non_empty_text_is_never_zero_tokens() {
    assert_eq!(
        estimate(""),
        0,
        "empty text is the one case that may be zero"
    );
    for text in ["a", "ab", "abc", "abcd", "\u{4e16}"] {
        assert!(estimate(text) >= 1, "{text:?}");
    }
}
