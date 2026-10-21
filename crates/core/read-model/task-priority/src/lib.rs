//! Task priority, derived when read and never stored. A project's tag
//! schema declares two formulas: `priority_fn` scores a task from its tags
//! and the built-ins, and `decay_fn` gives `priority_fn` its `age_factor`.
//! The scores are rank-normalized onto `[0, MAX]` over the live (non-
//! terminal) tasks, and `triage:exploited-in-wild` forces a live task to
//! `MAX`. [`Diagnostics`] says when the ranking means little.
//!
//! The formulas are expr-lang's, as Go's are, with `{{...}}` placeholders:
//! a name holding `:` reads a tag (`{{triage:level}}`, `{{triage:kind=chore}}`,
//! `{{triage:exposed=*}}`), any other a built-in (`{{age_days}}`). Each
//! placeholder is compiled to a variable bound per task, so a tag's value
//! is data and never formula text. expr-lang checks names only as it runs,
//! so an unknown name in a formula is refused when a task is scored, not
//! when the formula compiles.

mod error;
mod formula;

use std::collections::{BTreeMap, BTreeSet};

use annals::next::Timestamp;
use tasks::{Tag, DEFERRED};

pub use error::{diagnostic_samples, PriorityError, EN_US};
use formula::Failure;
pub use formula::{placeholders, Facet, Formula};

/// The top of the display scale, and what the exploited-in-wild override
/// forces.
pub const MAX: f64 = 5.0;

/// The tag that forces a live task's display priority to [`MAX`].
pub const EXPLOITED_IN_WILD: &str = "triage:exploited-in-wild";

/// A task's age in days, to both formulas.
pub const AGE_DAYS: &str = "age_days";

/// `decay_fn`'s result, to `priority_fn`.
pub const AGE_FACTOR: &str = "age_factor";

/// The built-ins `facet`'s formulas may read, sorted. `decay_fn` produces
/// `age_factor`, so it cannot read it.
pub fn builtins(facet: Facet) -> &'static [&'static str] {
    match facet {
        Facet::PriorityFn => &[AGE_DAYS, AGE_FACTOR],
        Facet::DecayFn => &[AGE_DAYS],
    }
}

/// The project's formulas, compiled: at most one of each facet.
#[derive(Debug)]
pub struct Formulas {
    priority: Option<Formula>,
    decay: Option<Formula>,
}

impl Formulas {
    /// Every declared formula of each facet compiled (each `target`'s,
    /// by target), so a bad one on any target is refused; then more than
    /// one of a facet is refused as ambiguous, never resolved by a pick.
    pub fn compile(
        priority_fns: &BTreeMap<String, String>,
        decay_fns: &BTreeMap<String, String>,
    ) -> Result<Formulas, PriorityError> {
        Ok(Formulas {
            priority: one(Facet::PriorityFn, priority_fns)?,
            decay: one(Facet::DecayFn, decay_fns)?,
        })
    }

    fn each(&self) -> impl Iterator<Item = &Formula> {
        self.priority.iter().chain(&self.decay)
    }
}

/// `facet`'s one formula, if any is declared.
fn one(
    facet: Facet,
    declared: &BTreeMap<String, String>,
) -> Result<Option<Formula>, PriorityError> {
    let mut compiled = declared
        .iter()
        .map(|(target, source)| Formula::compile(facet, target, source))
        .collect::<Result<Vec<_>, _>>()?;
    if compiled.len() > 1 {
        return Err(PriorityError::Ambiguous {
            facet,
            targets: declared.keys().cloned().collect(),
        });
    }
    Ok(compiled.pop())
}

/// One task to rank.
#[derive(Clone, Debug)]
pub struct Candidate<'a> {
    pub harp_id: &'a str,
    pub status: &'a str,
    /// Whether the status is terminal: such a task is scored but never
    /// ranked.
    pub terminal: bool,
    pub tags: &'a [Tag],
    pub created_at: Timestamp,
}

/// One task's priority.
#[derive(Clone, PartialEq, Debug)]
pub struct Score {
    /// `priority_fn`'s result; 0 with no `priority_fn`.
    pub raw: f64,
    /// The display priority in `[0, MAX]`: 0 for a terminal task.
    pub priority: f64,
    /// Whether `priority` was forced to [`MAX`] by [`EXPLOITED_IN_WILD`].
    pub overridden: bool,
}

/// Every task's score, and how far the ranking can be trusted.
#[derive(Clone, PartialEq, Debug)]
pub struct Ranking {
    /// In the candidates' order.
    pub scores: Vec<Score>,
    pub diagnostics: Diagnostics,
}

/// When a ranking exists but means little. A ranking with no basis looks
/// like any other, so a caller sorting by it must say so.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Diagnostics {
    /// No `priority_fn` is declared: every raw score is 0.
    pub no_priority_fn: bool,
    /// More than one live task, every one on the same raw score.
    pub all_tied: bool,
    /// The live tasks carrying any target a formula reads.
    pub scored_tasks: usize,
    /// The live tasks: the population ranked, and `scored_tasks`'
    /// denominator.
    pub non_terminal_tasks: usize,
    /// Every target a formula reads and how many live tasks carry it,
    /// worst covered first, then by target. Empty with no formula reading
    /// a tag.
    pub target_coverage: Vec<TargetCoverage>,
}

/// One target a formula reads, and how many live tasks carry it, valued
/// or bare.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TargetCoverage {
    /// `ns:key`, without a placeholder's `=value`.
    pub target: String,
    pub tasks: usize,
}

/// Rank `candidates` by `formulas`, their ages measured to `now`.
pub fn rank(
    candidates: &[Candidate<'_>],
    formulas: &Formulas,
    now: Timestamp,
) -> Result<Ranking, PriorityError> {
    let referenced = referenced_targets(formulas);
    let mut carriers: BTreeMap<&str, usize> = referenced.iter().map(|t| (t.as_str(), 0)).collect();
    let mut scores = Vec::with_capacity(candidates.len());
    let mut live = Vec::new();
    let mut scored_tasks = 0;
    for (at, candidate) in candidates.iter().enumerate() {
        let values = tag_values(candidate.tags);
        let raw = raw_score(candidate, &values, formulas, now)?;
        scores.push(Score {
            raw,
            priority: 0.0,
            overridden: !candidate.terminal && values.contains_key(EXPLOITED_IN_WILD),
        });
        if candidate.terminal {
            continue;
        }
        live.push(at);
        let mut scored = false;
        for (target, count) in &mut carriers {
            if values.contains_key(*target) {
                *count += 1;
                scored = true;
            }
        }
        scored_tasks += usize::from(scored);
    }

    let raws: Vec<f64> = live.iter().map(|&at| scores[at].raw).collect();
    for (&at, priority) in live.iter().zip(rank_normalize(&raws)) {
        let score = &mut scores[at];
        score.priority = if score.overridden { MAX } else { priority };
    }
    let mut target_coverage: Vec<TargetCoverage> = carriers
        .into_iter()
        .map(|(target, tasks)| TargetCoverage {
            target: target.to_string(),
            tasks,
        })
        .collect();
    // Stable, so equal counts keep the targets' order.
    target_coverage.sort_by_key(|c| c.tasks);
    Ok(Ranking {
        scores,
        diagnostics: Diagnostics {
            no_priority_fn: formulas.priority.is_none(),
            all_tied: raws.len() > 1 && raws.iter().all(|raw| *raw == raws[0]),
            scored_tasks,
            non_terminal_tasks: live.len(),
            target_coverage,
        },
    })
}

/// `candidate`'s raw score: `priority_fn` over its tags, its age and
/// `decay_fn`'s age factor. A Deferred task is parked on purpose, so age
/// does not move it: its age factor is 1.
fn raw_score(
    candidate: &Candidate<'_>,
    values: &BTreeMap<String, f64>,
    formulas: &Formulas,
    now: Timestamp,
) -> Result<f64, PriorityError> {
    let age_days = age_days(candidate, now)?;
    let age_factor = match &formulas.decay {
        Some(decay) if candidate.status != DEFERRED => {
            evaluate(decay, candidate, values, &[(AGE_DAYS, age_days)])?
        }
        _ => 1.0,
    };
    let Some(priority) = &formulas.priority else {
        return Ok(0.0);
    };
    let raw = evaluate(
        priority,
        candidate,
        values,
        &[(AGE_DAYS, age_days), (AGE_FACTOR, age_factor)],
    )?;
    if !raw.is_finite() {
        // A non-finite score has no rank.
        return Err(PriorityError::NonFinite {
            harp_id: candidate.harp_id.to_string(),
            score: raw.to_string(),
        });
    }
    Ok(raw)
}

/// `candidate`'s age in days at `now`, 0 if it was created after `now`
/// (clock skew, a restored backup): a negative age would put the decay
/// curve's poles in reach. A task with no creation time is refused rather
/// than given an age of decades.
fn age_days(candidate: &Candidate<'_>, now: Timestamp) -> Result<f64, PriorityError> {
    let created = candidate.created_at;
    if created == Timestamp::default() {
        return Err(PriorityError::NoCreatedAt {
            harp_id: candidate.harp_id.to_string(),
        });
    }
    let seconds = (now.seconds - created.seconds) as f64
        + f64::from(now.nanos - created.nanos) / 1_000_000_000.0;
    Ok((seconds / 86_400.0).max(0.0))
}

/// `formula`'s result for `candidate`.
fn evaluate(
    formula: &Formula,
    candidate: &Candidate<'_>,
    values: &BTreeMap<String, f64>,
    builtins: &[(&str, f64)],
) -> Result<f64, PriorityError> {
    let harp_id = candidate.harp_id.to_string();
    let facet = formula.facet();
    let source = formula.source().to_string();
    formula
        .eval(values, builtins)
        .map_err(|failure| match failure {
            Failure::Unevaluable(reason) => PriorityError::Unevaluable {
                harp_id,
                facet,
                formula: source,
                reason,
            },
            Failure::NotANumber(result) => PriorityError::NotANumber {
                harp_id,
                facet,
                formula: source,
                result,
            },
        })
}

/// Every tag target the formulas read, without a placeholder's `=value`.
fn referenced_targets(formulas: &Formulas) -> BTreeSet<String> {
    formulas
        .each()
        .flat_map(Formula::tag_reads)
        .map(|read| read.split_once('=').map_or(read, |(target, _)| target))
        .map(str::to_string)
        .collect()
}

/// What a formula's placeholders read of a task with `tags`:
///
/// - every tag present sets `target=*` to 1, valued or bare;
/// - a bare tag sets `target` to 1;
/// - a valued tag sets `target=value` to 1, and `target` to its value as
///   a number: 0 when it is no finite number (lint reports it, priority
///   never fails on it).
///
/// An absent target sets nothing, so it reads 0. A target holding `=`
/// (tagma's quoting allows one) sets nothing either: its keys would land
/// on another target's `target=value`, and no placeholder can name it.
/// Tags are read in order, so a target's last value wins.
pub(crate) fn tag_values(tags: &[Tag]) -> BTreeMap<String, f64> {
    let mut values = BTreeMap::new();
    for tag in tags {
        let target = tag.target();
        if target.contains('=') {
            continue;
        }
        values.insert(format!("{target}=*"), 1.0);
        let Some(value) = &tag.value else {
            values.insert(target, 1.0);
            continue;
        };
        values.insert(format!("{target}={value}"), 1.0);
        let number = value.parse::<f64>().ok().filter(|n| n.is_finite());
        values.insert(target, number.unwrap_or(0.0));
    }
    values
}

/// Each of `raw`'s percentile on `[0, MAX]`: `MAX` times the share of the
/// population scoring no higher. Ties share a percentile, and the highest
/// score is always `MAX`.
pub(crate) fn rank_normalize(raw: &[f64]) -> Vec<f64> {
    let n = raw.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| raw[a].total_cmp(&raw[b]));
    let mut normalized = vec![0.0; n];
    let mut start = 0;
    while start < n {
        let value = raw[order[start]];
        // `==`, not total_cmp: -0 and 0 are one score. A NaN equals
        // nothing, so it is a group of its own.
        let mut end = start + 1;
        while end < n && raw[order[end]] == value {
            end += 1;
        }
        let priority = MAX * end as f64 / n as f64;
        for &at in &order[start..end] {
            normalized[at] = priority;
        }
        start = end;
    }
    normalized
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
