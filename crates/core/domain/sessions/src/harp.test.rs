//! What a harp may be called. Go: `internal/shared/harp` (`Validate`,
//! `ValidateRename`, `MaxNameLen`), release/0.7.

use crate::{validate, validate_rename, SessionRejection, MAX_NAME_LEN};

fn invalid(harp: &str, reason: &str) -> SessionRejection {
    SessionRejection::HarpInvalid {
        harp: harp.to_string(),
        reason: reason.to_string(),
    }
}

// Go: Validate rejects only what cannot be a path component: it is
// "deliberately permissive on charset".
#[test]
fn validate_refuses_what_cannot_be_a_path_component() {
    assert_eq!(validate(""), Err(invalid("", "empty")));
    assert_eq!(validate("."), Err(invalid(".", "directory")));
    assert_eq!(validate(".."), Err(invalid("..", "directory")));
    for bad in ["a/b", "a\\b", "a:b"] {
        assert_eq!(validate(bad), Err(invalid(bad, "separator")), "{bad:?}");
    }
    for bad in ["nul\0name", "tab\tname", "del\x7fname", "line\nbreak"] {
        assert_eq!(validate(bad), Err(invalid(bad, "control")), "{bad:?}");
    }
    for bad in [" lead", "trail ", "\u{a0}x"] {
        // U+00A0 is whitespace to Go's TrimSpace.
        assert_eq!(validate(bad), Err(invalid(bad, "whitespace")), "{bad:?}");
    }
}

#[test]
fn validate_accepts_harps_and_the_odd_name_a_person_might_choose() {
    for good in [
        "amber-swift-owl",
        "x",
        "\u{dc}n\u{ef}code-n\u{e4}me",
        "with space inside",
        "dots.ok",
    ] {
        assert_eq!(validate(good), Ok(()), "{good:?}");
    }
}

// Go: ValidateRename adds the bound, in runes; "a name recorded before the
// bound existed must still resolve", so Validate has none.
#[test]
fn validate_rename_bounds_the_length_in_characters_and_validate_does_not() {
    assert_eq!(MAX_NAME_LEN, 24);
    let exactly = "\u{e9}".repeat(24);
    assert_eq!(validate_rename(&exactly), Ok(()));
    let over = "\u{e9}".repeat(25);
    assert_eq!(
        validate_rename(&over),
        Err(SessionRejection::HarpTooLong {
            harp: over.clone(),
            limit: 24,
            length: 25,
        })
    );
    assert_eq!(validate(&over), Ok(()));
    assert_eq!(validate_rename("a/b"), Err(invalid("a/b", "separator")));
}
