//! A bundle's envelope, `bundle.yaml` (Go's `bundles.TreeEnvelope`): the
//! bundle's own metadata, stamped with the format generation a writer emits.

use angzarr_yaml::edit::Document;
use angzarr_yaml::Value;

use super::section::after_marker;
use super::yamledit::{edit_list, put_scalar, scalar_of, text_of};
use crate::BundleError;

/// The generation an envelope is written in.
pub const GENERATION: u32 = 4;

/// What `bundle edit` changes of the envelope.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct EnvelopeEdit {
    pub description: Option<String>,
    pub version: Option<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
}

/// A new bundle's envelope: the generation, the version, and the description
/// when there is one.
pub fn new_envelope(version: &str, description: &str) -> String {
    let mut entries = vec![
        (
            "schema_version".to_string(),
            Value::Scalar(GENERATION.to_string()),
        ),
        ("version".to_string(), Value::Scalar(version.to_string())),
    ];
    if !description.is_empty() {
        entries.push((
            "description".to_string(),
            Value::Scalar(description.to_string()),
        ));
    }
    angzarr_yaml::write(&Value::Map(entries), usize::MAX)
}

fn unwritable(error: angzarr_yaml::edit::EditError) -> BundleError {
    BundleError::BadValue {
        path: "bundle.yaml".to_string(),
        detail: format!("cannot be edited in place: {:?}", error.problem),
    }
}

/// `text` with `edit` applied, and a line for each change made, in Go's order
/// (the description, the version, the tags added then removed). What restates
/// the held value is no change. Where nothing changed the text is returned as
/// it was. Every other key, the comments and the order stay as written; a key
/// that is not there is added after the others.
pub fn edit_envelope(
    text: &str,
    edit: &EnvelopeEdit,
) -> Result<(String, Vec<String>), BundleError> {
    let (marker, rest) = after_marker(text);
    let mut doc = Document::parse(rest).map_err(|cause| BundleError::Syntax { cause })?;
    let mut changes = Vec::new();
    if let Some(description) = edit
        .description
        .as_deref()
        .filter(|d| *d != scalar_of(&doc, "description"))
    {
        put_scalar(&mut doc, "description", description).map_err(unwritable)?;
        changes.push("updated description".to_string());
    }
    if let Some(version) = edit
        .version
        .as_deref()
        .filter(|v| *v != scalar_of(&doc, "version"))
    {
        put_scalar(&mut doc, "version", version).map_err(unwritable)?;
        changes.push("updated version".to_string());
    }
    let (added, removed) =
        edit_list(&mut doc, "tags", &edit.add_tags, &edit.remove_tags).map_err(unwritable)?;
    changes.extend(added.iter().map(|tag| format!("added tag: {tag}")));
    changes.extend(removed.iter().map(|tag| format!("removed tag: {tag}")));
    if changes.is_empty() {
        return Ok((text.to_string(), changes));
    }
    Ok((format!("{marker}{}", text_of(&doc)), changes))
}

#[cfg(test)]
#[path = "envelope.test.rs"]
mod tests;
