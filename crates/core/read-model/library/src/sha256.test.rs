//! FIPS 180-2 / NIST CAVP known answers, and the digests Go printed for the
//! `code-quality` fixture's skill files (`golden-corpus`: `skill show`).

use super::*;

// NIST FIPS 180-2 appendix B
#[test]
fn test_the_nist_known_answers() {
    assert_eq!(
        hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    let million_a = vec![b'a'; 1_000_000];
    assert_eq!(
        hex(&million_a),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
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
