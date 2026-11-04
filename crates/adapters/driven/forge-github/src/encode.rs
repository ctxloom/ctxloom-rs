//! What a request carries that std does not encode: base64 (a blob's content)
//! and a percent-encoded path segment.

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// `bytes` as standard base64 with padding (RFC 4648).
pub fn base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Every byte but the unreserved set and `/`.
const PATH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

/// `path` with every character outside the unreserved set and `/` percent-
/// encoded, so a name cannot end the path or start a query.
pub fn path(path: &str) -> String {
    utf8_percent_encode(path, PATH).to_string()
}

#[cfg(test)]
#[path = "encode.test.rs"]
mod tests;
