//! The `Tasks` port over the taskloom command line (S8): ctxloom keeps no
//! tasks or plans, and taskloom stays a separate program, so the Deferred
//! tasks are read by running
//! `taskloom tasks list --status Deferred --format json --project <id>` and
//! parsing what it prints. The program is `TASKLOOM_BIN` from the injected
//! environment, else `taskloom` on that environment's `PATH`; the run is
//! bounded in time and the program given exactly that environment.
//!
//! taskloom's list is an array of task objects (`harp_id`, `text`, `status`,
//! `checked`, `text_hash`, `trigger`, `origin_session`, `tags`, `created_at`,
//! `derived_priority`, and `deferred_since` where this taskloom emits it);
//! only the fields a `DeferredTask` holds are read, the rest ignored. A
//! failing taskloom prints `{code, args, message}` on standard error.

use std::ffi::OsString;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use angzarr_diagnostic::Report;
use process_env::EnvSnapshot;
use serde::Deserialize;
use tasks_ports::{DeferredTask, Tasks, TasksError};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

mod process;

use process::{output_within, Unfinished};

/// The variable naming the taskloom program.
pub const BIN_VAR: &str = "TASKLOOM_BIN";

/// How long one taskloom run may take.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Asks taskloom, for one project.
#[derive(Clone, Debug)]
pub struct TaskloomCli {
    program: OsString,
    project: String,
    timeout: Duration,
    env: Arc<EnvSnapshot>,
}

impl TaskloomCli {
    /// A client for `project` (taskloom's `--project`), running the program
    /// `env` names (`TASKLOOM_BIN`, else `taskloom`), with `env` as its
    /// environment.
    pub fn new(env: Arc<EnvSnapshot>, project: impl Into<String>) -> TaskloomCli {
        let program = env
            .non_empty(BIN_VAR)
            .map_or_else(|| OsString::from("taskloom"), OsString::from);
        TaskloomCli {
            program,
            project: project.into(),
            timeout: DEFAULT_TIMEOUT,
            env,
        }
    }

    /// The same client with `timeout` as the limit on a run.
    pub fn with_timeout(mut self, timeout: Duration) -> TaskloomCli {
        self.timeout = timeout;
        self
    }

    fn name(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }
}

impl Tasks for TaskloomCli {
    fn deferred(&self) -> Result<Vec<DeferredTask>, TasksError> {
        let program = self.name();
        let mut command = Command::new(&self.program);
        command
            .args(["tasks", "list", "--status", "Deferred", "--format", "json"])
            .args(["--project", &self.project])
            .env_clear()
            .envs(self.env.vars());
        let output = match output_within(&mut command, self.timeout) {
            Ok(output) => output,
            Err(Unfinished::TimedOut) => {
                return Err(TasksError::TimedOut {
                    program,
                    timeout: format!("{:?}", self.timeout),
                })
            }
            Err(Unfinished::Failed(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(TasksError::NotFound { program })
            }
            Err(Unfinished::Failed(e)) => {
                return Err(TasksError::Unrunnable {
                    program,
                    why: e.to_string(),
                })
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            let exit = output.status.to_string();
            return Err(match envelope(&stderr) {
                Some(cause) => TasksError::Refused {
                    program,
                    exit,
                    cause,
                },
                None => TasksError::Failed {
                    program,
                    exit,
                    stderr: stderr.trim().to_string(),
                },
            });
        }
        parse(&output.stdout).map_err(|why| TasksError::Unparsable { program, why })
    }
}

/// One task as taskloom prints it; fields not held here are not read.
#[derive(Deserialize)]
struct TaskJson {
    harp_id: String,
    text: String,
    #[serde(default)]
    trigger: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    deferred_since: Option<String>,
}

fn parse(stdout: &[u8]) -> Result<Vec<DeferredTask>, String> {
    let tasks: Vec<TaskJson> = serde_json::from_slice(stdout).map_err(|e| e.to_string())?;
    tasks
        .into_iter()
        .map(|task| {
            let deferred_since = task
                .deferred_since
                .as_deref()
                .map(|at| {
                    since_epoch(at)
                        .map_err(|e| format!("{}: deferred_since {at:?}: {e}", task.harp_id))
                })
                .transpose()?;
            Ok(DeferredTask {
                harp_id: task.harp_id,
                text: task.text,
                trigger: task.trigger.unwrap_or_default(),
                tags: task.tags,
                deferred_since,
            })
        })
        .collect()
}

/// An RFC 3339 time as the time since the Unix epoch.
fn since_epoch(at: &str) -> Result<Duration, String> {
    let at = OffsetDateTime::parse(at, &Rfc3339).map_err(|e| e.to_string())?;
    Duration::try_from(at - OffsetDateTime::UNIX_EPOCH)
        .map_err(|_| "before the Unix epoch".to_string())
}

/// taskloom's error envelope in `stderr`, as a diagnostic: its code and args
/// (a value that is not text as its JSON) and its own message. taskloom may
/// say other things before it (`taskloom: project x`), so the envelope is the
/// whole of standard error, or what follows the first line that opens one.
fn envelope(stderr: &str) -> Option<Report> {
    #[derive(Deserialize)]
    struct Envelope {
        code: String,
        #[serde(default)]
        args: serde_json::Map<String, serde_json::Value>,
        #[serde(default)]
        message: String,
    }
    let start = if stderr.trim_start().starts_with('{') {
        0
    } else {
        stderr.find("\n{")? + 1
    };
    let envelope: Envelope = serde_json::from_str(&stderr[start..]).ok()?;
    let mut report = Report::new(envelope.code);
    for (name, value) in envelope.args {
        let text = match value {
            serde_json::Value::String(text) => text,
            other => other.to_string(),
        };
        report = report.arg(name, text);
    }
    Some(report.arg("message", envelope.message))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "real.test.rs"]
mod real_tests;
