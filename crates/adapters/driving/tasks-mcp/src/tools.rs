//! Each tool's arguments translated into a use case's request, and its
//! answer into Go taskloom's result shape.

use presentation::Catalog;
use rmcp::model::Tool;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tasks_application::{ListQuery, Listing, NewTask, Project, Task, Written};
use tasks_presentation::TaskJson;

use crate::{McpError, McpNotice, Settings, TOOLS};

pub const TASK_LIST: &str = "task_list";
pub const TASK_ADD: &str = "task_add";

/// The tools `TOOLS` describes.
pub fn offered() -> Vec<Tool> {
    serde_json::from_str(TOOLS).expect("TOOLS is a JSON array of tools")
}

/// `task_list`'s arguments: Go's `taskListInput`. Only `statuses` and
/// `include_completed` are served yet; the rest are accepted only when
/// they ask for nothing.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ListArgs {
    statuses: Option<Vec<String>>,
    include_completed: bool,
    term: String,
    tag_query: String,
    include_summary: bool,
    global: bool,
    sort: String,
    compact: bool,
    limit: i64,
}

impl ListArgs {
    /// The first argument given that this server cannot serve yet.
    fn unserved(&self) -> Option<&'static str> {
        [
            ("term", !self.term.is_empty()),
            ("tag_query", !self.tag_query.is_empty()),
            ("include_summary", self.include_summary),
            ("global", self.global),
            ("sort", !self.sort.is_empty()),
            ("compact", self.compact),
            ("limit", self.limit != 0),
        ]
        .into_iter()
        .find_map(|(name, given)| given.then_some(name))
    }
}

/// `task_add`'s arguments: Go's `taskAddInput`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddArgs {
    text: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    trigger: String,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

fn arguments<T: DeserializeOwned>(tool: &str, arguments: Value) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|e| McpError::Arguments {
        tool: tool.to_string(),
        detail: e.to_string(),
    })
}

/// The listing `task_list`'s `arguments` ask for.
pub fn list_query(arguments: Value, settings: &Settings) -> Result<ListQuery, McpError> {
    let args: ListArgs = self::arguments(TASK_LIST, arguments)?;
    if let Some(parameter) = args.unserved() {
        return Err(McpError::Unserved {
            tool: TASK_LIST.to_string(),
            parameter: parameter.to_string(),
        });
    }
    Ok(ListQuery {
        project: settings.project.clone(),
        statuses: args.statuses.unwrap_or_default(),
        all: args.include_completed,
        tag_query: String::new(),
        config_set: Vec::new(),
    })
}

/// The task `task_add`'s `arguments` ask for, added in the server's
/// session.
pub fn new_task(arguments: Value, settings: &Settings) -> Result<NewTask, McpError> {
    let args: AddArgs = self::arguments(TASK_ADD, arguments)?;
    Ok(NewTask {
        project: settings.project.clone(),
        text: args.text,
        status: args.status,
        trigger: args.trigger,
        tags: args.tags.unwrap_or_default(),
        origin_session: settings.session.clone(),
        config_set: Vec::new(),
    })
}

/// A task as a tool answers it: Go's `tasks.Task`, with its
/// `derived_priority`, which is null until priority is ported, as Go's is
/// when a listing is not sorted by it.
#[derive(Serialize)]
struct ToolTask<'a> {
    #[serde(flatten)]
    task: TaskJson<'a>,
    derived_priority: Option<f64>,
}

impl<'a> ToolTask<'a> {
    fn of(task: &'a Task) -> ToolTask<'a> {
        ToolTask {
            task: TaskJson::of(task),
            derived_priority: None,
        }
    }
}

/// A listed task: Go's `taskRow`, the task and the project it is in.
#[derive(Serialize)]
struct Row<'a> {
    #[serde(flatten)]
    task: ToolTask<'a>,
    project_id: &'a str,
    project_dir: &'a str,
}

/// `task_list`'s result: Go's `taskListResult`, so far as it is served.
/// `path` is the project's store, and is empty, with a `warning` saying
/// so, where there is no project yet.
#[derive(Serialize)]
struct ListResult<'a> {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_dir: Option<&'a str>,
    tasks: Vec<Row<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
}

/// `task_add`'s result: Go's `taskAddResult`, its `path` the project's
/// store.
#[derive(Serialize)]
struct AddResult<'a> {
    path: String,
    task: ToolTask<'a>,
    project_id: &'a str,
    project_dir: &'a str,
}

fn store_path(project: &Project) -> String {
    project.store.display().to_string()
}

/// `listing` as `task_list` answers it.
pub fn listed(listing: &Listing, catalog: &Catalog) -> Value {
    let project = listing.project.as_ref();
    let rows = project
        .map(|project| {
            listing
                .tasks
                .iter()
                .map(|task| Row {
                    task: ToolTask::of(task),
                    project_id: &project.id,
                    project_dir: &project.root,
                })
                .collect()
        })
        .unwrap_or_default();
    let result = ListResult {
        path: project.map(store_path).unwrap_or_default(),
        project_id: project.map(|p| p.id.as_str()),
        project_dir: project.map(|p| p.root.as_str()),
        tasks: rows,
        warning: project
            .is_none()
            .then(|| catalog.message(&McpNotice::NoProject)),
    };
    serde_json::to_value(result).unwrap_or_default()
}

/// `written` as `task_add` answers it.
pub fn added(written: &Written) -> Value {
    let result = AddResult {
        path: store_path(&written.project),
        task: ToolTask::of(&written.task),
        project_id: &written.project.id,
        project_dir: &written.project.root,
    };
    serde_json::to_value(result).unwrap_or_default()
}

#[cfg(test)]
#[path = "tools.test.rs"]
mod tests;
