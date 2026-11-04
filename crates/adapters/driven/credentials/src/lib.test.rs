//! The OS-entropy token source. RED (S5): a stub.

use std::collections::BTreeSet;

use run_ports::TokenSource;

use crate::OsTokenSource;

// NEW-DESIGN (plan 5.3: "the application mints a 256-bit token per run"):
// a thousand mints are a thousand distinct tokens, none all-zero.
#[test]
fn every_mint_is_a_distinct_nonzero_token() {
    let source = OsTokenSource;
    let mut seen = BTreeSet::new();
    for _ in 0..1000 {
        let token = source.mint();
        assert_ne!(token.as_bytes(), &[0u8; 32]);
        assert!(seen.insert(*token.as_bytes()), "a token repeated");
    }
}

// NEW-DESIGN: a minted token survives the 0600 file's text round trip.
#[test]
fn a_minted_token_round_trips_through_its_text() {
    let token = OsTokenSource.mint();
    assert_eq!(run_ports::Token::from_text(&token.to_text()), Ok(token));
}
