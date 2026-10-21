//! Why a use case did not complete.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic, Report};

/// Why a use case did not complete. A wrapper of another error
/// ([`AppError::Rejected`], [`AppError::TagRefused`], [`AppError::Failed`],
/// [`AppError::Identity`], [`AppError::Configuration`],
/// [`AppError::TagQuery`], [`AppError::Prune`], [`AppError::Priority`]) is
/// that error's diagnostic: its code, args and English.
#[derive(Clone, PartialEq, Debug)]
pub enum AppError {
    /// The domain refused the command: tell the person.
    Rejected(Report),
    /// A tag given to a write was refused before any command was sent
    /// ([`crate::TagRefusal`]): tell the person.
    TagRefused(Report),
    /// The store or the kernel failed in a way a retry will not mend.
    Failed(Report),
    /// The project could not be resolved.
    Identity(Report),
    /// The configuration could not be read, or its tag schema is malformed.
    Configuration(Report),
    /// A malformed tag query ([`crate::QueryError`]).
    TagQuery(Report),
    /// A prune refused before it read anything ([`crate::PruneRefusal`]).
    Prune(Report),
    /// Every attempt the retry budget allows failed with a retryable error;
    /// `error` is the last.
    GaveUp { attempts: u32, error: Report },
    /// Every freshly minted task id was already taken.
    HarpsExhausted { attempts: u32 },
    /// A task the store accepted is not there when read back.
    Vanished { harp_id: String },
    /// A change to an existing task where the work root has no project.
    NoProject,
    /// An import of a project Go keeps no task log for.
    NoGoLog { project: String },
    /// A status filter's value starting with `@` that names no class.
    UnknownStatusClass { status: String },
    /// A sort taskloom does not know.
    UnknownSort { sort: String },
    /// Priority could not be derived (`task_priority::PriorityError`): a
    /// formula that does not compile, or a task it cannot score.
    Priority(Report),
    /// Ids asked for that no task holds: every one of them.
    NotFound { harp_ids: Vec<String> },
    /// Ids held in more than one project, read across projects: each with
    /// the projects holding it.
    Ambiguous { held: Vec<(String, Vec<String>)> },
}

/// Each of `ids`, quoted, comma-separated.
fn quoted(ids: &[String]) -> String {
    ids.iter()
        .map(|id| format!("{id:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Each id held twice and the projects that hold it.
fn held_in(held: &[(String, Vec<String>)]) -> String {
    held.iter()
        .map(|(id, projects)| format!("{id:?} in {}", projects.join(", ")))
        .collect::<Vec<_>>()
        .join("; ")
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Rejected(cause)
            | AppError::TagRefused(cause)
            | AppError::Failed(cause)
            | AppError::Identity(cause)
            | AppError::Configuration(cause)
            | AppError::TagQuery(cause)
            | AppError::Prune(cause)
            | AppError::Priority(cause) => cause.fmt(f),
            AppError::GaveUp { attempts, error } => {
                write!(f, "gave up after {attempts} attempts: {error}")
            }
            AppError::HarpsExhausted { attempts } => {
                write!(f, "no free task id after {attempts} tries")
            }
            AppError::Vanished { harp_id } => {
                write!(f, "task {harp_id} was written but cannot be read back")
            }
            AppError::NoProject => {
                f.write_str("there is no project here yet, so no task to change")
            }
            AppError::NoGoLog { project } => {
                write!(f, "Go taskloom has no task log for project {project}")
            }
            AppError::UnknownStatusClass { status } => write!(
                f,
                "{status} is no status class (the classes are {}): '@' is reserved for them, so no status starts with it",
                crate::read::classes()
            ),
            AppError::UnknownSort { sort } => {
                write!(f, "{sort} is no sort: the only one is priority")
            }
            AppError::NotFound { harp_ids } => write!(
                f,
                "no task holds these harp ids: {} (see `taskloom list`)",
                quoted(harp_ids)
            ),
            AppError::Ambiguous { held } => write!(
                f,
                "harp ids held by more than one project ({}): pass --project <id> instead of --global to choose one",
                held_in(held)
            ),
        }
    }
}

impl Diagnostic for AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::Rejected(cause)
            | AppError::TagRefused(cause)
            | AppError::Failed(cause)
            | AppError::Identity(cause)
            | AppError::Configuration(cause)
            | AppError::TagQuery(cause)
            | AppError::Prune(cause)
            | AppError::Priority(cause) => cause.code(),
            AppError::GaveUp { .. } => "tasks-application-gave-up",
            AppError::HarpsExhausted { .. } => "tasks-application-harps-exhausted",
            AppError::Vanished { .. } => "tasks-application-vanished",
            AppError::NoProject => "tasks-application-no-project",
            AppError::NoGoLog { .. } => "tasks-application-no-go-log",
            AppError::UnknownStatusClass { .. } => "tasks-application-unknown-status-class",
            AppError::UnknownSort { .. } => "tasks-application-unknown-sort",
            AppError::NotFound { .. } => "tasks-application-not-found",
            AppError::Ambiguous { .. } => "tasks-application-ambiguous",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            AppError::Rejected(cause)
            | AppError::TagRefused(cause)
            | AppError::Failed(cause)
            | AppError::Identity(cause)
            | AppError::Configuration(cause)
            | AppError::TagQuery(cause)
            | AppError::Prune(cause)
            | AppError::Priority(cause) => cause.args(),
            AppError::GaveUp { attempts, error } => vec![
                ("attempts", (*attempts).into()),
                ("error", Arg::nested(error)),
            ],
            AppError::HarpsExhausted { attempts } => vec![("attempts", (*attempts).into())],
            AppError::Vanished { harp_id } => vec![("harp_id", harp_id.as_str().into())],
            AppError::NoProject => Vec::new(),
            AppError::NoGoLog { project } => vec![("project", project.as_str().into())],
            AppError::UnknownStatusClass { status } => vec![
                ("status", status.as_str().into()),
                ("classes", crate::read::classes().into()),
            ],
            AppError::UnknownSort { sort } => vec![("sort", sort.as_str().into())],
            AppError::NotFound { harp_ids } => vec![
                ("harp_ids", quoted(harp_ids).into()),
                ("count", harp_ids.len().into()),
            ],
            AppError::Ambiguous { held } => {
                vec![("held", held_in(held).into()), ("count", held.len().into())]
            }
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "tasks-application",
    include_str!("../locales/en-US/tasks-application.ftl"),
);

/// The en-US catalogs of every diagnostic a use case returns but its
/// adapters': the domain's rejections and this crate's errors.
pub const CATALOGS: [(&str, &str); 3] = [tasks::EN_US, task_priority::EN_US, EN_US];

/// One value of every error this crate defines, of every reason a tag is
/// refused, and of every reason an import gives for leaving a record behind (not the wrappers, whose codes
/// are their causes'), for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let tags = crate::tags::samples().into_iter();
    let schema = crate::schema::samples().into_iter();
    let mut samples: Vec<Box<dyn Diagnostic>> = vec![
        Box::new(AppError::GaveUp {
            attempts: 10,
            error: Report::new("x-busy", "busy"),
        }),
        Box::new(AppError::HarpsExhausted { attempts: 100 }),
        Box::new(AppError::Vanished {
            harp_id: "swift-amber".to_string(),
        }),
        Box::new(AppError::NoProject),
        Box::new(AppError::NoGoLog {
            project: "jolly-mute-magma".to_string(),
        }),
        Box::new(AppError::UnknownStatusClass {
            status: "@done".to_string(),
        }),
        Box::new(AppError::UnknownSort {
            sort: "age".to_string(),
        }),
        Box::new(AppError::NotFound {
            harp_ids: vec!["swift-amber".to_string(), "calm-otter".to_string()],
        }),
        Box::new(AppError::Ambiguous {
            held: vec![(
                "swift-amber".to_string(),
                vec!["jolly-mute-magma".to_string(), "brisk-tall-oak".to_string()],
            )],
        }),
        Box::new(crate::Why::Displaced),
        Box::new(crate::Why::HarpTaken),
        Box::new(crate::Why::TaskNotImported),
    ];
    samples.extend(tags.map(|t| Box::new(t) as Box<dyn Diagnostic>));
    samples.extend(schema.map(|s| Box::new(s) as Box<dyn Diagnostic>));
    samples.extend(
        crate::lint::samples()
            .into_iter()
            .map(|f| Box::new(f) as Box<dyn Diagnostic>),
    );
    samples.extend(
        crate::prune::samples()
            .into_iter()
            .map(|p| Box::new(p) as Box<dyn Diagnostic>),
    );
    samples.extend(
        crate::priority::samples()
            .into_iter()
            .map(|n| Box::new(n) as Box<dyn Diagnostic>),
    );
    samples.push(Box::new(crate::QueryError {
        query: "and".to_string(),
        postfix: "x".to_string(),
        infix: "x".to_string(),
    }));
    samples
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
