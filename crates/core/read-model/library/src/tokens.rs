//! The token estimate (Go's `internal/shared/tokens`): one place that knows
//! the heuristic, so the dry-run preview and every other surface agree.

/// The rough bytes per token. BYTES, not characters.
pub const BYTES_PER_TOKEN: usize = 4;

/// A rough token count for `text`, rounded up: any non-empty text is at least
/// one token. Only empty text is zero.
pub fn estimate(text: &str) -> usize {
    text.len().div_ceil(BYTES_PER_TOKEN)
}

#[cfg(test)]
#[path = "tokens.test.rs"]
mod tests;
