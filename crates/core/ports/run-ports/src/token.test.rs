//! The credential token and its digest (D29). RED (S5): stubs.

use angzarr_diagnostic::Diagnostic;

use crate::{diagnostic_samples, Token, TokenError, TokenHash, EN_US};

const ZEROS: [u8; 32] = [0; 32];

// NEW-DESIGN (plan 5.3: "a 256-bit token ... only its SHA-256 goes in the
// event"). The digest of 32 zero bytes is a published value, so the test
// fixes the algorithm and not just its length.
#[test]
fn a_token_hashes_to_its_sha256() {
    let hash = Token::from_bytes(ZEROS).hash();
    let hex: String = hash.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925"
    );
}

// PORTED-FROM-GO: internal/core/coord/creds_test.go
// TestVerifyToken_ConstantTimeMatch: the right token matches its hash and any
// other does not.
#[test]
fn the_digest_matches_its_token_and_no_other() {
    let token = Token::from_bytes([7; 32]);
    let hash = token.hash();
    assert!(hash.matches(&token));
    assert!(!hash.matches(&Token::from_bytes([8; 32])));
}

// NEW-DESIGN: a digest read back from an event must be 32 bytes.
#[test]
fn a_digest_is_32_bytes_or_nothing() {
    assert!(TokenHash::from_slice(&[0; 32]).is_some());
    assert!(TokenHash::from_slice(&[0; 31]).is_none());
    assert!(TokenHash::from_slice(&[]).is_none());
    let token = Token::from_bytes([9; 32]);
    let hash = token.hash();
    assert_eq!(TokenHash::from_slice(hash.as_bytes()), Some(hash));
}

// NEW-DESIGN (plan 5.3: the token reaches the runner "through a 0600 file"):
// 64 lowercase hex characters, and back.
#[test]
fn a_token_round_trips_through_its_text() {
    let token = Token::from_bytes([0xab; 32]);
    let text = token.to_text();
    assert_eq!(text.len(), 64);
    assert_eq!(text, "ab".repeat(32));
    assert_eq!(Token::from_text(&text), Ok(token));
}

// NEW-DESIGN: a text that is not a token is refused naming its length, and
// the refusal never echoes the text.
#[test]
fn a_malformed_token_text_is_refused_by_its_length() {
    assert_eq!(
        Token::from_text("abc"),
        Err(TokenError::Malformed { length: 3 })
    );
    let not_hex = "zz".repeat(32);
    assert!(Token::from_text(&not_hex).is_err());
}

// NEW-DESIGN (plan 5.3: a raw token must not leak): Debug shows nothing of it.
#[test]
fn a_tokens_debug_is_redacted() {
    let shown = format!("{:?}", Token::from_bytes([0xab; 32]));
    assert!(!shown.contains("ab"), "{shown}");
    assert!(shown.contains("redacted"), "{shown}");
}

#[test]
fn every_diagnostic_has_a_message_using_its_args() {
    let (name, catalog) = EN_US;
    assert_eq!(name, "run-ports");
    for sample in diagnostic_samples() {
        let code = sample.code();
        assert!(code.starts_with("run-ports-"), "{code} is prefixed");
        let line = catalog
            .lines()
            .find(|l| l.starts_with(&format!("{code} =")))
            .unwrap_or_else(|| panic!("{code} has no message"));
        for (arg, _) in sample.args() {
            assert!(line.contains(&format!("${arg}")), "{code} omits {arg}");
        }
    }
}

// NEW-DESIGN: every hexadecimal digit means its value, in either nibble.
#[test]
fn every_hex_digit_is_read_at_its_value() {
    let text = "0123456789abcdef".repeat(4);
    let token = Token::from_text(&text).unwrap();
    let expected: Vec<u8> = (0..32)
        .map(|i| (((2 * i) % 16) as u8) << 4 | ((2 * i + 1) % 16) as u8)
        .collect();
    assert_eq!(token.as_bytes().as_slice(), expected.as_slice());
    assert_eq!(token.to_text(), text);
}

// NEW-DESIGN: an error is its code and its args, never English, and the
// catalog test has something to check.
#[test]
fn an_error_displays_its_code_and_args_and_has_samples() {
    let error = TokenError::Malformed { length: 3 };
    assert_eq!(error.to_string(), "run-ports-token-malformed length=3");
    assert_eq!(error.args().len(), 1);
    assert_eq!(diagnostic_samples().len(), 1);
}
