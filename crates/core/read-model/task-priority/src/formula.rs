//! A formula: its placeholders, and their compilation to expr-lang.

use std::collections::BTreeMap;
use std::fmt;

use expr::{Context, Program, Value};

use crate::{builtins, PriorityError};

/// Which formula: `priority_fn` or `decay_fn`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Facet {
    PriorityFn,
    DecayFn,
}

impl fmt::Display for Facet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Facet::PriorityFn => "priority_fn",
            Facet::DecayFn => "decay_fn",
        })
    }
}

/// One formula, compiled: its `n`th placeholder is the variable `__p<n>`,
/// bound for each evaluation to what it reads.
#[derive(Debug)]
pub struct Formula {
    facet: Facet,
    source: String,
    program: Program,
    /// The placeholders' names, in their variables' order.
    reads: Vec<String>,
}

/// Why a formula could not score a task; the caller names the task.
pub(crate) enum Failure {
    /// expr-lang failed as it ran.
    Unevaluable(String),
    /// The result, which is no number.
    NotANumber(String),
}

impl Formula {
    /// `source`, `facet`'s formula on `target`, compiled. A placeholder
    /// naming no tag (no `:`) must name one of `facet`'s built-ins.
    pub fn compile(facet: Facet, target: &str, source: &str) -> Result<Formula, PriorityError> {
        let mut rewritten = String::with_capacity(source.len());
        let mut reads = Vec::new();
        let mut copied = 0;
        for placeholder in scan(source) {
            if !is_tag(placeholder.name) && !builtins(facet).contains(&placeholder.name) {
                return Err(PriorityError::UnknownBuiltin {
                    facet,
                    target: target.to_string(),
                    name: placeholder.name.to_string(),
                });
            }
            rewritten.push_str(&source[copied..placeholder.start]);
            rewritten.push_str(&variable(reads.len()));
            reads.push(placeholder.name.to_string());
            copied = placeholder.end;
        }
        rewritten.push_str(&source[copied..]);
        let program = expr::compile(&rewritten).map_err(|e| PriorityError::Malformed {
            facet,
            target: target.to_string(),
            formula: source.to_string(),
            reason: e.to_string(),
        })?;
        Ok(Formula {
            facet,
            source: source.to_string(),
            program,
            reads,
        })
    }

    /// The formula as declared.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn facet(&self) -> Facet {
        self.facet
    }

    /// The tags this formula's placeholders read, as written (`ns:key`,
    /// `ns:key=value`, `ns:key=*`).
    pub(crate) fn tag_reads(&self) -> impl Iterator<Item = &str> {
        self.reads.iter().map(String::as_str).filter(|r| is_tag(r))
    }

    /// The formula's result with each tag placeholder reading `tags` (an
    /// absent key reads 0) and each built-in reading `builtins`.
    pub(crate) fn eval(
        &self,
        tags: &BTreeMap<String, f64>,
        builtins: &[(&str, f64)],
    ) -> Result<f64, Failure> {
        let mut context = Context::default();
        for (n, read) in self.reads.iter().enumerate() {
            let value = if is_tag(read) {
                tags.get(read).copied()
            } else {
                builtins
                    .iter()
                    .find(|(name, _)| name == read)
                    .map(|(_, v)| *v)
            };
            context.insert(variable(n), Value::Float(value.unwrap_or(0.0)));
        }
        match expr::run(&self.program, &context) {
            Ok(Value::Float(score)) => Ok(score),
            // A whole number is a score: Go's expr.AsFloat64 widens it too.
            Ok(Value::Integer(score)) => Ok(score as f64),
            Ok(other) => Err(Failure::NotANumber(other.to_string())),
            Err(e) => Err(Failure::Unevaluable(e.to_string())),
        }
    }
}

/// Whether a placeholder reads a tag rather than a built-in.
fn is_tag(name: &str) -> bool {
    name.contains(':')
}

/// The variable a formula's `n`th placeholder compiles to.
fn variable(n: usize) -> String {
    format!("__p{n}")
}

/// The names of `formula`'s placeholders, `{{ name }}`, trimmed, in order,
/// repeats kept: what Go's `\{\{\s*([^{}]+?)\s*}}` finds.
pub fn placeholders(formula: &str) -> Vec<&str> {
    scan(formula).into_iter().map(|p| p.name).collect()
}

/// One placeholder in a formula: its name, and where it starts and ends.
struct Placeholder<'a> {
    name: &'a str,
    start: usize,
    end: usize,
}

/// Every placeholder in `formula`, in order.
fn scan(formula: &str) -> Vec<Placeholder<'_>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = formula[from..].find("{{") {
        let start = from + at;
        let body = &formula[start + 2..];
        let close = body.find(['{', '}']).unwrap_or(body.len());
        let name = body[..close].trim();
        if body[close..].starts_with("}}") && !name.is_empty() {
            let end = start + 2 + close + 2;
            found.push(Placeholder { name, start, end });
            from = end;
        } else {
            from = start + 1;
        }
    }
    found
}
