//! Which ids are valid.

use super::valid;
use crate::RegistryError;

fn reason(id: &str) -> String {
    match valid(id, "--project") {
        Err(RegistryError::InvalidId { reason, origin, .. }) => {
            assert_eq!(origin, "--project");
            reason
        }
        other => panic!("{id:?} refused: {other:?}"),
    }
}

#[test]
fn test_a_harp_and_its_kin_are_valid() {
    for id in ["jolly-mute-magma", "a", "A_b.c-9", &"x".repeat(255)] {
        assert_eq!(valid(id, "marker").unwrap(), id);
    }
}

#[test]
fn test_an_empty_id_is_refused() {
    assert_eq!(reason(""), "it is empty");
}

#[test]
fn test_an_id_over_255_bytes_is_refused() {
    assert_eq!(reason(&"x".repeat(256)), "it is 256 bytes long");
}

#[test]
fn test_an_id_with_a_separator_or_other_character_is_refused() {
    assert_eq!(reason("a/b"), "it contains '/'");
    assert_eq!(reason("a b"), "it contains ' '");
    assert_eq!(reason("caf\u{e9}"), "it contains '\u{e9}'");
}

#[test]
fn test_a_dotted_id_that_could_climb_or_hide_is_refused() {
    for id in [".", "..", ".hidden", "a..b"] {
        assert_eq!(reason(id), "it is not a plain path segment", "{id}");
    }
    assert!(valid("a.b", "x").is_ok());
}
