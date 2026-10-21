//! What a command tells the person besides its answer, on stderr: never
//! on stdout, which stays one JSON document.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::Project;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    /// The project a write went to.
    Project { root: String, id: String },
    /// A listing found no project here, so lists nothing.
    NoProject,
    /// A read found no project in `directory`, which lies in no work
    /// root, so it read every project.
    EveryProject { directory: String },
    /// A search's matches the default view hid: `completed` terminal,
    /// `deferred` Deferred.
    Hidden {
        hidden: usize,
        completed: usize,
        deferred: usize,
    },
    /// The tasks a limit left out.
    Omitted { omitted: usize },
}

impl Notice {
    pub fn project(project: &Project) -> Notice {
        Notice::Project {
            root: project.root.clone(),
            id: project.id.clone(),
        }
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Notice::Project { root, id } => write!(f, "project {root} ({id})"),
            Notice::NoProject => f.write_str("no project here yet: nothing to list"),
            Notice::EveryProject { directory } => write!(
                f,
                "no project here: {directory} is in no git repository, has no CTXLOOM_ROOT and no project of its own, so this reads every project; pass --project <id>, or work in a project, to read one"
            ),
            Notice::Hidden {
                hidden,
                completed,
                deferred,
            } => write!(
                f,
                "{hidden} more matching tasks hidden by the default view ({completed} completed, {deferred} deferred): add --all to include them"
            ),
            Notice::Omitted { omitted } => write!(
                f,
                "{omitted} more tasks omitted by --limit: raise or drop it to see them (summary's counts are unaffected)"
            ),
        }
    }
}

impl Diagnostic for Notice {
    fn code(&self) -> &'static str {
        match self {
            Notice::Project { .. } => "tasks-cli-project",
            Notice::NoProject => "tasks-cli-no-project",
            Notice::EveryProject { .. } => "tasks-cli-every-project",
            Notice::Hidden { .. } => "tasks-cli-hidden",
            Notice::Omitted { .. } => "tasks-cli-omitted",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            Notice::Project { root, id } => {
                vec![("root", root.as_str().into()), ("id", id.as_str().into())]
            }
            Notice::NoProject => Vec::new(),
            Notice::EveryProject { directory } => vec![("directory", directory.as_str().into())],
            Notice::Hidden {
                hidden,
                completed,
                deferred,
            } => vec![
                ("hidden", (*hidden).into()),
                ("completed", (*completed).into()),
                ("deferred", (*deferred).into()),
            ],
            Notice::Omitted { omitted } => vec![("omitted", (*omitted).into())],
        }
    }
}
