//! The server over an in-memory pipe, spoken to in JSON-RPC as a client
//! speaks to it over stdio, serving the real use cases over fake ports.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{
    AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines, ReadHalf, WriteHalf,
};
use tokio::sync::oneshot;

use super::{Settings, TaskTools, INSTRUCTIONS};
use crate::fakes::World;

/// Go taskloom's MCP instructions and its tools as its tools/list answers
/// them, as `scripts/go-companion-fixtures.sh` extracted them from the Go
/// source (`cmd/taskloom/mcp.go`, `cmd/taskloom/mcp_tools.go`); run it
/// again to regenerate them.
const GO_INSTRUCTIONS: &str = include_str!("../fixtures/go/instructions.txt");
const GO_TOOLS: &str = include_str!("../fixtures/go/tools.json");

fn catalog() -> presentation::Catalog {
    let mut sources = vec![crate::EN_US];
    sources.extend(tasks_application::CATALOGS);
    presentation::Catalog::en_us(&sources).expect("the catalog loads")
}

fn settings() -> Settings {
    Settings {
        project: None,
        session: "calm-otter-fox".to_string(),
        name: "taskloom".to_string(),
        version: "1.2.3".to_string(),
    }
}

/// A client's end of the pipe, speaking one JSON-RPC message per line.
struct Client {
    to_server: WriteHalf<DuplexStream>,
    from_server: Lines<BufReader<ReadHalf<DuplexStream>>>,
    next_id: u64,
}

impl Client {
    async fn send(&mut self, message: Value) {
        let mut line = message.to_string();
        line.push('\n');
        self.to_server.write_all(line.as_bytes()).await.unwrap();
    }

    /// The next message from the server, failing rather than hanging when
    /// none comes.
    async fn recv(&mut self) -> Value {
        let line = tokio::time::timeout(Duration::from_secs(10), self.from_server.next_line())
            .await
            .expect("the server answers")
            .unwrap()
            .expect("the server is still there");
        serde_json::from_str(&line).unwrap()
    }

    /// Send a request; its id.
    async fn request(&mut self, method: &str, params: Value) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        id
    }

    /// A request's answer, the next message.
    async fn ask(&mut self, method: &str, params: Value) -> Value {
        let id = self.request(method, params).await;
        let answer = self.recv().await;
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }

    async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self
            .ask("tools/call", json!({"name": tool, "arguments": arguments}))
            .await;
        answer["result"].clone()
    }
}

/// The server over `world`, initialized by a client; the client's answer
/// to initialize too.
async fn connect(world: Arc<World>, settings: Settings) -> (Client, Value) {
    let (client_end, server_end) = tokio::io::duplex(1 << 20);
    let tools = TaskTools::new(move || world.service(), settings, Arc::new(catalog()));
    tokio::spawn(tools.serve(server_end));
    let (from_server, to_server) = tokio::io::split(client_end);
    let mut client = Client {
        to_server,
        from_server: BufReader::new(from_server).lines(),
        next_id: 0,
    };
    let initialized = client
        .ask(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"},
            }),
        )
        .await;
    client
        .send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .await;
    (client, initialized)
}

fn run(test: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(test);
}

fn structured(result: &Value) -> &Value {
    assert_eq!(result["isError"], false, "{result}");
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        text, result["structuredContent"],
        "the text is the structured result"
    );
    &result["structuredContent"]
}

/// A tool error's envelope, from its text.
fn envelope(result: &Value) -> Value {
    assert_eq!(result["isError"], true, "{result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

// ------------------------------------------------------------ parity with Go

#[test]
fn test_the_instructions_are_gos_byte_for_byte() {
    assert!(
        INSTRUCTIONS == GO_INSTRUCTIONS,
        "the instructions differ from Go's"
    );
    run(async {
        let (_client, initialized) = connect(Arc::default(), settings()).await;
        let result = &initialized["result"];
        assert!(
            result["instructions"].as_str() == Some(GO_INSTRUCTIONS),
            "initialize's instructions differ from Go's"
        );
        assert_eq!(result["serverInfo"]["name"], "taskloom");
        assert_eq!(result["serverInfo"]["version"], "1.2.3");
        assert_eq!(result["protocolVersion"], "2025-06-18");
        assert!(result["capabilities"]["tools"].is_object(), "{result}");
    });
}

#[test]
fn test_the_tools_are_gos_task_add_and_task_list_byte_for_byte() {
    let go: Vec<Value> = serde_json::from_str(GO_TOOLS).unwrap();
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let listed = client.ask("tools/list", json!({})).await;
        let tools = listed["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["task_add", "task_list"]);
        for tool in tools {
            let gos = go
                .iter()
                .find(|g| g["name"] == tool["name"])
                .expect("Go has the tool");
            assert!(
                tool["description"].as_str() == gos["description"].as_str(),
                "{}'s description differs from Go's",
                tool["name"]
            );
            assert_eq!(tool["inputSchema"], gos["inputSchema"], "{}", tool["name"]);
            assert_eq!(
                tool["outputSchema"], gos["outputSchema"],
                "{}",
                tool["name"]
            );
            assert_eq!(tool, gos, "nothing else differs");
        }
    });
}

// ------------------------------------------------------------ the tools

#[test]
fn test_task_add_answers_the_task_in_gos_shape() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let result = client
            .call("task_add", json!({"text": "Ship it", "tags": ["b", "a"]}))
            .await;
        assert_eq!(
            structured(&result),
            &json!({
                "path": "/home/.ctxloom/projects/jolly-mute-magma/state.db",
                "project_id": "jolly-mute-magma",
                "project_dir": "/work/tree",
                "task": {
                    "harp_id": "swift-amber",
                    "text": "Ship it",
                    "status": "To Do",
                    "checked": false,
                    "text_hash": tasks_application::text_hash("Ship it"),
                    "trigger": "",
                    "origin_session": "calm-otter-fox",
                    "tags": ["a", "b"],
                    "created_at": "2026-06-10T02:35:20.837009562Z",
                    "derived_priority": null,
                },
            })
        );
    });
}

#[test]
fn test_task_list_answers_each_task_with_its_project() {
    run(async {
        let world = Arc::new(World {
            bound: true,
            ..World::default()
        });
        let (mut client, _) = connect(world, settings()).await;
        let listed = client.call("task_list", json!({})).await;
        assert_eq!(
            structured(&listed),
            &json!({
                "path": "/home/.ctxloom/projects/jolly-mute-magma/state.db",
                "project_id": "jolly-mute-magma",
                "project_dir": "/work/tree",
                "tasks": [],
            })
        );
    });
}

#[test]
fn test_task_list_where_there_is_no_project_says_so_and_lists_nothing() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let listed = client.call("task_list", json!({"statuses": null})).await;
        assert_eq!(
            structured(&listed),
            &json!({
                "path": "",
                "tasks": [],
                "warning": "no project here yet: nothing to list",
            })
        );
    });
}

#[test]
fn test_a_refused_add_is_a_tool_error_carrying_its_envelope() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let refused = client
            .call("task_add", json!({"text": "Later", "status": "Deferred"}))
            .await;
        let envelope = envelope(&refused);
        assert_eq!(envelope["code"], "tasks-trigger-required", "{envelope}");
        assert!(
            envelope["message"].as_str().unwrap().contains("trigger"),
            "{envelope}"
        );
        assert!(refused.get("structuredContent").is_none(), "{refused}");
    });
}

// task_add's tags go through the application, as the CLI's do: one tagma
// does not parse is refused there, and nothing is written.
#[test]
fn test_an_add_with_a_tag_outside_the_grammar_is_a_tool_error_carrying_its_envelope() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let refused = client
            .call(
                "task_add",
                json!({"text": "Ship it", "tags": ["ok", "not ok"]}),
            )
            .await;
        let envelope = envelope(&refused);
        assert_eq!(
            envelope["code"], "tasks-application-tag-unparseable",
            "{envelope}"
        );
        assert!(
            envelope["message"].as_str().unwrap().contains("not ok"),
            "{envelope}"
        );
        assert!(refused.get("structuredContent").is_none(), "{refused}");
    });
}

#[test]
fn test_arguments_outside_the_schema_and_parameters_not_served_are_tool_errors() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let missing = client.call("task_add", json!({})).await;
        assert_eq!(envelope(&missing)["code"], "tasks-mcp-arguments");
        let unknown = client.call("task_list", json!({"colour": "red"})).await;
        assert_eq!(envelope(&unknown)["code"], "tasks-mcp-arguments");
        let unserved = client.call("task_list", json!({"term": "ship"})).await;
        assert_eq!(
            envelope(&unserved),
            json!({
                "code": "tasks-mcp-unserved",
                "args": {"tool": "task_list", "parameter": "term"},
                "message": "task_list: term is not served yet",
            })
        );
    });
}

#[test]
fn test_an_unknown_tool_is_a_protocol_error_carrying_its_envelope() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let answer = client
            .ask("tools/call", json!({"name": "task_frob", "arguments": {}}))
            .await;
        let error = &answer["error"];
        assert_eq!(error["code"], -32602, "{answer}");
        assert_eq!(error["message"], "no tool is named task_frob");
        assert_eq!(error["data"]["code"], "tasks-mcp-unknown-tool");
    });
}

#[test]
fn test_no_resources_are_served_yet() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let listed = client.ask("resources/list", json!({})).await;
        assert_eq!(listed["result"]["resources"], json!([]), "{listed}");
    });
}

#[test]
fn test_a_pinned_project_reaches_every_call() {
    // The pin is passed to the identity port; the fake resolves any pin
    // the same, so what is checked is the request each call makes.
    let pinned = Settings {
        project: Some("pinned-proj-id".to_string()),
        ..settings()
    };
    let query = crate::tools::list_query(json!({}), &pinned).unwrap();
    assert_eq!(query.project.as_deref(), Some("pinned-proj-id"));
    let task = crate::tools::new_task(json!({"text": "x"}), &pinned).unwrap();
    assert_eq!(task.project.as_deref(), Some("pinned-proj-id"));
}

// ------------------------------------------------------------ concurrency

/// One add is held inside its retry: its first commit conflicted, and the
/// wait before it tries again is held open. A listing called meanwhile
/// answers, so the server does not serialize its calls; then the add,
/// released, answers too.
#[test]
fn test_a_call_answers_while_another_waits_inside_an_adds_retry() {
    run(async {
        let (begun_tx, begun) = oneshot::channel();
        let (release, release_rx) = oneshot::channel();
        let world = Arc::new(World {
            bound: true,
            conflicts: 1,
            hold: std::sync::Mutex::new(Some((begun_tx, release_rx))),
            ..World::default()
        });
        let (mut client, _) = connect(Arc::clone(&world), settings()).await;

        let add = client
            .request(
                "tools/call",
                json!({"name": "task_add", "arguments": {"text": "Held"}}),
            )
            .await;
        begun.await.expect("the add waits inside its retry");

        let list = client
            .request("tools/call", json!({"name": "task_list", "arguments": {}}))
            .await;
        let first = client.recv().await;
        assert_eq!(first["id"], list, "the listing answers first: {first}");
        assert_eq!(first["result"]["isError"], false, "{first}");

        release.send(()).unwrap();
        let second = client.recv().await;
        assert_eq!(second["id"], add, "{second}");
        assert_eq!(
            structured(&second["result"])["task"]["text"],
            "Held",
            "the add commits on its retry"
        );
        assert_eq!(*world.made.lock().unwrap(), 2, "a service for each call");
    });
}

#[test]
fn test_a_project_is_resolved_off_the_servers_thread() {
    run(async {
        let world = Arc::new(World::default());
        let (mut client, _) = connect(Arc::clone(&world), settings()).await;
        client.call("task_add", json!({"text": "x"})).await;
        client.call("task_list", json!({})).await;
        let asked_on = world.asked_on.lock().unwrap().clone();
        // The add's project; the listing's, and, finding none, whether
        // the working directory lies outside every work root.
        assert_eq!(asked_on.len(), 3);
        let here = std::thread::current().id();
        assert!(
            asked_on.iter().all(|thread| *thread != here),
            "the registry's lock is waited for on a blocking thread, never the server's"
        );
    });
}
