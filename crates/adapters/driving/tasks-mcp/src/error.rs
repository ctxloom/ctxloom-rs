//! Why a tool call did not answer, and what a result says besides its
//! answer; and this crate's catalog.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

#[derive(Clone, PartialEq, Debug)]
pub enum McpError {
    /// The use case failed: its error's diagnostic.
    App(AppError),
    /// A tool's arguments do not fit its input schema.
    Arguments { tool: String, detail: String },
    /// No tool has the name called.
    UnknownTool { name: String },
    /// No resource has the URI read.
    UnknownResource { uri: String },
    /// The call's thread stopped before it answered (it panicked).
    Crashed { detail: String },
    /// The server could not start, or stopped other than by its client
    /// going away.
    Serve { detail: String },
}

/// What a result says besides its answer, in its `warning`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum McpNotice {
    /// A read found no project here, so reads nothing.
    NoProject,
    /// A read found no project in `directory`, which lies outside every
    /// work root, so read every project instead.
    EveryProject { directory: String },
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::App(error) => error.fmt(f),
            McpError::Arguments { tool, detail } => {
                write!(f, "invalid arguments to {tool}: {detail}")
            }
            McpError::UnknownTool { name } => write!(f, "no tool is named {name}"),
            McpError::UnknownResource { uri } => write!(f, "no resource is at {uri}"),
            McpError::Crashed { detail } => {
                write!(f, "the call stopped before it answered: {detail}")
            }
            McpError::Serve { detail } => write!(f, "the MCP server stopped: {detail}"),
        }
    }
}

impl fmt::Display for McpNotice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpNotice::NoProject => f.write_str("no project here yet: nothing to list"),
            McpNotice::EveryProject { directory } => write!(
                f,
                "no project here: {directory} is in no git repository, has no CTXLOOM_ROOT \
                 and no project of its own, so this reads every project; pass --project \
                 <id>, or work in a project, to read one"
            ),
        }
    }
}

impl std::error::Error for McpError {}

impl Diagnostic for McpError {
    fn code(&self) -> &'static str {
        match self {
            McpError::App(error) => error.code(),
            McpError::Arguments { .. } => "tasks-mcp-arguments",
            McpError::UnknownTool { .. } => "tasks-mcp-unknown-tool",
            McpError::UnknownResource { .. } => "tasks-mcp-unknown-resource",
            McpError::Crashed { .. } => "tasks-mcp-crashed",
            McpError::Serve { .. } => "tasks-mcp-serve",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            McpError::App(error) => error.args(),
            McpError::Arguments { tool, detail } => {
                vec![
                    ("tool", tool.as_str().into()),
                    ("detail", detail.as_str().into()),
                ]
            }
            McpError::UnknownTool { name } => vec![("name", name.as_str().into())],
            McpError::UnknownResource { uri } => vec![("uri", uri.as_str().into())],
            McpError::Crashed { detail } | McpError::Serve { detail } => {
                vec![("detail", detail.as_str().into())]
            }
        }
    }
}

impl Diagnostic for McpNotice {
    fn code(&self) -> &'static str {
        match self {
            McpNotice::NoProject => "tasks-mcp-no-project",
            McpNotice::EveryProject { .. } => "tasks-mcp-every-project",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        match self {
            McpNotice::NoProject => Vec::new(),
            McpNotice::EveryProject { directory } => {
                vec![("directory", directory.as_str().into())]
            }
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = ("tasks-mcp", include_str!("../locales/en-US/tasks-mcp.ftl"));

/// One value of every error and notice this crate defines (not the
/// wrapper), for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(McpError::Arguments {
            tool: "task_add".to_string(),
            detail: "missing field `text`".to_string(),
        }),
        Box::new(McpError::UnknownTool {
            name: "task_frob".to_string(),
        }),
        Box::new(McpError::UnknownResource {
            uri: "taskloom://frob".to_string(),
        }),
        Box::new(McpError::Crashed {
            detail: "task 7 panicked".to_string(),
        }),
        Box::new(McpError::Serve {
            detail: "connection closed: initialize request".to_string(),
        }),
        Box::new(McpNotice::NoProject),
        Box::new(McpNotice::EveryProject {
            directory: "/tmp/somewhere".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
