//! What a project id may be: one clean path segment, since it names a
//! directory under the home.

use crate::RegistryError;

/// `id`, if it is a valid project id: 1 to 255 ASCII letters, digits, '-',
/// '_' or '.', not starting with '.' and holding no "..", as Go's
/// `ValidateProjectID`. `origin` names where it came from, for the error.
pub fn valid(id: &str, origin: &str) -> Result<String, RegistryError> {
    let refuse = |reason: String| RegistryError::InvalidId {
        id: id.to_string(),
        origin: origin.to_string(),
        reason,
    };
    if id.is_empty() {
        return Err(refuse("it is empty".to_string()));
    }
    if id.len() > 255 {
        return Err(refuse(format!("it is {} bytes long", id.len())));
    }
    if let Some(c) = id
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    {
        return Err(refuse(format!("it contains {c:?}")));
    }
    if id.starts_with('.') || id.contains("..") {
        return Err(refuse("it is not a plain path segment".to_string()));
    }
    Ok(id.to_string())
}

#[cfg(test)]
#[path = "id.test.rs"]
mod tests;
