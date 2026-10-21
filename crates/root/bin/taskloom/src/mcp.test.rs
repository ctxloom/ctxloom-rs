//! Acceptance: `taskloom mcp`, the real binary in a temporary home, served
//! over its stdin and stdout as an MCP client speaks to it: initialize
//! and its capabilities, the tools and resources Go offers (their
//! descriptions and schemas Go's), a call of every tool, a read of each
//! resource, and the command line reading the same store.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde_json::{json, Value};

use super::{binary, Env};

/// Go taskloom's capabilities, tools and resources as its initialize,
/// tools/list and resources/list answer them, and its tag-schema
/// resource with nothing configured, as `scripts/go-companion-fixtures.sh`
/// extracted them from the Go source. Rust serves Go's tools with the
/// reviewed factual changes applied (`reviewed`).
const GO_CAPABILITIES: &str =
    include_str!("../../../../adapters/driving/tasks-mcp/fixtures/go/capabilities.json");
#[path = "../../../../adapters/driving/tasks-mcp/fixtures/reviewed.rs"]
mod reviewed;
const GO_RESOURCES: &str =
    include_str!("../../../../adapters/driving/tasks-mcp/fixtures/go/resources.json");
const GO_TAG_SCHEMA: &str =
    include_str!("../../../../adapters/driving/tasks-mcp/fixtures/go/tag-schema.json");

/// A running `taskloom mcp`, spoken to one JSON-RPC message per line.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    next_id: u64,
}

impl Server {
    fn start(env: &Env) -> Server {
        Server::start_in(env, env.project.path())
    }

    /// `taskloom mcp` started in `dir`.
    fn start_in(env: &Env, dir: &Path) -> Server {
        let mut child = Command::new(binary())
            .arg("mcp")
            .current_dir(dir)
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

    /// Initialize, as a client begins; the server's answer.
    fn initialize(&mut self) -> Value {
        let initialized = self.ask(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "acceptance", "version": "0"},
            }),
        );
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        initialized
    }

    /// The text of the resource at `uri`, read as JSON.
    fn read(&mut self, uri: &str) -> String {
        let answer = self.ask("resources/read", json!({"uri": uri}));
        let contents = &answer["result"]["contents"];
        assert_eq!(contents[0]["uri"], uri, "{answer}");
        assert_eq!(contents[0]["mimeType"], "application/json", "{answer}");
        contents[0]["text"].as_str().expect("text").to_string()
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
fn test_mcp_offers_gos_capabilities_tools_and_resources_over_stdio() {
    let env = Env::new("mcp-test-harp");
    let mut server = Server::start(&env);

    let initialized = server.initialize();
    let info = &initialized["result"];
    assert_eq!(info["serverInfo"]["name"], "taskloom", "{initialized}");
    assert_eq!(info["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        info["instructions"].as_str() == Some(reviewed::instructions().as_str()),
        "the instructions differ from Go's, reviewed: {initialized}"
    );
    let go_capabilities: Value = serde_json::from_str(GO_CAPABILITIES).unwrap();
    assert_eq!(info["capabilities"], go_capabilities, "{initialized}");

    let listed = server.ask("tools/list", json!({}));
    let go_tools: Value = serde_json::from_str(&reviewed::tools()).unwrap();
    assert_eq!(
        listed["result"]["tools"], go_tools,
        "the tools are Go's, reviewed"
    );
    let resources = server.ask("resources/list", json!({}));
    let go_resources: Value = serde_json::from_str(GO_RESOURCES).unwrap();
    assert_eq!(
        resources["result"]["resources"], go_resources,
        "the resources are Go's"
    );

    let schema = server.read("taskloom://tag-schema");
    assert!(
        schema == GO_TAG_SCHEMA,
        "the tag schema differs from Go's:\n{schema}"
    );
    let (ok, stderr) = server.finish();
    assert!(ok, "{stderr}");
}

#[test]
fn test_mcp_serves_every_tool_and_the_vocabulary_on_the_projects_store() {
    let env = Env::new("mcp-test-harp");
    let mut server = Server::start(&env);
    server.initialize();

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
    let harp = task["harp_id"].clone();
    server.call(
        "task_add",
        json!({"text": "Done already", "status": "Done"}),
    );

    let moved = server.call(
        "task_set_status",
        json!({"harp_id": harp, "status": "In Progress"}),
    );
    assert_eq!(moved["task"]["status"], "In Progress");
    assert_eq!(moved["project_id"], project_id);
    let edited = server.call(
        "task_edit",
        json!({"harp_id": harp, "text": "Ship it all\nwith the rest"}),
    );
    assert_eq!(edited["task"]["text"], "Ship it all\nwith the rest");
    let tagged = server.call(
        "task_tag",
        json!({"harp_id": harp, "add": ["triage:kind=defect"], "remove": ["b"]}),
    );
    assert_eq!(tagged["task"]["tags"], json!(["a", "triage:kind=defect"]));

    lists(&mut server, &harp, project_id);
    let vocabulary: Value =
        serde_json::from_str(&server.read("taskloom://tag-vocabulary")).unwrap();
    assert_eq!(vocabulary["project_id"], project_id);
    assert_eq!(
        vocabulary["tags"],
        json!([
            {"tag": "a", "active": 1, "total": 1},
            {"tag": "triage:kind=defect", "active": 1, "total": 1},
        ])
    );

    let (ok, stderr) = server.finish();
    assert!(
        ok,
        "taskloom mcp exits 0 when its client goes away: {stderr}"
    );

    // The command line reads the same store.
    let cli = env.list();
    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0]["harp_id"], harp);
    assert_eq!(cli[0]["status"], "In Progress");
    assert_eq!(cli[0]["tags"], json!(["a", "triage:kind=defect"]));
}

/// task_list over the project's two tasks, `harp` In Progress and one
/// Done: the default view, a narrowed and compact one with its summary,
/// and every project.
fn lists(server: &mut Server, harp: &Value, project_id: &str) {
    let listing = server.call("task_list", json!({}));
    let rows = listing["tasks"].as_array().expect("tasks");
    assert_eq!(rows.len(), 1, "{listing}");
    assert_eq!(rows[0]["harp_id"], *harp);
    assert_eq!(rows[0]["project_id"], project_id);
    assert_eq!(listing["project_id"], project_id);
    assert_eq!(listing["hidden_completed"], 1);
    let narrowed = server.call(
        "task_list",
        json!({
            "term": "SHIP", "tag_query": "triage:kind=defect", "compact": true,
            "include_summary": true, "limit": 1, "statuses": ["@open"],
        }),
    );
    assert_eq!(narrowed["tasks"], json!(null), "{narrowed}");
    assert_eq!(narrowed["compact_tasks"][0]["headline"], "Ship it all");
    assert_eq!(
        narrowed["summary"]["counts"],
        json!({"Done": 1, "In Progress": 1})
    );
    let every = server.call(
        "task_list",
        json!({"global": true, "include_completed": true}),
    );
    assert_eq!(every["global"], true, "{every}");
    assert_eq!(every["project_count"], 1);
    assert_eq!(every["tasks"].as_array().map(Vec::len), Some(2));
}

// Go's no-project fallback: outside every work root a listing reads every
// project and says why, in the words the command line uses on stderr.
#[test]
fn test_mcp_lists_every_project_outside_any_work_root_as_the_cli_does() {
    let env = Env::new("mcp-test-harp");
    env.seed("in the project");
    let stray = tempfile::tempdir().unwrap();
    let mut server = Server::start_in(&env, stray.path());
    server.initialize();
    let listing = server.call("task_list", json!({}));
    assert_eq!(listing["global"], true, "{listing}");
    assert_eq!(listing["project_count"], 1);
    assert_eq!(listing["tasks"][0]["text"], "in the project");
    let notice = listing["notice"].as_str().expect("a notice").to_string();
    let (ok, stderr) = server.finish();
    assert!(ok, "{stderr}");

    let cli = env.run_in(stray.path(), &["list", "--json"], &[]);
    assert!(cli.ok, "{}", cli.stderr);
    assert_eq!(
        cli.stderr,
        format!("taskloom: {notice}\n"),
        "one notice, both edges"
    );
}

// Each call's service is configured as the command line's is: the home's
// tag schema refuses a tag outside its enum, as a tool error carrying the
// envelope, and the configured hiding leaves the JSON every tag.
#[test]
fn test_mcp_calls_are_served_under_the_configured_tag_schema() {
    let env = Env::new("mcp-test-harp");
    let dir = env.home.path().join(".taskloom");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("config.yaml"),
        "tag_schema:\n  - 'tagma.enum:\"team:size\"=\"small,large\"'\n  - 'tagma.hide:\"team:size\"=true'\n",
    )
    .unwrap();
    let mut server = Server::start(&env);
    server.initialize();

    let refused = server.ask(
        "tools/call",
        json!({"name": "task_add", "arguments": {"text": "Ship it", "tags": ["team:size=huge"]}}),
    );
    let result = &refused["result"];
    assert_eq!(result["isError"], true, "{refused}");
    let envelope: Value =
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        envelope["code"], "tasks-application-tag-not-in-enum",
        "{envelope}"
    );

    let added = server.call(
        "task_add",
        json!({"text": "Ship it", "tags": ["team:size=small"]}),
    );
    assert_eq!(added["task"]["tags"], json!(["team:size=small"]));
    let schema: Value = serde_json::from_str(&server.read("taskloom://tag-schema")).unwrap();
    let declared = schema["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["target"] == "team:size")
        .cloned();
    assert_eq!(
        declared,
        Some(json!({"target": "team:size", "enum": ["small", "large"]})),
        "the resource reads the configured schema"
    );
    let vocabulary: Value =
        serde_json::from_str(&server.read("taskloom://tag-vocabulary")).unwrap();
    assert_eq!(vocabulary["tags"], json!([]), "the hidden tag is left out");
    let (ok, stderr) = server.finish();
    assert!(ok, "{stderr}");
}
