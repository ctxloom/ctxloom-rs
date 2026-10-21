//! Acceptance: `taskloom watch`, the real binary in a temporary home: its
//! first line at once, a line for a task another process adds, one JSON
//! document a line, and a clean exit at SIGINT or SIGTERM with no process
//! left behind. The test waits on the stream's lines (each wait bounded,
//! so a stream that never answers fails rather than hangs), never on a
//! sleep.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use serde_json::Value;

use super::{binary, one_document, Env};

/// How long a line may take before the stream is judged not to answer.
const PATIENCE: Duration = Duration::from_secs(30);

/// A running `taskloom watch`, killed if a test ends with it still running.
pub(super) struct Watching {
    child: Option<Child>,
    lines: Receiver<String>,
}

impl Watching {
    pub(super) fn start(env: &Env, args: &[&str]) -> Watching {
        let mut child = Command::new(binary())
            .arg("watch")
            .args(args)
            .current_dir(env.project.path())
            .env_clear()
            .env("HOME", env.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("taskloom watch starts");
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Watching {
            child: Some(child),
            lines,
        }
    }

    /// The stream's next line.
    pub(super) fn line(&self) -> String {
        self.lines
            .recv_timeout(PATIENCE)
            .unwrap_or_else(|e| panic!("no line from watch: {e}"))
    }

    /// Send `signal` and wait for the process to end; whatever it wrote
    /// after the lines already read must be nothing.
    pub(super) fn stop(mut self, signal: &str) -> ExitStatus {
        let mut child = self.child.take().unwrap();
        let sent = Command::new("kill")
            .args([&format!("-{signal}"), &child.id().to_string()])
            .status()
            .expect("kill runs");
        assert!(sent.success(), "kill -{signal}");
        let status = child.wait().expect("watch ends");
        match self.lines.recv_timeout(PATIENCE) {
            Err(RecvTimeoutError::Disconnected) => {}
            other => panic!("watch wrote more, or its stdout stayed open: {other:?}"),
        }
        status
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Go's line, for `project`.
fn event(project: &str) -> Value {
    serde_json::json!({"event": "changed", "kind": "tasks", "project": project})
}

#[test]
fn test_watch_writes_a_line_at_once_and_one_for_a_task_another_process_adds() {
    let env = Env::new("watch-harp");
    env.seed("Before");
    let shown = env.run(&["project", "show", "--json"]);
    let project = one_document(&shown.stdout)["project_id"]
        .as_str()
        .expect("the project")
        .to_string();

    let watching = Watching::start(&env, &[]);
    // A subscriber renders the current state from the first line, so it
    // comes before any change; one JSON document a line.
    assert_eq!(one_document(&watching.line()), event(&project));

    let added = env.run(&["add", "After"]);
    assert!(added.ok, "{}", added.stderr);
    assert_eq!(one_document(&watching.line()), event(&project));

    let status = watching.stop("INT");
    assert!(status.success(), "SIGINT is a clean end: {status}");
}

#[test]
fn test_watch_ends_cleanly_at_sigterm_and_watches_a_pinned_project() {
    let env = Env::new("watch-term-harp");
    env.seed("Seed");
    let shown = env.run(&["project", "show", "--json"]);
    let project = one_document(&shown.stdout)["project_id"]
        .as_str()
        .unwrap()
        .to_string();
    let elsewhere = Env::new("watch-term-elsewhere");
    let watching = Watching::start(&elsewhere, &["--project", &project, "--json"]);
    assert_eq!(one_document(&watching.line()), event(&project));
    let status = watching.stop("TERM");
    assert!(status.success(), "SIGTERM is a clean end: {status}");
}

#[test]
fn test_watch_where_there_is_no_project_says_so_and_binds_none() {
    let env = Env::new("watch-none-harp");
    let ran = env.run(&["watch", "--format", "text"]);
    assert!(!ran.ok);
    assert_eq!(ran.stdout, "");
    assert_eq!(
        ran.stderr,
        "taskloom: no project here yet; add a task first (taskloom add \"<task text>\")\n"
    );
    assert!(
        !env.home.path().join(".ctxloom").exists(),
        "a watch binds no project and creates nothing"
    );

    let ran = env.run(&["watch", "--json"]);
    assert!(!ran.ok);
    assert_eq!(ran.stdout, "", "an error leaves stdout empty");
    assert_eq!(one_document(&ran.stderr)["code"], "tasks-watch-no-project");
}
