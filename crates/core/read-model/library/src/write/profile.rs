//! A profile document, made and changed in place (Go's
//! `operations.CreateProfile`/`UpdateProfile`): `profiles/<n>.yaml` in a
//! bundle. A change touches the lines of the keys it changes and no others, so
//! an author's comments and order survive.

use angzarr_yaml::edit::{Document, EditError};
use angzarr_yaml::Value;

use super::section::after_marker;
use super::yamledit::{edit_list, put_scalar, scalar_of, text_of};
use crate::ProfileError;

/// The name the bundle that holds the project's own profiles has.
pub const PROJECT_BUNDLE: &str = "project";

/// `profiles/<name>.yaml`.
pub fn profile_path(name: &str) -> String {
    format!("profiles/{name}.yaml")
}

/// What `profile create` makes of a profile.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct NewProfile {
    pub description: String,
    pub parents: Vec<String>,
    pub llm: String,
    pub bundles: Vec<String>,
}

fn list(items: &[String]) -> Value {
    Value::Seq(items.iter().cloned().map(Value::Scalar).collect())
}

/// The document for `new`: `description`, `parents`, `llm`, `bundles`, the
/// ones it has, in that order.
pub fn new_profile(new: &NewProfile) -> String {
    let mut entries: Vec<(String, Value)> = Vec::new();
    if !new.description.is_empty() {
        entries.push((
            "description".to_string(),
            Value::Scalar(new.description.clone()),
        ));
    }
    if !new.parents.is_empty() {
        entries.push(("parents".to_string(), list(&new.parents)));
    }
    if !new.llm.is_empty() {
        entries.push(("llm".to_string(), Value::Scalar(new.llm.clone())));
    }
    if !new.bundles.is_empty() {
        entries.push(("bundles".to_string(), list(&new.bundles)));
    }
    angzarr_yaml::write(&Value::Map(entries), usize::MAX)
}

/// What `profile modify` changes.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct ProfileEdit {
    pub description: Option<String>,
    pub llm: Option<String>,
    pub add_parents: Vec<String>,
    pub remove_parents: Vec<String>,
    pub add_bundles: Vec<String>,
    pub remove_bundles: Vec<String>,
    pub exclude_fragments: Vec<String>,
    pub include_fragments: Vec<String>,
    pub exclude_mcp: Vec<String>,
    pub include_mcp: Vec<String>,
}

fn bad(detail: String) -> ProfileError {
    ProfileError::BadValue {
        key: "/".to_string(),
        detail,
    }
}

fn unwritable(error: EditError) -> ProfileError {
    bad(format!("cannot be edited in place: {:?}", error.problem))
}

/// One list key edited: the adds then the removes, a line for each real
/// change.
fn edit_one(
    doc: &mut Document,
    key: &str,
    label: &str,
    add: &[String],
    remove: &[String],
    changes: &mut Vec<String>,
) -> Result<(), ProfileError> {
    let (added, removed) = edit_list(doc, key, add, remove).map_err(unwritable)?;
    changes.extend(added.iter().map(|item| format!("added {label}: {item}")));
    changes.extend(
        removed
            .iter()
            .map(|item| format!("removed {label}: {item}")),
    );
    Ok(())
}

/// `text` with `edit` applied, and a line for each change made, in Go's order:
/// the description, the llm, then for each of parents, bundles, excluded
/// fragments and excluded mcp the additions and then the removals. An item
/// already there is not added again; one that is not there is not removed; a
/// restated description is no change. A list left empty takes its key.
pub fn edit_profile(text: &str, edit: &ProfileEdit) -> Result<(String, Vec<String>), ProfileError> {
    let (marker, rest) = after_marker(text);
    let mut doc = Document::parse(rest).map_err(|cause| ProfileError::Syntax { cause })?;
    if !matches!(doc.value(), Value::Map(_) | Value::Null) {
        return Err(bad("a profile is a mapping".to_string()));
    }
    let mut changes = Vec::new();
    if let Some(description) = edit
        .description
        .as_ref()
        .filter(|d| **d != scalar_of(&doc, "description"))
    {
        put_scalar(&mut doc, "description", description).map_err(unwritable)?;
        changes.push("updated description".to_string());
    }
    if let Some(llm) = edit.llm.as_ref().filter(|l| **l != scalar_of(&doc, "llm")) {
        put_scalar(&mut doc, "llm", llm).map_err(unwritable)?;
        changes.push(if llm.is_empty() {
            "cleared llm".to_string()
        } else {
            format!("set llm to \"{llm}\"")
        });
    }
    let lists = [
        ("parents", "parent", &edit.add_parents, &edit.remove_parents),
        ("bundles", "bundle", &edit.add_bundles, &edit.remove_bundles),
        (
            "exclude_fragments",
            "exclude fragment",
            &edit.exclude_fragments,
            &edit.include_fragments,
        ),
        (
            "exclude_mcp",
            "exclude mcp",
            &edit.exclude_mcp,
            &edit.include_mcp,
        ),
    ];
    for (key, label, add, remove) in lists {
        edit_one(&mut doc, key, label, add, remove, &mut changes)?;
    }
    if changes.is_empty() {
        return Ok((text.to_string(), changes));
    }
    Ok((format!("{marker}{}", text_of(&doc)), changes))
}

#[cfg(test)]
#[path = "profile.test.rs"]
mod tests;
