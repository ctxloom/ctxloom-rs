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
        }
    }
}

impl Diagnostic for Notice {
    fn code(&self) -> &'static str {
        match self {
            Notice::Project { .. } => "tasks-cli-project",
            Notice::NoProject => "tasks-cli-no-project",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            Notice::Project { root, id } => {
                vec![("root", root.as_str().into()), ("id", id.as_str().into())]
            }
            Notice::NoProject => Vec::new(),
        }
    }
}
