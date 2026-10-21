//! Tags as the use cases take them: strings in tagma's grammar,
//! `(namespace:)key(=value)`, parsed into [`Tag`]s at the edge of every
//! write, and admitted only if they could be queried and the project's
//! schema allows them (Go's `operations.validateTag`): a write is refused
//! at the first that is not, with the reason. Only what a write adds is
//! checked; a tag already held, or one being removed, never is.
//!
//! A scalar target's values collapse before the retag is sent
//! ([`collapse`]): decide never reads the schema.

use std::collections::BTreeMap;
use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

pub use tasks::Tag;

use crate::schema::{TagSchema, FACET_NAMESPACE, SEMVER};
use crate::semver::Version;

/// The words tagma's query grammar reads as operators, in any case: a tag
/// spelled as one could never be matched.
pub const OPERATOR_WORDS: [&str; 3] = ["and", "or", "not"];

/// Why a tag given to a write was refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TagRefusal {
    /// Not a tag in tagma's grammar: it could never be queried.
    Unparseable { tag: String, detail: String },
    /// `and`, `or` or `not`, which a query always reads as an operator.
    OperatorWord { tag: String },
    /// In the `tagma` namespace (or under it), which is the schema's.
    ReservedNamespace { tag: String, namespace: String },
    /// A value outside its target's declared enum.
    NotInEnum {
        tag: String,
        value: String,
        target: String,
        members: String,
    },
    /// A value that is not a finite number, where a range is declared.
    NotANumber {
        tag: String,
        value: String,
        target: String,
        min: String,
        max: String,
    },
    /// A number outside its target's declared range.
    OutOfRange {
        tag: String,
        value: String,
        target: String,
        min: String,
        max: String,
    },
    /// A value that is not a SemVer 2.0.0 version, where `type=semver` is
    /// declared.
    NotSemver {
        tag: String,
        value: String,
        target: String,
        detail: String,
    },
}

impl fmt::Display for TagRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TagRefusal::Unparseable { tag, detail } => {
                write!(f, "the tag {tag} is not a tag in tagma's grammar: {detail}")
            }
            TagRefusal::OperatorWord { tag } => write!(
                f,
                "the tag {tag} is a query operator (and, or, not, in any case), so no query could match it"
            ),
            TagRefusal::ReservedNamespace { tag, namespace } => write!(
                f,
                "the tag {tag} is in the namespace {namespace}: tagma's namespace is reserved for the tag schema's declarations"
            ),
            TagRefusal::NotInEnum {
                tag,
                value,
                target,
                members,
            } => write!(
                f,
                "the tag {tag}'s value {value} is not one of {target}'s declared values: {members}"
            ),
            TagRefusal::NotANumber {
                tag,
                value,
                target,
                min,
                max,
            } => write!(
                f,
                "the tag {tag}'s value {value} is not a finite number, which {target}'s declared range [{min}, {max}] requires"
            ),
            TagRefusal::OutOfRange {
                tag,
                value,
                target,
                min,
                max,
            } => write!(
                f,
                "the tag {tag}'s value {value} is outside {target}'s declared range [{min}, {max}]"
            ),
            TagRefusal::NotSemver {
                tag,
                value,
                target,
                detail,
            } => write!(
                f,
                "the tag {tag}'s value {value} is not a SemVer 2.0.0 version, which {target}'s declared type semver requires: {detail}"
            ),
        }
    }
}

impl Diagnostic for TagRefusal {
    fn code(&self) -> &'static str {
        match self {
            TagRefusal::Unparseable { .. } => "tasks-application-tag-unparseable",
            TagRefusal::OperatorWord { .. } => "tasks-application-tag-operator-word",
            TagRefusal::ReservedNamespace { .. } => "tasks-application-tag-reserved-namespace",
            TagRefusal::NotInEnum { .. } => "tasks-application-tag-not-in-enum",
            TagRefusal::NotANumber { .. } => "tasks-application-tag-not-a-number",
            TagRefusal::OutOfRange { .. } => "tasks-application-tag-out-of-range",
            TagRefusal::NotSemver { .. } => "tasks-application-tag-not-semver",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let s = |v: &String| Arg::from(v.as_str());
        match self {
            TagRefusal::Unparseable { tag, detail } => vec![("tag", s(tag)), ("detail", s(detail))],
            TagRefusal::OperatorWord { tag } => vec![("tag", s(tag))],
            TagRefusal::ReservedNamespace { tag, namespace } => {
                vec![("tag", s(tag)), ("namespace", s(namespace))]
            }
            TagRefusal::NotInEnum {
                tag,
                value,
                target,
                members,
            } => vec![
                ("tag", s(tag)),
                ("value", s(value)),
                ("target", s(target)),
                ("members", s(members)),
            ],
            TagRefusal::NotANumber {
                tag,
                value,
                target,
                min,
                max,
            }
            | TagRefusal::OutOfRange {
                tag,
                value,
                target,
                min,
                max,
            } => vec![
                ("tag", s(tag)),
                ("value", s(value)),
                ("target", s(target)),
                ("min", s(min)),
                ("max", s(max)),
            ],
            TagRefusal::NotSemver {
                tag,
                value,
                target,
                detail,
            } => vec![
                ("tag", s(tag)),
                ("value", s(value)),
                ("target", s(target)),
                ("detail", s(detail)),
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

/// Whether `tag` may be written under `schema`: it could be queried, it
/// stays out of tagma's namespace, and its value is one its target's
/// declared enum, range and type allow.
pub fn admit(tag: &Tag, schema: &TagSchema) -> Result<(), TagRefusal> {
    let spelled = || tag.to_string();
    let bare = tag.namespace.is_none() && tag.value.is_none();
    if bare && OPERATOR_WORDS.contains(&tag.key.to_ascii_lowercase().as_str()) {
        return Err(TagRefusal::OperatorWord { tag: spelled() });
    }
    if let Some(namespace) = &tag.namespace {
        let under = namespace
            .strip_prefix(FACET_NAMESPACE)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
        if under {
            return Err(TagRefusal::ReservedNamespace {
                tag: spelled(),
                namespace: namespace.clone(),
            });
        }
    }
    let Some(value) = &tag.value else {
        return Ok(());
    };
    let target = tag.target();
    if let Some(members) = schema.enum_of(&target) {
        if !members.contains(value) {
            return Err(TagRefusal::NotInEnum {
                tag: spelled(),
                value: value.clone(),
                target,
                members: members.join(", "),
            });
        }
    }
    if let Some(range) = schema.range_of(&target) {
        let (min, max) = (range.min.to_string(), range.max.to_string());
        match value.parse::<f64>().ok().filter(|f| f.is_finite()) {
            None => {
                return Err(TagRefusal::NotANumber {
                    tag: spelled(),
                    value: value.clone(),
                    target,
                    min,
                    max,
                })
            }
            Some(n) if n < range.min || n > range.max => {
                return Err(TagRefusal::OutOfRange {
                    tag: spelled(),
                    value: value.clone(),
                    target,
                    min,
                    max,
                })
            }
            Some(_) => {}
        }
    }
    if schema.type_of(&target) == Some(SEMVER) {
        if let Err(detail) = Version::parse(value) {
            return Err(TagRefusal::NotSemver {
                tag: spelled(),
                value: value.clone(),
                target,
                detail,
            });
        }
    }
    Ok(())
}

/// Every tag `spelled` names, parsed and admitted under `schema`, in its
/// order; refused at the first that is not.
pub fn admit_all(spelled: &[String], schema: &TagSchema) -> Result<Vec<Tag>, TagRefusal> {
    let tags = parse_all(spelled)?;
    for tag in &tags {
        admit(tag, schema)?;
    }
    Ok(tags)
}

/// `add`, with every value of a scalar target but its last dropped, and the
/// tags of `held` that a surviving scalar value displaces: a different
/// value of the same target. A scalar target is always namespaced, so a
/// tag with no namespace is never collapsed.
pub fn collapse(schema: &TagSchema, held: &[Tag], add: Vec<Tag>) -> (Vec<Tag>, Vec<Tag>) {
    let scalar = |tag: &Tag| {
        let target = tag.target();
        (tag.namespace.is_some() && schema.is_scalar(&target)).then_some(target)
    };
    let mut last: BTreeMap<String, usize> = BTreeMap::new();
    for (i, tag) in add.iter().enumerate() {
        if let Some(target) = scalar(tag) {
            last.insert(target, i);
        }
    }
    let kept: Vec<Tag> = add
        .into_iter()
        .enumerate()
        .filter(|(i, tag)| scalar(tag).is_none_or(|target| last[&target] == *i))
        .map(|(_, tag)| tag)
        .collect();
    let surviving: BTreeMap<String, &Option<String>> = kept
        .iter()
        .filter_map(|tag| Some((scalar(tag)?, &tag.value)))
        .collect();
    let displaced = held
        .iter()
        .filter(|tag| {
            scalar(tag)
                .and_then(|target| surviving.get(&target).copied())
                .is_some_and(|value| *value != tag.value)
        })
        .cloned()
        .collect();
    (kept, displaced)
}

/// One of every refusal, for the catalog test.
pub(crate) fn samples() -> Vec<TagRefusal> {
    let x = || "x".to_string();
    vec![
        TagRefusal::Unparseable {
            tag: x(),
            detail: x(),
        },
        TagRefusal::OperatorWord { tag: x() },
        TagRefusal::ReservedNamespace {
            tag: x(),
            namespace: x(),
        },
        TagRefusal::NotInEnum {
            tag: x(),
            value: x(),
            target: x(),
            members: x(),
        },
        TagRefusal::NotANumber {
            tag: x(),
            value: x(),
            target: x(),
            min: x(),
            max: x(),
        },
        TagRefusal::OutOfRange {
            tag: x(),
            value: x(),
            target: x(),
            min: x(),
            max: x(),
        },
        TagRefusal::NotSemver {
            tag: x(),
            value: x(),
            target: x(),
            detail: x(),
        },
    ]
}

#[cfg(test)]
#[path = "tags.test.rs"]
mod tests;
