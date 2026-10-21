//! Tags as the use cases take them: strings in tagma's grammar,
//! `(namespace:)key(=value)`, parsed into [`Tag`]s at the edge of every
//! write. A tag that does not parse is refused, with the reason.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

pub use tasks::Tag;

/// Why a tag given to a write was refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TagRefusal {
    /// Not a tag in tagma's grammar: it could never be queried.
    Unparseable { tag: String, detail: String },
}

impl fmt::Display for TagRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagRefusal::Unparseable { tag, detail } => {
                write!(f, "the tag {tag} is not a tag in tagma's grammar: {detail}")
            }
        }
    }
}

impl Diagnostic for TagRefusal {
    fn code(&self) -> &'static str {
        match self {
            TagRefusal::Unparseable { .. } => "tasks-application-tag-unparseable",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            TagRefusal::Unparseable { tag, detail } => vec![
                ("tag", tag.as_str().into()),
                ("detail", detail.as_str().into()),
            ],
        }
    }
}

/// `spelled`, trimmed, parsed as tagma parses a tag; `None` when it is
/// empty once trimmed, which is no tag at all.
pub fn parse(spelled: &str) -> Option<Result<Tag, TagRefusal>> {
    let trimmed = spelled.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        tagma_core::Tag::parse(trimmed)
            .map(Tag::from)
            .map_err(|detail| TagRefusal::Unparseable {
                tag: trimmed.to_string(),
                detail,
            }),
    )
}

/// Every tag `spelled` names, in its order, the empty ones left out;
/// refused at the first that does not parse.
pub fn parse_all(spelled: &[String]) -> Result<Vec<Tag>, TagRefusal> {
    spelled.iter().filter_map(|s| parse(s)).collect()
}

#[cfg(test)]
#[path = "tags.test.rs"]
mod tests;
