//! Acceptance: `taskloom mcp`, the real binary in a temporary home, served
//! over its stdin and stdout as an MCP client speaks to it: initialize,
//! the tools Go offers (task_add and task_list so far, their schemas
//! Go's), an add, a listing, and the resources (none until step 5).

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{json, Value};

use super::{binary, Env};

/// Go taskloom's tools as its tools/list answers them, as
/// `scripts/go-companion-fixtures.sh` extracted them from the Go source.
const GO_TOOLS: &str =
    include_str!("../../../../adapters/driving/tasks-mcp/fixtures/go/tools.json");

/// A running `taskloom mcp`, spoken to one JSON-RPC message per line.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: u64,
}

impl Server {
    fn start(env: &Env) -> Server {
        let mut child = Command::new(binary())
            .arg("mcp")
            .current_dir(env.project.path())
            .env_clear()
            .env("HOME", env.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("CTXLOOM_SESSION_HARP", env.session)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("taskloom mcp starts");
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
        Server {
            stdin: child.stdin.take(),
            child,
            lines,
            next_id: 0,
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin still open");
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    /// The answer to `method`, failing rather than hanging when none comes.
    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let line = self
            .lines
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or_else(|e| panic!("no answer to {method}: {e}"));
        let answer: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("stdout carries only JSON-RPC ({e}): {line:?}"));
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    /// A tool's structured result, which its text repeats.
    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self.ask("tools/call", json!({"name": tool, "arguments": arguments}));
        let result = &answer["result"];
        assert_eq!(result["isError"], false, "{answer}");
        let text: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(text, result["structuredContent"]);
        text
    }

    /// Close stdin, as a client going away does; the exit status, and
    /// what was written to stderr.
    fn finish(mut self) -> (bool, String) {
        drop(self.stdin.take());
        let output = self.child.wait_with_output().expect("taskloom mcp exits");
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

#[test]
fn test_mcp_serves_gos_tools_over_stdio_on_the_projects_store() {
    let env = Env::new("mcp-test-harp");
    let mut server = Server::start(&env);

    let initialized = server.ask(
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "acceptance", "version": "0"},
        }),
    );
    let info = &initialized["result"];
    assert_eq!(info["serverInfo"]["name"], "taskloom", "{initialized}");
    assert_eq!(info["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        info["instructions"]
            .as_str()
            .is_some_and(|i| i.starts_with("Per-project task tracking.")),
        "{initialized}"
    );
    server.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let listed = server.ask("tools/list", json!({}));
    let go: Vec<Value> = serde_json::from_str(GO_TOOLS).unwrap();
    let tools = listed["result"]["tools"].as_array().expect("tools");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, ["task_add", "task_list"]);
    for tool in tools {
        let gos = go.iter().find(|g| g["name"] == tool["name"]).unwrap();
        assert_eq!(tool["inputSchema"], gos["inputSchema"], "{}", tool["name"]);
        assert_eq!(tool, gos, "{} is Go's", tool["name"]);
    }

    let added = server.call("task_add", json!({"text": "Ship it", "tags": ["b", "a"]}));
    let task = &added["task"];
    assert_eq!(task["text"], "Ship it");
    assert_eq!(task["status"], "To Do");
    assert_eq!(task["tags"], json!(["a", "b"]));
    assert_eq!(task["origin_session"], "mcp-test-harp");
    let project_id = added["project_id"]
        .as_str()
        .expect("the project written to");
    assert!(
        added["path"]
            .as_str()
            .is_some_and(|p| p.ends_with(&format!(".ctxloom/projects/{project_id}/state.db"))),
        "{added}"
    );

    let listing = server.call("task_list", json!({}));
    let rows = listing["tasks"].as_array().expect("tasks");
    assert_eq!(rows.len(), 1, "{listing}");
    assert_eq!(rows[0]["harp_id"], task["harp_id"]);
    assert_eq!(rows[0]["project_id"], project_id);
    assert_eq!(listing["project_id"], project_id);

    let resources = server.ask("resources/list", json!({}));
    assert_eq!(resources["result"]["resources"], json!([]), "{resources}");

    let (ok, stderr) = server.finish();
    assert!(
        ok,
        "taskloom mcp exits 0 when its client goes away: {stderr}"
    );

    // The command line reads the same store.
    let cli = env.list();
    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0]["harp_id"], task["harp_id"]);
}
