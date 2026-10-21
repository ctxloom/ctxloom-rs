//! A listing sorted by priority: each project's tasks ranked against that
//! project's whole live population (not the page a filter narrowed them
//! to), by the tag schema's formulas, and the warnings a ranking that
//! means little earns.

use std::collections::BTreeMap;
use std::fmt;

use annals::next::Timestamp;
use annals_diagnostic::{Arg, Diagnostic, Report};
use task_priority::{Candidate, Diagnostics, Formulas};

use crate::view::{is_terminal, Row};
use crate::{AppError, TagSchema};

/// A ranking warns of a formula term reaching fewer than this share of
/// the live tasks: most of them are ranked by its absence, not by
/// anything they carry.
const THIN_COVERAGE: f64 = 0.5;

/// Why a ranking means little.
#[derive(Clone, PartialEq, Debug)]
pub enum PriorityWarning {
    /// No `priority_fn` is declared.
    NoPriorityFn,
    /// Every live task ties.
    AllTied { scored: usize, live: usize },
    /// A target the formulas read that no live task carries.
    Inert { target: String, live: usize },
    /// A target the formulas read that most live tasks lack.
    Thin {
        uncovered: usize,
        live: usize,
        target: String,
    },
}

/// A warning, and the project it is about when a listing reached more
/// than one.
#[derive(Clone, PartialEq, Debug)]
pub struct PriorityNote {
    pub project: Option<String>,
    pub warning: PriorityWarning,
}

/// The rows' priorities, by project and harp id, and the warnings.
pub(crate) struct Ranked {
    priorities: BTreeMap<String, BTreeMap<String, f64>>,
    pub notes: Vec<PriorityNote>,
}

impl Ranked {
    /// `row`'s priority: 0 for a row not ranked.
    pub fn priority(&self, row: &Row) -> f64 {
        self.priorities
            .get(&row.project.id)
            .and_then(|project| project.get(&row.task.harp_id))
            .copied()
            .unwrap_or(0.0)
    }
}

/// `schema`'s formulas, compiled.
pub(crate) fn formulas(schema: &TagSchema) -> Result<Formulas, AppError> {
    Formulas::compile(schema.priority_fns(), schema.decay_fns()).map_err(refused)
}

/// `row` as a candidate for ranking.
pub(crate) fn candidate(row: &Row) -> Candidate<'_> {
    let task = &row.task;
    Candidate {
        harp_id: &task.harp_id,
        status: &task.status,
        terminal: is_terminal(&task.status),
        tags: &task.tags,
        created_at: task.created_at,
    }
}

/// Every project's `rows` ranked among themselves at `now`; the warnings
/// name their project if `attributed`.
pub(crate) fn rank_rows(
    rows: &[Row],
    formulas: &Formulas,
    now: Timestamp,
    attributed: bool,
) -> Result<Ranked, AppError> {
    let mut by_project: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        by_project.entry(&row.project.id).or_default().push(row);
    }
    let mut ranked = Ranked {
        priorities: BTreeMap::new(),
        notes: Vec::new(),
    };
    for (project, rows) in by_project {
        let candidates: Vec<Candidate<'_>> = rows.iter().map(|row| candidate(row)).collect();
        let ranking = task_priority::rank(&candidates, formulas, now).map_err(refused)?;
        let priorities = rows
            .iter()
            .zip(&ranking.scores)
            .map(|(row, score)| (row.task.harp_id.clone(), score.priority))
            .collect();
        ranked.priorities.insert(project.to_string(), priorities);
        ranked
            .notes
            .extend(
                warnings(&ranking.diagnostics)
                    .into_iter()
                    .map(|warning| PriorityNote {
                        project: attributed.then(|| project.to_string()),
                        warning,
                    }),
            );
    }
    Ok(ranked)
}

fn refused(error: task_priority::PriorityError) -> AppError {
    AppError::Priority(Report::of(&error))
}

/// What `diagnostics` warns of: no `priority_fn` alone, or a tie and each
/// target fewer than half the live tasks carry, worst first. With no live
/// task there is no ranking to mislead.
pub(crate) fn warnings(diagnostics: &Diagnostics) -> Vec<PriorityWarning> {
    if diagnostics.no_priority_fn {
        return vec![PriorityWarning::NoPriorityFn];
    }
    let live = diagnostics.non_terminal_tasks;
    let mut warned = Vec::new();
    if diagnostics.all_tied {
        warned.push(PriorityWarning::AllTied {
            scored: diagnostics.scored_tasks,
            live,
        });
    }
    if live == 0 {
        return warned;
    }
    for coverage in &diagnostics.target_coverage {
        if coverage.tasks as f64 >= THIN_COVERAGE * live as f64 {
            continue;
        }
        let target = coverage.target.clone();
        warned.push(match coverage.tasks {
            0 => PriorityWarning::Inert { target, live },
            carried => PriorityWarning::Thin {
                uncovered: live - carried,
                live,
                target,
            },
        });
    }
    warned
}

impl fmt::Display for PriorityWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PriorityWarning::NoPriorityFn => f.write_str(
                "--sort priority is meaningless here: this project's tag_schema declares no priority_fn, so every task's raw score is 0 and the ranking reflects nothing",
            ),
            PriorityWarning::AllTied { scored, live } => write!(
                f,
                "--sort priority is meaningless here: every active task ties at the same raw priority score (only {scored} of {live} active tasks carry a tag any priority_fn/decay_fn formula actually reads) \u{2014} the ranking reflects nothing"
            ),
            PriorityWarning::Inert { target, live } => write!(
                f,
                "priority_fn/decay_fn reads {target}, which none of the {live} active tasks carries \u{2014} that term is inert and moves no task's rank"
            ),
            PriorityWarning::Thin {
                uncovered,
                live,
                target,
            } => write!(
                f,
                "{uncovered} of {live} active tasks carry no {target} \u{2014} they rank as if it were absent, on a formula that reads it"
            ),
        }
    }
}

impl Diagnostic for PriorityWarning {
    fn code(&self) -> &'static str {
        match self {
            PriorityWarning::NoPriorityFn => "tasks-application-priority-no-priority-fn",
            PriorityWarning::AllTied { .. } => "tasks-application-priority-all-tied",
            PriorityWarning::Inert { .. } => "tasks-application-priority-inert",
            PriorityWarning::Thin { .. } => "tasks-application-priority-thin",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            PriorityWarning::NoPriorityFn => Vec::new(),
            PriorityWarning::AllTied { scored, live } => {
                vec![("scored", (*scored).into()), ("live", (*live).into())]
            }
            PriorityWarning::Inert { target, live } => {
                vec![("target", target.as_str().into()), ("live", (*live).into())]
            }
            PriorityWarning::Thin {
                uncovered,
                live,
                target,
            } => vec![
                ("uncovered", (*uncovered).into()),
                ("live", (*live).into()),
                ("target", target.as_str().into()),
            ],
        }
    }
}

/// Go's `attributeToProject`: a warning about one of several projects
/// names it, so each line of a listing over every project says whose it is.
impl fmt::Display for PriorityNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.project {
            Some(project) => write!(f, "project {project}: {}", self.warning),
            None => self.warning.fmt(f),
        }
    }
}

impl Diagnostic for PriorityNote {
    fn code(&self) -> &'static str {
        match self.project {
            Some(_) => "tasks-application-priority-in-project",
            None => self.warning.code(),
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match &self.project {
            Some(project) => vec![
                ("project", project.as_str().into()),
                ("warning", Arg::nested(&self.warning)),
            ],
            None => self.warning.args(),
        }
    }
}

/// One of every warning, for the catalog test.
pub(crate) fn samples() -> Vec<PriorityNote> {
    let x = || "x".to_string();
    let bare = |warning| PriorityNote {
        project: None,
        warning,
    };
    vec![
        bare(PriorityWarning::NoPriorityFn),
        bare(PriorityWarning::AllTied { scored: 1, live: 2 }),
        bare(PriorityWarning::Inert {
            target: x(),
            live: 2,
        }),
        bare(PriorityWarning::Thin {
            uncovered: 1,
            live: 2,
            target: x(),
        }),
        PriorityNote {
            project: Some(x()),
            warning: PriorityWarning::NoPriorityFn,
        },
    ]
}

#[cfg(test)]
#[path = "priority.test.rs"]
mod tests;
