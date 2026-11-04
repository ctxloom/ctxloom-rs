//! What a harp may be called: Go's `harp.Validate` and `ValidateRename`.

use crate::SessionRejection;

/// The longest name, in characters, a human may rename a session to (Go's
/// `harp.MaxNameLen`).
pub const MAX_NAME_LEN: usize = 24;

/// Whether `name` can be a session's name at all (Go's `harp.Validate`): it
/// becomes a path component, so it is not empty, `.` or `..`, carries no path
/// separator (`/`, `\` or `:`) or control character, and has no whitespace at
/// either end.
pub fn validate(name: &str) -> Result<(), SessionRejection> {
    let invalid = |reason: &str| SessionRejection::HarpInvalid {
        harp: name.to_string(),
        reason: reason.to_string(),
    };
    if name.is_empty() {
        return Err(invalid("empty"));
    }
    if name == "." || name == ".." {
        return Err(invalid("directory"));
    }
    if name.contains(['/', '\\', ':']) {
        return Err(invalid("separator"));
    }
    if name.chars().any(|c| c < '\u{20}' || c == '\u{7f}') {
        return Err(invalid("control"));
    }
    if name.trim() != name {
        return Err(invalid("whitespace"));
    }
    Ok(())
}

/// Whether a human may choose `name` (Go's `harp.ValidateRename`): [`validate`]
/// and at most [`MAX_NAME_LEN`] characters.
pub fn validate_rename(name: &str) -> Result<(), SessionRejection> {
    validate(name)?;
    let length = name.chars().count();
    if length > MAX_NAME_LEN {
        return Err(SessionRejection::HarpTooLong {
            harp: name.to_string(),
            limit: MAX_NAME_LEN as u64,
            length: length as u64,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "harp.test.rs"]
mod tests;
