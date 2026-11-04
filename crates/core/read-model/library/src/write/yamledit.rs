//! Edits in place that every document writer here shares: the lines of one
//! key or one item change, and nothing else (comments, order and spacing stay).

use angzarr_yaml::edit::{Document, EditError, EditProblem};
use angzarr_yaml::Value;

/// The strings of the list at `key`; none where it is not a list.
pub(crate) fn list_of(doc: &Document, key: &str) -> Vec<String> {
    doc.value()
        .get(key)
        .and_then(Value::as_seq)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The scalar at `key`; empty where there is none.
pub(crate) fn scalar_of(doc: &Document, key: &str) -> String {
    doc.value()
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Take `key` out of the root mapping. The editor cannot leave a document of
/// no keys; where `key` was the only one, the document becomes empty.
pub(crate) fn drop_key(doc: &mut Document, key: &str) -> Result<(), EditError> {
    match doc.remove_key(&[], key) {
        Err(e) if e.problem == EditProblem::Unfaithful && sole_key(doc, key) => {
            *doc = Document::parse("").map_err(|_| e)?;
            Ok(())
        }
        other => other,
    }
}

fn sole_key(doc: &Document, key: &str) -> bool {
    matches!(doc.value(), Value::Map(entries) if entries.len() == 1 && entries[0].0 == key)
}

/// `key` set to the scalar `value`, or taken out where `value` is empty.
pub(crate) fn put_scalar(doc: &mut Document, key: &str, value: &str) -> Result<(), EditError> {
    if value.is_empty() {
        if doc.value().get(key).is_some() {
            drop_key(doc, key)?;
        }
        return Ok(());
    }
    doc.set(&[], key, &Value::Scalar(value.to_string()), usize::MAX)
}

/// The adds then the removes of one list key; the lines of each change made.
/// An item already there is not added again, one that is not there is not
/// removed; a list left empty takes its key. Only the lines of the items
/// touched change.
pub(crate) fn edit_list(
    doc: &mut Document,
    key: &str,
    add: &[String],
    remove: &[String],
) -> Result<(Vec<String>, Vec<String>), EditError> {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut items = list_of(doc, key);
    // An item both added and removed is added and then removed: if it was
    // there it goes, if it was not there nothing happens.
    for item in add.iter().filter(|item| !remove.contains(item)) {
        if !items.contains(item) {
            doc.push(&[key], &Value::Scalar(item.clone()), usize::MAX)?;
            items.push(item.clone());
            added.push(item.clone());
        }
    }
    for item in remove {
        if let Some(at) = items.iter().position(|i| i == item) {
            doc.remove(&[key], at)?;
            items.remove(at);
            removed.push(item.clone());
        }
    }
    if items.is_empty() && doc.value().get(key).is_some() && !removed.is_empty() {
        drop_key(doc, key)?;
    }
    Ok((added, removed))
}

/// The document's text, with a final line break.
pub(crate) fn text_of(doc: &Document) -> String {
    let mut out = doc.text();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}
