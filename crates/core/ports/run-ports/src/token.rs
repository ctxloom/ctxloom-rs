//! The credential: a 256-bit token, and its SHA-256 digest (D29). Only the
//! digest is ever journaled.

use std::fmt;

use angzarr_diagnostic::{code, Arg, Diagnostic};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// The value of one lowercase hexadecimal digit.
fn hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

/// A run's bearer token: 256 random bits, shown to nobody. Its `Debug` is
/// redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct Token([u8; 32]);

impl Token {
    pub fn from_bytes(bytes: [u8; 32]) -> Token {
        Token(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// As 64 lowercase hexadecimal characters: how the 0600 token file holds it.
    pub fn to_text(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The token a text holds, or why it holds none.
    pub fn from_text(text: &str) -> Result<Token, TokenError> {
        let malformed = || TokenError::Malformed {
            length: text.len() as u64,
        };
        let digits = text.as_bytes();
        if digits.len() != 64 {
            return Err(malformed());
        }
        let mut bytes = [0u8; 32];
        for (byte, pair) in bytes.iter_mut().zip(digits.chunks(2)) {
            let high = hex_digit(pair[0]).ok_or_else(malformed)?;
            let low = hex_digit(pair[1]).ok_or_else(malformed)?;
            *byte = high * 16 + low;
        }
        Ok(Token(bytes))
    }

    /// The digest the event holds.
    pub fn hash(&self) -> TokenHash {
        TokenHash(Sha256::digest(self.0).into())
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(redacted)")
    }
}

/// SHA-256 of a token: what a `RunEnqueued` holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TokenHash([u8; 32]);

impl TokenHash {
    /// The digest an event holds, if it holds 32 bytes.
    pub fn from_slice(bytes: &[u8]) -> Option<TokenHash> {
        <[u8; 32]>::try_from(bytes).ok().map(TokenHash)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Whether `token` hashes to this, compared in constant time.
    pub fn matches(&self, token: &Token) -> bool {
        self.0.ct_eq(&token.hash().0).into()
    }
}

/// Where tokens come from: the OS entropy in the `credentials` adapter, a
/// counter in a test.
pub trait TokenSource: Send + Sync {
    fn mint(&self) -> Token;
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TokenError {
    /// A text that is not 64 hexadecimal characters.
    Malformed { length: u64 },
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        angzarr_diagnostic::display(self, f)
    }
}

impl Diagnostic for TokenError {
    fn code(&self) -> &str {
        code!("token-malformed")
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        let TokenError::Malformed { length } = self;
        vec![("length", (*length).into())]
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!("../locales/en-US/messages.ftl"),
);

/// One value of every error, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![Box::new(TokenError::Malformed { length: 3 })]
}

#[cfg(test)]
#[path = "token.test.rs"]
mod tests;
