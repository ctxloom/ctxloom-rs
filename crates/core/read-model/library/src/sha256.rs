//! SHA-256, for the digests a skill's files and an assembled context are
//! listed with. Pure: bytes in, bytes out (the `sha2` crate's digest).

use sha2::{Digest, Sha256};

/// The digest of `data`.
pub fn digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// The digest of `data` as lowercase hex.
pub fn hex(data: &[u8]) -> String {
    digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "sha256.test.rs"]
mod tests;
