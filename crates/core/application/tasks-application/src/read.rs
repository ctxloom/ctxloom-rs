//! The reads: which projects a read reaches, every live task in them, and
//! the listing taken from those tasks.
//!
//! A read is about the project the work root resolves to, or with
//! `global` about every project with a store, as Go's `--global` is. A
//! read that finds no project falls back to every project when the
//! working directory lies in no work root of its own (no CTXLOOM_ROOT, no
//! git repository), as Go's does; inside a work root it reads nothing.

use std::collections::{BTreeMap, BTreeSet};

use annals_diagnostic::Report;
use ports::{Clock, Configuration, ProjectIdentity, TaskStores};
use task_lists::TaskList;

use crate::lint::{self, LintReport};
use crate::priority::{self, PriorityNote};
use crate::view::{
    is_active, is_terminal, Row, StatusInfo, Task, DEFAULT_STATUSES, IN_PROGRESS, OPEN,
    STATUS_CLASSES, TERMINAL,
};
use crate::{failed, query, AppError, Project, Service, Shown, Tag};

/// What a read reached.
#[derive(Clone, PartialEq, Debug)]
pub enum Reach {
    /// The project the work root resolves to, or the one pinned.
    Project(Project),
    /// No project, in a work root of its own: nothing was read.
    NoProject,
    /// Every project with a store: `projects` of them. `fallback` is the
    /// working directory when no project there led to it, not `global`.
    Every {
        projects: usize,
        fallback: Option<String>,
    },
}

impl Reach {
    /// The one project read, if one was.
    pub fn project(&self) -> Option<&Project> {
        match self {
            Reach::Project(project) => Some(project),
            Reach::NoProject | Reach::Every { .. } => None,
        }
    }

    /// Whether it reached every project.
    pub fn is_every(&self) -> bool {
        matches!(self, Reach::Every { .. })
    }
}

/// Every live task a read reached: one project's in the order they were
/// added, or every project's by project id and then harp id, as Go's
/// `--global` orders them.
pub(crate) struct Scan {
    pub reach: Reach,
    pub rows: Vec<Row>,
}

/// The tasks a default listing hides among those its filters matched.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Hidden {
    /// Terminal: Done or Archived.
    pub completed: usize,
    pub deferred: usize,
}

impl Hidden {
    pub fn total(&self) -> usize {
        self.completed + self.deferred
    }
}

/// A listing: the tasks a query selected, where it read them, and what it
/// left out.
#[derive(Clone, PartialEq, Debug)]
pub struct Listing {
    pub reach: Reach,
    pub rows: Vec<Row>,
    /// Which of their tags a person is shown.
    pub shown: Shown,
    /// What the default view hid of what the filters matched.
    pub hidden: Hidden,
    /// Whether a term or a tag query searched, so that what the default
    /// view hid is worth saying.
    pub searched: bool,
    /// How many tasks `limit` cut off the end.
    pub omitted: usize,
    /// Each row's derived priority, in the rows' order, when the listing
    /// is sorted by it.
    pub priorities: Option<Vec<f64>>,
    /// Why the ranking means little, when sorted by priority.
    pub priority_notes: Vec<PriorityNote>,
}

impl Listing {
    /// The one project read, if one was.
    pub fn project(&self) -> Option<&Project> {
        self.reach.project()
    }

    /// The tasks, in order.
    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.rows.iter().map(|row| &row.task)
    }

    /// The rows, in order, each with its derived priority when the listing
    /// is sorted by it.
    pub fn ranked(&self) -> impl Iterator<Item = (&Row, Option<f64>)> {
        let priorities = self.priorities.iter().flatten().copied().map(Some);
        self.rows
            .iter()
            .zip(priorities.chain(std::iter::repeat(None)))
    }
}

/// The tasks `show` was asked for, in the order asked, and where it read
/// them.
#[derive(Clone, PartialEq, Debug)]
pub struct Details {
    pub reach: Reach,
    pub rows: Vec<Row>,
    /// Which of their tags a person is shown.
    pub shown: Shown,
}

/// How many tasks hold each status, over every task the read reached
/// whatever a listing would filter or hide, and the tasks In Progress.
#[derive(Clone, PartialEq, Debug)]
pub struct Summary {
    pub reach: Reach,
    /// By status, sorted.
    pub counts: BTreeMap<String, usize>,
    pub in_progress: Vec<Row>,
}

/// The statuses: the default ones in their order, then any other a task
/// the read reached holds, by name.
#[derive(Clone, PartialEq, Debug)]
pub struct Statuses {
    pub reach: Reach,
    pub statuses: Vec<StatusInfo>,
}

/// One tag in use: how many tasks hold it in all, and how many of those
/// are active (neither terminal nor Deferred).
#[derive(Clone, PartialEq, Debug)]
pub struct TagCount {
    pub tag: Tag,
    pub active: usize,
    pub total: usize,
}

/// The tags in use among the tasks a read selected, by their canonical
/// spelling, those the tag schema hides left out.
#[derive(Clone, PartialEq, Debug)]
pub struct Vocabulary {
    pub reach: Reach,
    pub tags: Vec<TagCount>,
}

/// A status filter: statuses by name, and the classes `@open` (every
/// status not terminal, whatever its name) and `@terminal`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct StatusFilter {
    names: Vec<String>,
    open: bool,
    terminal: bool,
}

impl StatusFilter {
    /// The filter `statuses` name, trimmed, blanks dropped; a value
    /// starting with `@` that names no class is refused.
    pub fn parse(statuses: &[String]) -> Result<StatusFilter, AppError> {
        let mut filter = StatusFilter::default();
        for status in statuses.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            match status {
                OPEN => filter.open = true,
                TERMINAL => filter.terminal = true,
                class if class.starts_with(tasks::STATUS_CLASS_SIGIL) => {
                    return Err(AppError::UnknownStatusClass {
                        status: class.to_string(),
                    })
                }
                name => filter.names.push(name.to_string()),
            }
        }
        Ok(filter)
    }

    /// Whether it names anything: a filter naming nothing holds every
    /// status.
    pub fn is_explicit(&self) -> bool {
        self.open || self.terminal || !self.names.is_empty()
    }

    pub fn holds(&self, status: &str) -> bool {
        if !self.is_explicit() {
            return true;
        }
        let terminal = is_terminal(status);
        (self.open && !terminal)
            || (self.terminal && terminal)
            || self.names.iter().any(|n| n == status)
    }
}

/// Whether `text` holds `term`, case-insensitively; an empty term is
/// held by every text.
pub(crate) fn mentions(text: &str, term: &str) -> bool {
    let term = term.trim().to_lowercase();
    term.is_empty() || text.to_lowercase().contains(&term)
}

/// The rows of `rows` whose status `filter` holds, whose text mentions
/// `term`, and which `tag_query` selects.
pub(crate) fn matching(
    rows: Vec<Row>,
    filter: &StatusFilter,
    term: &str,
    tag_query: &str,
    schema: &crate::TagSchema,
) -> Result<Vec<Row>, AppError> {
    let candidates: Vec<Row> = rows
        .into_iter()
        .filter(|row| filter.holds(&row.task.status) && mentions(&row.task.text, term))
        .collect();
    // Indexed by position: harp ids repeat across projects.
    let ids: Vec<String> = (0..candidates.len()).map(|i| i.to_string()).collect();
    let selected = query::select(
        tag_query,
        candidates
            .iter()
            .zip(&ids)
            .map(|(row, id)| (id.as_str(), row.task.tags.as_slice())),
        schema,
    )
    .map_err(|e| AppError::TagQuery(Report::of(&e)))?;
    Ok(candidates
        .into_iter()
        .zip(ids)
        .filter(|(_, id)| selected.contains(id))
        .map(|(row, _)| row)
        .collect())
}

/// The status classes, as an error lists them.
pub(crate) fn classes() -> String {
    STATUS_CLASSES.join(", ")
}

impl<I, S, N, C, D, L, G> Service<I, S, N, C, D, L, G>
where
    I: ProjectIdentity,
    S: TaskStores,
    C: Clock,
    G: Configuration,
{
    /// Every live task the read reaches: `pinned`'s project, the work
    /// root's, or with `global` (or no project, outside a work root) every
    /// project's.
    pub(crate) async fn scan(
        &mut self,
        pinned: Option<&str>,
        global: bool,
    ) -> Result<Scan, AppError> {
        let fallback = if global {
            None
        } else {
            let found = self.identity.for_read(pinned).map_err(AppError::Identity)?;
            match found {
                Some(project) => {
                    let rows = self.rows_of(&project).await?.unwrap_or_default();
                    return Ok(Scan {
                        reach: Reach::Project(project),
                        rows,
                    });
                }
                None => match self
                    .identity
                    .rootless_directory()
                    .map_err(AppError::Identity)?
                {
                    Some(directory) => Some(directory),
                    None => {
                        return Ok(Scan {
                            reach: Reach::NoProject,
                            rows: Vec::new(),
                        })
                    }
                },
            }
        };
        let every = self.identity.every_project().map_err(AppError::Identity)?;
        let (mut rows, mut projects) = (Vec::new(), 0);
        for project in every {
            if let Some(found) = self.rows_of(&project).await? {
                projects += 1;
                rows.extend(found);
            }
        }
        rows.sort_by(|a, b| {
            (a.project.id.as_str(), a.task.harp_id.as_str())
                .cmp(&(b.project.id.as_str(), b.task.harp_id.as_str()))
        });
        Ok(Scan {
            reach: Reach::Every { projects, fallback },
            rows,
        })
    }

    /// The live tasks of `project`'s store in the order they were added;
    /// `None` when it has no store.
    async fn rows_of(&mut self, project: &Project) -> Result<Option<Vec<Row>>, AppError> {
        let Some(store) = self.stores.open_existing(project).await.map_err(failed)? else {
            return Ok(None);
        };
        let mut list = TaskList::new(Box::new(|_| true));
        list.catch_up_async(store, &self.registry)
            .await
            .map_err(|e| AppError::Failed(Report::of(&*e)))?;
        let mut tasks: Vec<Task> = list.entries().into_iter().map(Task::of).collect();
        tasks.sort_by(|a, b| {
            let added = |t: &Task| (t.created_at.seconds, t.created_at.nanos);
            added(a)
                .cmp(&added(b))
                .then_with(|| a.harp_id.cmp(&b.harp_id))
        });
        Ok(Some(
            tasks
                .into_iter()
                .map(|task| Row {
                    project: project.clone(),
                    task,
                })
                .collect(),
        ))
    }

    /// The counts by status of every task `scope` reaches.
    pub(crate) async fn summed(&mut self, scope: crate::Scope) -> Result<Summary, AppError> {
        let scan = self.scan(scope.project.as_deref(), scope.global).await?;
        let mut counts = BTreeMap::new();
        for row in &scan.rows {
            *counts.entry(row.task.status.clone()).or_insert(0) += 1;
        }
        let in_progress = scan
            .rows
            .into_iter()
            .filter(|row| row.task.status == IN_PROGRESS)
            .collect();
        Ok(Summary {
            reach: scan.reach,
            counts,
            in_progress,
        })
    }

    /// The default statuses, and those in use where `scope` reaches.
    pub(crate) async fn taxonomy(&mut self, scope: crate::Scope) -> Result<Statuses, AppError> {
        let scan = self.scan(scope.project.as_deref(), scope.global).await?;
        let others: BTreeSet<&str> = scan
            .rows
            .iter()
            .map(|row| row.task.status.as_str())
            .filter(|status| !DEFAULT_STATUSES.contains(status))
            .collect();
        let statuses = DEFAULT_STATUSES
            .into_iter()
            .chain(others)
            .enumerate()
            .map(|(order, name)| StatusInfo::of(name, order))
            .collect();
        Ok(Statuses {
            reach: scan.reach,
            statuses,
        })
    }

    /// The tags of the tasks `query` selects, every status included (its
    /// `all`, `limit` and `sort` mean nothing here), counted.
    pub(crate) async fn vocabulary(
        &mut self,
        query: crate::ListQuery,
    ) -> Result<Vocabulary, AppError> {
        query::check(&query.tag_query).map_err(|e| AppError::TagQuery(Report::of(&e)))?;
        let filter = StatusFilter::parse(&query.statuses)?;
        let schema = self.schema(&query.config_set)?;
        let scan = self.scan(query.project.as_deref(), query.global).await?;
        let rows = matching(scan.rows, &filter, &query.term, &query.tag_query, &schema)?;
        let shown = Shown::of(&schema);
        let mut counts: BTreeMap<String, TagCount> = BTreeMap::new();
        for row in &rows {
            let active = is_active(&row.task.status);
            for tag in row.task.tags.iter().filter(|t| shown.shows(t)) {
                let count = counts.entry(tag.to_string()).or_insert_with(|| TagCount {
                    tag: tag.clone(),
                    active: 0,
                    total: 0,
                });
                count.total += 1;
                count.active += usize::from(active);
            }
        }
        Ok(Vocabulary {
            reach: scan.reach,
            tags: counts.into_values().collect(),
        })
    }

    /// Every task `scope` reaches, and the schema, linted.
    pub(crate) async fn linted(&mut self, scope: crate::Scope) -> Result<LintReport, AppError> {
        let schema = self.schema(&scope.config_set)?;
        let scan = self.scan(scope.project.as_deref(), scope.global).await?;
        let (violations, checked_targets) = lint::lint(&scan.rows, &schema);
        Ok(LintReport {
            reach: scan.reach,
            violations,
            checked_targets,
        })
    }

    /// The tasks `query` names, every one of them found once.
    pub(crate) async fn details(&mut self, query: crate::ShowQuery) -> Result<Details, AppError> {
        let schema = self.schema(&query.config_set)?;
        let scan = self.scan(query.project.as_deref(), query.global).await?;
        let (mut rows, mut missing, mut held) = (Vec::new(), Vec::new(), Vec::new());
        for id in &query.harp_ids {
            let found: Vec<&Row> = scan.rows.iter().filter(|r| r.task.harp_id == *id).collect();
            match found.as_slice() {
                [] => missing.push(id.clone()),
                [row] => rows.push((*row).clone()),
                twice => held.push((
                    id.clone(),
                    twice.iter().map(|r| r.project.id.clone()).collect(),
                )),
            }
        }
        if !missing.is_empty() {
            return Err(AppError::NotFound { harp_ids: missing });
        }
        if !held.is_empty() {
            return Err(AppError::Ambiguous { held });
        }
        Ok(Details {
            reach: scan.reach,
            rows,
            shown: Shown::of(&schema),
        })
    }

    /// The listing `query` asks for.
    pub(crate) async fn listing(&mut self, query: crate::ListQuery) -> Result<Listing, AppError> {
        // A malformed query is refused whatever there is to run it over.
        query::check(&query.tag_query).map_err(|e| AppError::TagQuery(Report::of(&e)))?;
        let by_priority = match query.sort.trim() {
            "" => false,
            crate::PRIORITY => true,
            sort => {
                return Err(AppError::UnknownSort {
                    sort: sort.to_string(),
                })
            }
        };
        let filter = StatusFilter::parse(&query.statuses)?;
        let schema = self.schema(&query.config_set)?;
        let scan = self.scan(query.project.as_deref(), query.global).await?;
        // Each project's whole population is ranked, not the page the
        // filters leave.
        let ranked = match by_priority {
            true => {
                let formulas = priority::formulas(&schema)?;
                let now = self.clock.now();
                Some(priority::rank_rows(
                    &scan.rows,
                    &formulas,
                    now,
                    scan.reach.is_every(),
                )?)
            }
            false => None,
        };
        let matched = matching(scan.rows, &filter, &query.term, &query.tag_query, &schema)?;
        let mut hidden = Hidden::default();
        let mut rows: Vec<Row> = if query.all || filter.is_explicit() {
            matched
        } else {
            matched
                .into_iter()
                .filter(|row| {
                    let status = &row.task.status;
                    if is_active(status) {
                        return true;
                    }
                    if is_terminal(status) {
                        hidden.completed += 1;
                    } else {
                        hidden.deferred += 1;
                    }
                    false
                })
                .collect()
        };
        if let Some(ranked) = &ranked {
            // Stable: ties keep the listing's order. The limit is applied
            // after, so it keeps the highest priorities.
            rows.sort_by(|a, b| {
                a.project
                    .id
                    .cmp(&b.project.id)
                    .then_with(|| ranked.priority(b).total_cmp(&ranked.priority(a)))
            });
        }
        let omitted = match query.limit {
            0 => 0,
            limit => {
                let omitted = rows.len().saturating_sub(limit);
                rows.truncate(limit);
                omitted
            }
        };
        Ok(Listing {
            reach: scan.reach,
            shown: Shown::of(&schema),
            hidden,
            searched: !query.term.trim().is_empty() || !query.tag_query.trim().is_empty(),
            omitted,
            priorities: ranked.as_ref().map(|ranked| rows_priorities(&rows, ranked)),
            priority_notes: ranked.map(|ranked| ranked.notes).unwrap_or_default(),
            rows,
        })
    }
}

/// Each of `rows`' priority in `ranked`, in order.
fn rows_priorities(rows: &[Row], ranked: &priority::Ranked) -> Vec<f64> {
    rows.iter().map(|row| ranked.priority(row)).collect()
}

#[cfg(test)]
#[path = "read.test.rs"]
mod tests;
