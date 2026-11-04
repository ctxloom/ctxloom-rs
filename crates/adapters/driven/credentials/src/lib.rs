//! The driven adapter that mints credentials: [`OsTokenSource`] is
//! `run_ports::TokenSource` over the operating system's entropy. RED (S5):
//! a stub. The token FILE (0600, tmpfs for a container) is the launcher's, S6.

use run_ports::{Token, TokenSource};

/// 256 bits from the OS entropy source.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsTokenSource;

impl TokenSource for OsTokenSource {
    fn mint(&self) -> Token {
        todo!("S5 stub")
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
