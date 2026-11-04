//! The driven adapter that mints credentials: [`OsTokenSource`] is
//! `run_ports::TokenSource` over the operating system's entropy.  The token FILE (0600, tmpfs for a container) is the launcher's, S6.

use run_ports::{Token, TokenSource};

/// 256 bits from the OS entropy source.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsTokenSource;

impl TokenSource for OsTokenSource {
    fn mint(&self) -> Token {
        let mut bytes = [0u8; 32];
        // No entropy means no credential can be minted: a panic, never a weak token.
        getrandom::fill(&mut bytes).expect("the operating system has no entropy to mint a token");
        Token::from_bytes(bytes)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
