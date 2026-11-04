//! A config write is a section merge with sorted keys, and never heals a
//! corrupt file (plan §10 S2; Go's `config_save.go` `saveLocked`).
//!
//! The section a command owns (`agents`, `llm`, `companions`, `default_agent`)
//! is replaced whole with the keys of its mappings sorted; every other key,
//! comment and the order of the file stay exactly as the file has them. A text
//! that does not parse is refused, never rewritten: "I could not read what is
//! there" is not "there is nothing there".

use angzarr_yaml::edit::{Document, EditError, EditProblem};
use angzarr_yaml::Value;

use crate::ConfigError;

/// `value` with the keys of every mapping in it sorted (by bytes); lists keep
/// their order.
pub fn sorted(value: &Value) -> Value {
    match value {
        Value::Map(entries) => {
            let mut out: Vec<(String, Value)> = entries
                .iter()
                .map(|(key, inner)| (key.clone(), sorted(inner)))
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(out)
        }
        Value::Seq(items) => Value::Seq(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// A fresh document: `doc`'s mappings sorted all the way down, written in
/// block style.
pub fn render_sorted(doc: &Value) -> String {
    angzarr_yaml::write(&sorted(doc), usize::MAX)
}

/// The text split after a leading document marker (`---`, the one marker the
/// reader takes in stride), so the marker stays as written.
pub(crate) fn after_marker(text: &str) -> (&str, &str) {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let meaningful = line.trim();
        if meaningful.is_empty() || meaningful.starts_with('#') {
            offset += line.len();
            continue;
        }
        if line.trim_end() == "---" {
            return text.split_at(offset + line.len());
        }
        break;
    }
    ("", text)
}

/// The text read as a document: the root's value, and the editable document of
/// the text past its marker.
fn read(existing: &str) -> Result<(String, Value, Document), ConfigError> {
    let (marker, rest) = after_marker(existing);
    let root = angzarr_yaml::parse(rest).map_err(|cause| ConfigError::Syntax { cause })?;
    if !matches!(root, Value::Null | Value::Map(_)) {
        return Err(ConfigError::NotAMapping);
    }
    let doc = Document::parse(rest).map_err(|cause| ConfigError::Syntax { cause })?;
    Ok((marker.to_string(), root, doc))
}

fn unwritable(error: &EditError) -> ConfigError {
    match error.problem {
        EditProblem::NotAMapping => ConfigError::NotAMapping,
        _ => ConfigError::Unwritable {
            detail: format!("{:?}", error.problem),
        },
    }
}

/// The value of the top-level `key` of `existing`, if it is there. Refused as
/// [`set_section`] is.
pub fn section(existing: &str, key: &str) -> Result<Option<Value>, ConfigError> {
    let (_, root, _) = read(existing)?;
    Ok(root.get(key).cloned())
}

/// `existing` stamped with the generation this build writes where it holds no
/// key at all (a file that is not there, blank, or only comments); as it is
/// where it holds any. A config with no stamp is not read, so a first write
/// makes one that is.
pub fn stamped(existing: &str) -> Result<String, ConfigError> {
    let (_, root, _) = read(existing)?;
    if root != Value::Null {
        return Ok(existing.to_string());
    }
    set_section(
        existing,
        "schema_version",
        &Value::Scalar(crate::config::CURRENT.to_string()),
    )
}

/// `existing` with the top-level `key` set to `value` (its mappings sorted):
/// replaced in place where it is, appended where it is not. Only the entry's
/// own lines change, and none when the section already holds `value`. Refused
/// if `existing` does not parse, or its root is not a mapping.
pub fn set_section(existing: &str, key: &str, value: &Value) -> Result<String, ConfigError> {
    let value = sorted(value);
    let (marker, root, mut doc) = read(existing)?;
    if root == Value::Null {
        let mut text = existing.to_string();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&angzarr_yaml::write(
            &Value::Map(vec![(key.to_string(), value)]),
            usize::MAX,
        ));
        return Ok(text);
    }
    if root.get(key).map(sorted).as_ref() == Some(&value) {
        return Ok(existing.to_string());
    }
    doc.set(&[], key, &value, usize::MAX)
        .map_err(|e| unwritable(&e))?;
    let mut text = format!("{marker}{}", doc.text());
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// `existing` without the top-level `key` (and the comments directly above
/// it); unchanged where it is not there. Refused as [`set_section`] is.
pub fn remove_section(existing: &str, key: &str) -> Result<String, ConfigError> {
    let (marker, root, mut doc) = read(existing)?;
    if root.get(key).is_none() {
        return Ok(existing.to_string());
    }
    doc.remove_key(&[], key).map_err(|e| unwritable(&e))?;
    Ok(format!("{marker}{}", doc.text()))
}

#[cfg(test)]
#[path = "section.test.rs"]
mod tests;
