//! Why priority could not be derived.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

use crate::{builtins, Facet};

/// Why priority could not be derived: a formula refused when compiled, or
/// a task it could not score.
#[derive(Clone, PartialEq, Debug)]
pub enum PriorityError {
    /// The formula is not expr-lang.
    Malformed {
        facet: Facet,
        target: String,
        formula: String,
        reason: String,
    },
    /// A placeholder naming no tag and no built-in the facet has.
    UnknownBuiltin {
        facet: Facet,
        target: String,
        name: String,
    },
    /// More than one formula of a facet.
    Ambiguous { facet: Facet, targets: Vec<String> },
    /// The formula failed as it ran for a task (an unknown name, a
    /// function given the wrong arguments).
    Unevaluable {
        harp_id: String,
        facet: Facet,
        formula: String,
        reason: String,
    },
    /// The formula's result for a task is not a number.
    NotANumber {
        harp_id: String,
        facet: Facet,
        formula: String,
        result: String,
    },
    /// A task's score is NaN or infinite.
    NonFinite { harp_id: String, score: String },
    /// A task with no creation time has no age.
    NoCreatedAt { harp_id: String },
}

impl fmt::Display for PriorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PriorityError::Malformed {
                facet,
                target,
                formula,
                reason,
            } => write!(
                f,
                "{facet} on {target} is not a formula: {reason} (formula: {formula})"
            ),
            PriorityError::UnknownBuiltin {
                facet,
                target,
                name,
            } => write!(
                f,
                "{facet} on {target} reads {name}, which is no tag (no ':') and no built-in of {facet} (those are {})",
                known(*facet)
            ),
            PriorityError::Ambiguous { facet, targets } => write!(
                f,
                "more than one {facet} is declared (on {}): exactly one is allowed",
                targets.join(", ")
            ),
            PriorityError::Unevaluable {
                harp_id,
                facet,
                formula,
                reason,
            } => write!(
                f,
                "{facet} could not score task {harp_id}: {reason} (formula: {formula})"
            ),
            PriorityError::NotANumber {
                harp_id,
                facet,
                formula,
                result,
            } => write!(
                f,
                "{facet} gives task {harp_id} {result}, which is no number (formula: {formula})"
            ),
            PriorityError::NonFinite { harp_id, score } => write!(
                f,
                "priority_fn gives task {harp_id} the score {score}, which is no finite number: check its tags for a value that is not one"
            ),
            PriorityError::NoCreatedAt { harp_id } => write!(
                f,
                "task {harp_id} has no creation time, so it has no age to rank by"
            ),
        }
    }
}

impl Diagnostic for PriorityError {
    fn code(&self) -> &'static str {
        match self {
            PriorityError::Malformed { .. } => "task-priority-malformed",
            PriorityError::UnknownBuiltin { .. } => "task-priority-unknown-builtin",
            PriorityError::Ambiguous { .. } => "task-priority-ambiguous",
            PriorityError::Unevaluable { .. } => "task-priority-unevaluable",
            PriorityError::NotANumber { .. } => "task-priority-not-a-number",
            PriorityError::NonFinite { .. } => "task-priority-non-finite",
            PriorityError::NoCreatedAt { .. } => "task-priority-no-created-at",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let s = |s: &str| Arg::from(s);
        match self {
            PriorityError::Malformed {
                facet,
                target,
                formula,
                reason,
            } => vec![
                ("facet", facet.to_string().into()),
                ("target", s(target)),
                ("formula", s(formula)),
                ("reason", s(reason)),
            ],
            PriorityError::UnknownBuiltin {
                facet,
                target,
                name,
            } => vec![
                ("facet", facet.to_string().into()),
                ("target", s(target)),
                ("name", s(name)),
                ("known", known(*facet).into()),
            ],
            PriorityError::Ambiguous { facet, targets } => vec![
                ("facet", facet.to_string().into()),
                ("targets", targets.join(", ").into()),
            ],
            PriorityError::Unevaluable {
                harp_id,
                facet,
                formula,
                reason,
            } => vec![
                ("harp_id", s(harp_id)),
                ("facet", facet.to_string().into()),
                ("formula", s(formula)),
                ("reason", s(reason)),
            ],
            PriorityError::NotANumber {
                harp_id,
                facet,
                formula,
                result,
            } => vec![
                ("harp_id", s(harp_id)),
                ("facet", facet.to_string().into()),
                ("formula", s(formula)),
                ("result", s(result)),
            ],
            PriorityError::NonFinite { harp_id, score } => {
                vec![("harp_id", s(harp_id)), ("score", s(score))]
            }
            PriorityError::NoCreatedAt { harp_id } => vec![("harp_id", s(harp_id))],
        }
    }
}

/// `facet`'s built-ins, for a message.
fn known(facet: Facet) -> String {
    builtins(facet).join(", ")
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    "task-priority",
    include_str!("../locales/en-US/task-priority.ftl"),
);

/// One of every error, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let x = || "x".to_string();
    let facet = Facet::PriorityFn;
    vec![
        Box::new(PriorityError::Malformed {
            facet,
            target: x(),
            formula: x(),
            reason: x(),
        }),
        Box::new(PriorityError::UnknownBuiltin {
            facet,
            target: x(),
            name: x(),
        }),
        Box::new(PriorityError::Ambiguous {
            facet,
            targets: vec![x(), x()],
        }),
        Box::new(PriorityError::Unevaluable {
            harp_id: x(),
            facet,
            formula: x(),
            reason: x(),
        }),
        Box::new(PriorityError::NotANumber {
            harp_id: x(),
            facet,
            formula: x(),
            result: x(),
        }),
        Box::new(PriorityError::NonFinite {
            harp_id: x(),
            score: x(),
        }),
        Box::new(PriorityError::NoCreatedAt { harp_id: x() }),
    ]
}
