//! A project's tag schema: declarations, in tagma's own tag syntax, of
//! facts about the tags its tasks carry (Go's `tagschema`).
//!
//! A declaration is `tagma.<facet>:"<target>"=<value>`, e.g.
//! `tagma.arity:"triage:kind"=scalar`: its namespace names the facet, its
//! (quoted) key is the target, compared as a plain string with a tag's
//! `namespace:key` ([`target`]), and its value is the fact. The facets:
//!
//! - `arity`: `scalar` (at most one value per task: a new one displaces the
//!   old); any other value (tagma's `set`) is not scalar. The target must be
//!   `namespace:key`.
//! - `enum`: the values a target admits, comma-separated.
//! - `range`: `min,max`, the numbers a target's value lies within.
//! - `type`: the target's value type; `semver` is checked when written and
//!   ordered by SemVer precedence when queried.
//! - `hide`: `true` or `false`, whether listings show the target's tags.
//!   Any other value is skipped, as tagma skips it.
//! - `priority_fn`, `decay_fn`: the formulas priority is derived from, kept
//!   for when it is.
//!
//! A schema is immutable once parsed, and the default schema
//! ([`TagSchema::default`]) declares nothing. A later declaration of the
//! same facet and target replaces an earlier one, so a project's own
//! declarations, which come after [`DEFAULT_TAG_SCHEMA`], extend it. A
//! malformed declaration is refused, naming it: a schema never runs with a
//! declaration silently dropped. Unlike Go, an enum or range that cannot be
//! read is refused when the schema is parsed, not when a tag first meets
//! it.

use std::collections::BTreeMap;
use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

use crate::Tag;

/// The namespace every declaration lives under (`tagma.<facet>`), and that
/// no task's tag may use.
pub const FACET_NAMESPACE: &str = "tagma";

/// The `type` value whose tags are SemVer 2.0.0 versions.
pub const SEMVER: &str = "semver";

/// The facets a declaration may name, sorted.
pub const FACETS: [&str; 7] = [
    "arity",
    "decay_fn",
    "enum",
    "hide",
    "priority_fn",
    "range",
    "type",
];

/// The triage-classification baseline every project's schema starts from,
/// Go's `DefaultTagSchema`: `triage:level` (1, the worst, to 5),
/// `triage:kind`, `triage:effort` (0 to 5) and `triage:blocks-release` (a
/// SemVer version) scalar, with the vocabularies `taskloom lint` checks and
/// the priority formulas.
pub const DEFAULT_TAG_SCHEMA: [&str; 15] = [
    r#"tagma.arity:"triage:level"=scalar"#,
    r#"tagma.arity:"triage:verdict"=scalar"#,
    r#"tagma.arity:"triage:audited"=scalar"#,
    r#"tagma.arity:"triage:kind"=scalar"#,
    r#"tagma.arity:"triage:effort"=scalar"#,
    r#"tagma.arity:"triage:blocks-release"=scalar"#,
    r#"tagma.type:"triage:blocks-release"=semver"#,
    r#"tagma.enum:"triage:kind"="defect,capability,chore""#,
    r#"tagma.enum:"triage:exposed"="wire,cli,config,on-disk,api""#,
    r#"tagma.enum:"triage:verdict"="holds,phantom,obsolete,partial,unclear""#,
    r#"tagma.range:"triage:level"="1,5""#,
    r#"tagma.range:"triage:audited"="20250101,20991231""#,
    r#"tagma.range:"triage:effort"="0,5""#,
    r#"tagma.decay_fn:"triage:kind"="{{triage:exposed=*}} > 0 ? 1 + {{age_days}}/({{age_days}}+90) : {{triage:blind-gate=*}} > 0 ? 1 + {{age_days}}/({{age_days}}+30) : {{triage:kind=capability}} > 0 ? 0.4 + 0.6 * 0.5 ** ({{age_days}}/120) : {{triage:kind=chore}} > 0 ? 0.5 + 0.5 * 0.5 ** ({{age_days}}/180) : 1""#,
    r#"tagma.priority_fn:"triage:kind"="({{triage:level}} > 0 ? 2 ** (3 - {{triage:level}}) : 0.1) * {{age_factor}} * (1 + {{triage:blocks-release=*}}) / (1 + {{triage:effort}}/2)""#,
];

/// A target's numeric bounds, inclusive.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

/// A parsed tag schema. Declares nothing by default.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct TagSchema {
    scalar: BTreeMap<String, bool>,
    enums: BTreeMap<String, Vec<String>>,
    ranges: BTreeMap<String, Range>,
    types: BTreeMap<String, String>,
    hide: BTreeMap<String, bool>,
    priority_fns: BTreeMap<String, String>,
    decay_fns: BTreeMap<String, String>,
}

impl TagSchema {
    /// The schema `declarations` make, in order.
    pub fn parse<S: AsRef<str>>(
        declarations: impl IntoIterator<Item = S>,
    ) -> Result<TagSchema, SchemaError> {
        let mut schema = TagSchema::default();
        for declaration in declarations {
            schema.declare(declaration.as_ref())?;
        }
        Ok(schema)
    }

    /// [`DEFAULT_TAG_SCHEMA`] extended by `configured`.
    pub fn resolved(configured: &[String]) -> Result<TagSchema, SchemaError> {
        let defaults = DEFAULT_TAG_SCHEMA.iter().copied();
        TagSchema::parse(defaults.chain(configured.iter().map(String::as_str)))
    }

    fn declare(&mut self, declaration: &str) -> Result<(), SchemaError> {
        let invalid = |why| SchemaError {
            declaration: declaration.trim().to_string(),
            why,
        };
        let tag = tagma_core::Tag::parse(declaration.trim())
            .map_err(|detail| invalid(Why::Unparseable { detail }))?;
        let namespace = tag.namespace.ok_or_else(|| invalid(Why::NoNamespace))?;
        let facet = namespace
            .strip_prefix(FACET_NAMESPACE)
            .and_then(|rest| rest.strip_prefix('.'))
            .filter(|facet| !facet.is_empty())
            .ok_or_else(|| {
                invalid(Why::NotAFacet {
                    namespace: namespace.clone(),
                })
            })?
            .to_string();
        let value = tag.value.ok_or_else(|| invalid(Why::NoValue))?;
        let target = tag.key;
        match facet.as_str() {
            "arity" => {
                if !is_namespaced_key(&target) {
                    return Err(invalid(Why::ArityTarget { target }));
                }
                self.scalar.insert(target, value == "scalar");
            }
            "enum" => {
                let members: Vec<String> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                    .map(str::to_string)
                    .collect();
                if members.is_empty() {
                    return Err(invalid(Why::EnumEmpty { value }));
                }
                self.enums.insert(target, members);
            }
            "range" => {
                let range = range(&value).ok_or_else(|| invalid(Why::Range { value }))?;
                self.ranges.insert(target, range);
            }
            "type" => {
                self.types.insert(target, value);
            }
            "hide" => match value.as_str() {
                "true" => {
                    self.hide.insert(target, true);
                }
                "false" => {
                    self.hide.insert(target, false);
                }
                // Skipped, as tagma's own hide facts skip it: hiding only
                // narrows what a listing shows.
                _ => {}
            },
            "priority_fn" => {
                self.priority_fns.insert(target, value);
            }
            "decay_fn" => {
                self.decay_fns.insert(target, value);
            }
            _ => return Err(invalid(Why::UnknownFacet { facet })),
        }
        Ok(())
    }

    /// Whether `target` is declared scalar: at most one value per task.
    pub fn is_scalar(&self, target: &str) -> bool {
        self.scalar.get(target).copied().unwrap_or(false)
    }

    /// Every declared `(target, scalar)` arity, by target: those declared
    /// anything but scalar too.
    pub fn arities(&self) -> impl Iterator<Item = (&str, bool)> {
        self.scalar.iter().map(|(t, s)| (t.as_str(), *s))
    }

    /// The values `target` admits, if declared.
    pub fn enum_of(&self, target: &str) -> Option<&[String]> {
        self.enums.get(target).map(Vec::as_slice)
    }

    /// The bounds of `target`'s value, if declared.
    pub fn range_of(&self, target: &str) -> Option<Range> {
        self.ranges.get(target).copied()
    }

    /// Every declared `(target, members)` enum, by target.
    pub fn enums(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.enums.iter().map(|(t, m)| (t.as_str(), m.as_slice()))
    }

    /// Every declared `(target, range)`, by target.
    pub fn ranges(&self) -> impl Iterator<Item = (&str, Range)> {
        self.ranges.iter().map(|(t, r)| (t.as_str(), *r))
    }

    /// `target`'s declared type, if any.
    pub fn type_of(&self, target: &str) -> Option<&str> {
        self.types.get(target).map(String::as_str)
    }

    /// Every declared `(target, type)`, by target.
    pub fn types(&self) -> impl Iterator<Item = (&str, &str)> {
        self.types.iter().map(|(t, v)| (t.as_str(), v.as_str()))
    }

    /// Every declared `(target, hide)`, by target.
    pub fn hide_facts(&self) -> impl Iterator<Item = (&str, bool)> {
        self.hide.iter().map(|(t, h)| (t.as_str(), *h))
    }

    /// The `priority_fn` formulas, by target.
    pub fn priority_fns(&self) -> &BTreeMap<String, String> {
        &self.priority_fns
    }

    /// The `decay_fn` formulas, by target.
    pub fn decay_fns(&self) -> &BTreeMap<String, String> {
        &self.decay_fns
    }
}

/// Whether `target` is the target of some namespaced tag with no value:
/// the only shape a scalar declaration can constrain.
fn is_namespaced_key(target: &str) -> bool {
    match tagma_core::Tag::parse(target) {
        Ok(tag) => {
            tag.value.is_none() && tag.namespace.is_some() && Tag::from(tag).target() == target
        }
        Err(_) => false,
    }
}

/// `min,max`, both finite, `min` no greater than `max`.
fn range(value: &str) -> Option<Range> {
    let (min, max) = value.split_once(',')?;
    let finite = |s: &str| s.trim().parse::<f64>().ok().filter(|f| f.is_finite());
    let range = Range {
        min: finite(min)?,
        max: finite(max)?,
    };
    (range.min <= range.max).then_some(range)
}

/// A declaration the schema refuses, and why.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SchemaError {
    pub declaration: String,
    pub why: Why,
}

/// Why a declaration is refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Why {
    /// Not a tag in tagma's grammar.
    Unparseable { detail: String },
    /// No namespace, so no facet.
    NoNamespace,
    /// A namespace other than `tagma.<facet>`.
    NotAFacet { namespace: String },
    /// A facet outside [`FACETS`].
    UnknownFacet { facet: String },
    /// No value, so no fact.
    NoValue,
    /// An arity whose target is not `namespace:key`, so no tag can be it.
    ArityTarget { target: String },
    /// An enum with no member.
    EnumEmpty { value: String },
    /// A range that is not `min,max`, both finite, in order.
    Range { value: String },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = &self.declaration;
        match &self.why {
            Why::Unparseable { detail } => {
                write!(f, "tag_schema: {d} is not a tag in tagma's grammar: {detail}")
            }
            Why::NoNamespace => write!(f, "tag_schema: {d} has no namespace: expected tagma.<facet>"),
            Why::NotAFacet { namespace } => write!(
                f,
                "tag_schema: {d}'s namespace {namespace} is not tagma.<facet> (e.g. tagma.arity)"
            ),
            Why::UnknownFacet { facet } => write!(
                f,
                "tag_schema: {d} names the unknown facet {facet} (known: {})",
                FACETS.join(", ")
            ),
            Why::NoValue => write!(f, "tag_schema: {d} has no value"),
            Why::ArityTarget { target } => write!(
                f,
                "tag_schema: {d} targets {target}, but an arity's target must be namespace:key"
            ),
            Why::EnumEmpty { value } => {
                write!(f, "tag_schema: {d} lists no enum member in {value}")
            }
            Why::Range { value } => write!(
                f,
                "tag_schema: {d}'s range {value} is not min,max: two finite numbers, the least first"
            ),
        }
    }
}

impl Diagnostic for SchemaError {
    fn code(&self) -> &'static str {
        match self.why {
            Why::Unparseable { .. } => "tasks-application-schema-unparseable",
            Why::NoNamespace => "tasks-application-schema-no-namespace",
            Why::NotAFacet { .. } => "tasks-application-schema-not-a-facet",
            Why::UnknownFacet { .. } => "tasks-application-schema-unknown-facet",
            Why::NoValue => "tasks-application-schema-no-value",
            Why::ArityTarget { .. } => "tasks-application-schema-arity-target",
            Why::EnumEmpty { .. } => "tasks-application-schema-enum-empty",
            Why::Range { .. } => "tasks-application-schema-range",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let mut args = vec![("declaration", self.declaration.as_str().into())];
        match &self.why {
            Why::Unparseable { detail } => args.push(("detail", detail.as_str().into())),
            Why::NotAFacet { namespace } => args.push(("namespace", namespace.as_str().into())),
            Why::UnknownFacet { facet } => {
                args.push(("facet", facet.as_str().into()));
                args.push(("known", FACETS.join(", ").into()));
            }
            Why::ArityTarget { target } => args.push(("target", target.as_str().into())),
            Why::EnumEmpty { value } | Why::Range { value } => {
                args.push(("value", value.as_str().into()));
            }
            Why::NoNamespace | Why::NoValue => {}
        }
        args
    }
}

/// One of every refusal, for the catalog test.
pub(crate) fn samples() -> Vec<SchemaError> {
    let s = |why| SchemaError {
        declaration: "tagma.x:y=z".to_string(),
        why,
    };
    let x = || "x".to_string();
    vec![
        s(Why::Unparseable { detail: x() }),
        s(Why::NoNamespace),
        s(Why::NotAFacet { namespace: x() }),
        s(Why::UnknownFacet { facet: x() }),
        s(Why::NoValue),
        s(Why::ArityTarget { target: x() }),
        s(Why::EnumEmpty { value: x() }),
        s(Why::Range { value: x() }),
    ]
}

#[cfg(test)]
#[path = "schema.test.rs"]
mod tests;
