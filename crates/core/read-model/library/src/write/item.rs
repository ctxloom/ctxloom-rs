//! The files of a fragment and a command (Go's `internal/adapters/content`
//! `surface_markdown.go` and `bundles/tree_write.go`): `fragments/<n>.md` and
//! `prompts/<n>.md`, each an optional front matter and the body; a distilled
//! form beside it as `<n>.distilled.md`.

use angzarr_yaml::edit::Document;
use angzarr_yaml::Value;

use super::yamledit::drop_key;
use crate::frontmatter::split_front_matter;

/// The directory fragments are in.
pub const FRAGMENTS: &str = "fragments";

/// The directory commands are in (Go's `prompts`).
pub const PROMPTS: &str = "prompts";

/// `<dir>/<name>.md`.
pub fn item_path(dir: &str, name: &str) -> String {
    format!("{dir}/{name}.md")
}

/// `<dir>/<name>.distilled.md`.
pub fn distilled_path(dir: &str, name: &str) -> String {
    format!("{dir}/{name}.distilled.md")
}

/// Whether `name` can be an item's name: one path segment that is not hidden
/// (a dot-prefixed name is a sidecar's), holds no separator and no NUL.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', '\0'])
}

fn scalar(text: &str) -> Value {
    Value::Scalar(text.to_string())
}

/// An item file: the front matter of `entries` when there are any, then `body`.
fn render(entries: Vec<(String, Value)>, body: &str) -> String {
    if entries.is_empty() {
        return body.to_string();
    }
    format!(
        "---\n{}---\n{body}",
        angzarr_yaml::write(&Value::Map(entries), usize::MAX)
    )
}

/// What `fragment create` and `bundle edit --add-fragment` write.
pub fn placeholder_fragment(name: &str) -> String {
    format!("# {name}\n\nAdd content here.")
}

/// What `bundle edit --add-prompt` writes.
pub fn placeholder_prompt(name: &str) -> String {
    render(
        vec![("description".to_string(), scalar(name))],
        "Add prompt content here.",
    )
}

/// The example fragment `bundle create` seeds.
pub fn seed_fragment() -> String {
    render(
        vec![
            ("tags".to_string(), Value::Seq(vec![scalar("example")])),
            ("no_distill".to_string(), scalar("true")),
        ],
        "# Example Fragment\n\nAdd your content here.",
    )
}

/// The example command `bundle create` seeds.
pub fn seed_prompt() -> String {
    render(
        vec![
            ("description".to_string(), scalar("Example prompt")),
            ("tags".to_string(), Value::Seq(vec![scalar("example")])),
            ("no_distill".to_string(), scalar("true")),
        ],
        "Example prompt content. Describe what this prompt does.",
    )
}

/// The body of an item file: what follows its front matter (all of it when it
/// has none). This is what an editor is given.
pub fn body_of(text: &str) -> &str {
    split_front_matter(text).map_or(text, |split| split.body)
}

/// Whether the item's front matter marks it `no_distill: true`.
pub fn marked_no_distill(text: &str) -> bool {
    split_front_matter(text)
        .and_then(|split| angzarr_yaml::parse(split.meta).ok())
        .and_then(|meta| {
            meta.get("no_distill")
                .and_then(Value::as_str)
                .map(|v| v == "true")
        })
        .unwrap_or(false)
}

/// The front matter without its `content_hash`: none when no key is left.
fn without_hash(meta: &str) -> Option<String> {
    let Ok(mut doc) = Document::parse(meta) else {
        return Some(meta.to_string());
    };
    if doc.value().get("content_hash").is_some() && drop_key(&mut doc, "content_hash").is_err() {
        return Some(meta.to_string());
    }
    match doc.value() {
        Value::Map(entries) if !entries.is_empty() => Some(doc.text()),
        _ => None,
    }
}

/// The item file `text` with its body replaced by `body`: the front matter
/// stays as written, minus the hash of the content it described
/// (`content_hash`), and goes altogether when nothing else is left in it.
pub fn replace_body(text: &str, body: &str) -> String {
    let Some(split) = split_front_matter(text) else {
        return body.to_string();
    };
    match without_hash(split.meta) {
        Some(meta) => format!("---\n{meta}---\n{body}"),
        None => body.to_string(),
    }
}

#[cfg(test)]
#[path = "item.test.rs"]
mod tests;
