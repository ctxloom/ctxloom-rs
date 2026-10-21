//! Lint: a read-time sweep of every task's tags against the tag schema
//! (Go's `internal/shared/tasks/lint`). A write is already refused a value
//! outside a declared enum or range and collapses a scalar target, so a
//! violation here is data that did not come in through a write: an
//! import, or a schema changed since the tag was written. Lint never
//! blocks a write.
//!
//! It checks, for every task whatever its status: a value outside its
//! target's declared enum; a value on a range-declared target that is not
//! a finite number, or lies outside the range; a scalar target holding
//! more than one value. And, against the schema itself, a value-qualified
//! placeholder in a `priority_fn` or `decay_fn` (`{{triage:kind=foo}}`)
//! naming a value its target's enum does not declare. Go also reports a
//! stored tag that does not parse; here every stored tag is a parsed one,
//! so none can.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

use crate::schema::{self, TagSchema};
use crate::view::Row;
use task_priority::placeholders;

/// The subject of a violation in the schema rather than in a task (Go's
/// `SchemaViolationHarpID`), sorting before every harp id.
pub const SCHEMA_SUBJECT: &str = "(schema)";

/// What a violation is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Finding {
    /// A value outside its target's declared enum.
    NotInEnum {
        target: String,
        value: String,
        members: String,
    },
    /// A value on a range-declared target that is not a finite number.
    NotANumber { target: String, value: String },
    /// A number outside its target's declared range.
    OutOfRange {
        target: String,
        value: String,
        min: String,
        max: String,
    },
    /// A scalar target holding more than one value on one task.
    Scalar { target: String, values: Vec<String> },
    /// A formula's placeholder naming a value its target's enum does not
    /// declare.
    FormulaEnum {
        facet: String,
        target: String,
        placeholder: String,
        reference: String,
        value: String,
        members: String,
    },
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::NotInEnum {
                target,
                value,
                members,
            } => write!(
                f,
                "{target}={value} is not one of the declared enum values: {members}"
            ),
            Finding::NotANumber { target, value } => {
                write!(f, "{target}={value} does not parse as a finite number")
            }
            Finding::OutOfRange {
                target,
                value,
                min,
                max,
            } => write!(
                f,
                "{target}={value} is outside the declared range [{min}, {max}]"
            ),
            Finding::Scalar { target, values } => write!(
                f,
                "{target} carries {} distinct values ({}) but is declared arity=scalar",
                values.len(),
                values.join(", ")
            ),
            Finding::FormulaEnum {
                facet,
                target,
                placeholder,
                reference,
                value,
                members,
            } => write!(
                f,
                "{facet} on {target} references the placeholder {placeholder}, but {value} is not one of {reference}'s declared enum values: {members}"
            ),
        }
    }
}

impl Diagnostic for Finding {
    fn code(&self) -> &'static str {
        match self {
            Finding::NotInEnum { .. } => "tasks-application-lint-not-in-enum",
            Finding::NotANumber { .. } => "tasks-application-lint-not-a-number",
            Finding::OutOfRange { .. } => "tasks-application-lint-out-of-range",
            Finding::Scalar { .. } => "tasks-application-lint-scalar",
            Finding::FormulaEnum { .. } => "tasks-application-lint-formula-enum",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let s = |v: &String| Arg::from(v.as_str());
        match self {
            Finding::NotInEnum {
                target,
                value,
                members,
            } => vec![
                ("target", s(target)),
                ("value", s(value)),
                ("members", s(members)),
            ],
            Finding::NotANumber { target, value } => {
                vec![("target", s(target)), ("value", s(value))]
            }
            Finding::OutOfRange {
                target,
                value,
                min,
                max,
            } => vec![
                ("target", s(target)),
                ("value", s(value)),
                ("min", s(min)),
                ("max", s(max)),
            ],
            Finding::Scalar { target, values } => vec![
                ("target", s(target)),
                ("count", values.len().into()),
                ("values", values.join(", ").into()),
            ],
            Finding::FormulaEnum {
                facet,
                target,
                placeholder,
                reference,
                value,
                members,
            } => vec![
                ("facet", s(facet)),
                ("target", s(target)),
                ("placeholder", s(placeholder)),
                ("reference", s(reference)),
                ("value", s(value)),
                ("members", s(members)),
            ],
        }
    }
}

/// One violation: what, and of which task in which project, or of the
/// schema ([`SCHEMA_SUBJECT`], no project).
#[derive(Clone, PartialEq, Debug)]
pub struct Violation {
    pub project: Option<String>,
    pub harp_id: String,
    pub finding: Finding,
}

/// What a lint found, and how many targets it checked: an enum or range
/// declared. None checked means a clean sweep proves nothing.
#[derive(Clone, PartialEq, Debug)]
pub struct LintReport {
    pub reach: crate::Reach,
    pub violations: Vec<Violation>,
    pub checked_targets: usize,
}

/// The violations among `rows` and in `schema`, sorted by project, subject
/// and English, each once; and how many targets were checked.
pub(crate) fn lint(rows: &[Row], schema: &TagSchema) -> (Vec<Violation>, usize) {
    let enums: BTreeMap<&str, &[String]> = schema.enums().collect();
    let ranges: BTreeMap<&str, schema::Range> = schema.ranges().collect();
    let checked: BTreeSet<&str> = enums.keys().chain(ranges.keys()).copied().collect();
    let mut found: Vec<Violation> = formula_findings(schema, &enums)
        .into_iter()
        .map(|finding| Violation {
            project: None,
            harp_id: SCHEMA_SUBJECT.to_string(),
            finding,
        })
        .collect();
    for row in rows {
        let values = by_target(&row.task.tags);
        let findings = value_findings(&enums, &ranges, &values)
            .into_iter()
            .chain(scalar_findings(schema, &values));
        found.extend(findings.map(|finding| Violation {
            project: Some(row.project.id.clone()),
            harp_id: row.task.harp_id.clone(),
            finding,
        }));
    }
    let mut keyed: Vec<(String, Violation)> = found
        .into_iter()
        .map(|v| (v.finding.to_string(), v))
        .collect();
    keyed.sort_by(|(a, x), (b, y)| (&x.project, &x.harp_id, a).cmp(&(&y.project, &y.harp_id, b)));
    keyed.dedup_by(|(a, x), (b, y)| (&x.project, &x.harp_id, &*a) == (&y.project, &y.harp_id, &*b));
    (keyed.into_iter().map(|(_, v)| v).collect(), checked.len())
}

/// A task's distinct values by target, in the order its tags hold them;
/// a tag with no value carries nothing to check.
fn by_target(tags: &[crate::Tag]) -> BTreeMap<String, Vec<String>> {
    let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tag in tags {
        let Some(value) = &tag.value else { continue };
        let held = values.entry(tag.target()).or_default();
        if !held.contains(value) {
            held.push(value.clone());
        }
    }
    values
}

/// The values outside their target's enum or range.
fn value_findings(
    enums: &BTreeMap<&str, &[String]>,
    ranges: &BTreeMap<&str, schema::Range>,
    values: &BTreeMap<String, Vec<String>>,
) -> Vec<Finding> {
    let mut found = Vec::new();
    for (target, held) in values {
        if let Some(members) = enums.get(target.as_str()) {
            for value in held.iter().filter(|v| !members.contains(v)) {
                found.push(Finding::NotInEnum {
                    target: target.clone(),
                    value: value.clone(),
                    members: members.join(", "),
                });
            }
        }
        if let Some(range) = ranges.get(target.as_str()) {
            for value in held {
                match value.parse::<f64>().ok().filter(|n| n.is_finite()) {
                    None => found.push(Finding::NotANumber {
                        target: target.clone(),
                        value: value.clone(),
                    }),
                    Some(n) if n < range.min || n > range.max => found.push(Finding::OutOfRange {
                        target: target.clone(),
                        value: value.clone(),
                        min: range.min.to_string(),
                        max: range.max.to_string(),
                    }),
                    Some(_) => {}
                }
            }
        }
    }
    found
}

/// The scalar targets holding more than one value.
fn scalar_findings(schema: &TagSchema, values: &BTreeMap<String, Vec<String>>) -> Vec<Finding> {
    values
        .iter()
        .filter(|(target, held)| held.len() > 1 && schema.is_scalar(target))
        .map(|(target, held)| Finding::Scalar {
            target: target.clone(),
            values: held.clone(),
        })
        .collect()
}

/// The formulas' value-qualified placeholders whose value their target's
/// enum does not declare. `target=*` is a presence test, not a value.
fn formula_findings(schema: &TagSchema, enums: &BTreeMap<&str, &[String]>) -> Vec<Finding> {
    let facets = [
        ("priority_fn", schema.priority_fns()),
        ("decay_fn", schema.decay_fns()),
    ];
    let mut found = Vec::new();
    for (facet, formulas) in facets {
        for (target, formula) in formulas {
            for placeholder in placeholders(formula) {
                let Some((reference, value)) = placeholder.split_once('=') else {
                    continue;
                };
                if !reference.contains(':') || value == "*" {
                    continue;
                }
                let Some(members) = enums.get(reference) else {
                    continue;
                };
                if members.iter().any(|m| m == value) {
                    continue;
                }
                found.push(Finding::FormulaEnum {
                    facet: facet.to_string(),
                    target: target.clone(),
                    placeholder: format!("{{{{{placeholder}}}}}"),
                    reference: reference.to_string(),
                    value: value.to_string(),
                    members: members.join(", "),
                });
            }
        }
    }
    found
}

/// One of every finding, for the catalog test.
pub(crate) fn samples() -> Vec<Finding> {
    let x = || "x".to_string();
    vec![
        Finding::NotInEnum {
            target: x(),
            value: x(),
            members: x(),
        },
        Finding::NotANumber {
            target: x(),
            value: x(),
        },
        Finding::OutOfRange {
            target: x(),
            value: x(),
            min: x(),
            max: x(),
        },
        Finding::Scalar {
            target: x(),
            values: vec![x(), x()],
        },
        Finding::FormulaEnum {
            facet: x(),
            target: x(),
            placeholder: x(),
            reference: x(),
            value: x(),
            members: x(),
        },
    ]
}

#[cfg(test)]
#[path = "lint.test.rs"]
mod tests;
