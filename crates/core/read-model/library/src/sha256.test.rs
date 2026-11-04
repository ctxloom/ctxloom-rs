//! The hex form, and the digests Go printed for the `code-quality` fixture's
//! skill files (`golden-corpus`: `skill show`).

use super::*;

#[test]
fn test_hex_is_lowercase_and_zero_padded() {
    assert_eq!(
        hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

// The digests `ctxloom skill show` printed for the fixture skill (fixtures/golden
// lib-skill-show-json): the descriptor and the script.
#[test]
fn test_the_digests_go_printed_for_the_fixture_skill() {
    assert_eq!(
        hex(b"---\nname: reviewer\ndescription: SKILL-MARKER\n---\n\n# reviewer\n\nbody\n"),
        "5a88a1eb69a032b68569b840821888c75ffb2bb0f17ecbfc73296ff320faab17"
    );
    assert_eq!(
        hex(b"#!/bin/sh\necho hi\n"),
        "299001868fb8c02fd431c336c6d058f5558c5dff5b5af5e6fe04b870a6a9cbba"
    );
}
