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

#[path = "../fixtures/reviewed.rs"]
mod reviewed;

/// Go taskloom's capabilities as initialize answers them, its tools and
/// resources as tools/list and resources/list answer them, and what its
/// tag-schema resource reads with nothing configured, as
/// `scripts/go-companion-fixtures.sh` extracted them from the Go source
/// (`cmd/taskloom/mcp.go`, `mcp_tools.go`, `mcp_resources.go`); run it
/// again to regenerate them. Rust serves Go's instructions and tools with
/// the reviewed factual changes applied (`reviewed`).
const GO_CAPABILITIES: &str = include_str!("../fixtures/go/capabilities.json");
const GO_TOOLS: &str = include_str!("../fixtures/go/tools.json");
const GO_RESOURCES: &str = include_str!("../fixtures/go/resources.json");
const GO_TAG_SCHEMA: &str = include_str!("../fixtures/go/tag-schema.json");

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
fn test_the_instructions_are_gos_reviewed_byte_for_byte() {
    let reviewed = reviewed::instructions();
    assert!(
        INSTRUCTIONS == reviewed,
        "the instructions differ from Go's, reviewed"
    );
    run(async {
        let (_client, initialized) = connect(Arc::default(), settings()).await;
        let result = &initialized["result"];
        assert!(
            result["instructions"].as_str() == Some(reviewed.as_str()),
            "initialize's instructions differ from Go's, reviewed"
        );
        assert_eq!(result["serverInfo"]["name"], "taskloom");
        assert_eq!(result["serverInfo"]["version"], "1.2.3");
        assert_eq!(result["protocolVersion"], "2025-06-18");
    });
}

// A reviewed change applies to exactly one place in Go's text: once Go
// rewords it, or says it twice, the change is reviewed again.
#[test]
fn test_a_reviewed_change_must_match_gos_text_exactly_once() {
    assert_eq!(reviewed::applied("a b c", &[("b", "x")]), "a x c");
    for go in ["a c", "b b"] {
        let refused = std::panic::catch_unwind(|| reviewed::applied(go, &[("b", "x")]));
        assert!(refused.is_err(), "{go:?}");
    }
}

#[test]
fn test_the_capabilities_are_gos() {
    let go: Value = serde_json::from_str(GO_CAPABILITIES).unwrap();
    run(async {
        let (_client, initialized) = connect(Arc::default(), settings()).await;
        assert_eq!(initialized["result"]["capabilities"], go, "{initialized}");
    });
}

// Go advertises logging, so a client may set its level; nothing is
// logged at any.
#[test]
fn test_a_logging_level_is_accepted() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let set = client
            .ask("logging/setLevel", json!({"level": "debug"}))
            .await;
        assert_eq!(set["result"], json!({}), "{set}");
    });
}

#[test]
fn test_the_tools_are_gos_five_reviewed_byte_for_byte() {
    let reviewed = reviewed::tools();
    assert!(
        super::TOOLS == reviewed,
        "the tools differ from Go's, reviewed"
    );
    let go: Vec<Value> = serde_json::from_str(&reviewed).unwrap();
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let listed = client.ask("tools/list", json!({})).await;
        let tools = listed["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), go.len(), "every tool Go offers");
        for tool in tools {
            let gos = go
                .iter()
                .find(|g| g["name"] == tool["name"])
                .expect("Go has the tool");
            assert!(
                tool["description"].as_str() == gos["description"].as_str(),
                "{}'s description differs from Go's, reviewed",
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
fn test_arguments_outside_the_schema_are_tool_errors() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let missing = client.call("task_add", json!({})).await;
        assert_eq!(envelope(&missing)["code"], "tasks-mcp-arguments");
        let unknown = client.call("task_list", json!({"colour": "red"})).await;
        assert_eq!(envelope(&unknown)["code"], "tasks-mcp-arguments");
        let wrong = client.call("task_tag", json!({"harp_id": 3})).await;
        assert_eq!(envelope(&wrong)["args"]["tool"], "task_tag");
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
fn test_a_pinned_project_reaches_every_call() {
    // The pin is passed to the identity port; the fake resolves any pin
    // the same, so what is checked is the request each call makes.
    let pinned = Settings {
        project: Some("pinned-proj-id".to_string()),
        ..settings()
    };
    let pin = Some("pinned-proj-id");
    let request = crate::tools::list_request(json!({}), &pinned).unwrap();
    assert_eq!(request.query.project.as_deref(), pin);
    let task = crate::tools::new_task(json!({"text": "x"}), &pinned).unwrap();
    assert_eq!(task.project.as_deref(), pin);
    let arguments = json!({"harp_id": "h", "status": "Done"});
    let change = crate::tools::status_change(arguments, &pinned).unwrap();
    assert_eq!(change.project.as_deref(), pin);
    let edit = crate::tools::text_edit(json!({"harp_id": "h", "text": "x"}), &pinned).unwrap();
    assert_eq!(edit.project.as_deref(), pin);
    let tag = crate::tools::tag_change(json!({"harp_id": "h", "add": ["a"]}), &pinned).unwrap();
    assert_eq!(tag.project.as_deref(), pin);
    let vocabulary = crate::resources::vocabulary_query(&pinned);
    assert_eq!(
        vocabulary,
        tasks_application::ListQuery {
            project: pin.map(str::to_string),
            ..tasks_application::ListQuery::default()
        }
    );
}

/// A world with a project here.
fn bound() -> Arc<World> {
    Arc::new(World {
        bound: true,
        ..World::default()
    })
}

/// Whether `value` fits `schema`, as far as Go's output schemas say: a
/// type (or several), an object's properties (no others where
/// `additionalProperties` is false) and those it requires, an array's
/// items, a map's values.
fn conforms(value: &Value, schema: &Value, at: &str) {
    let types: Vec<&str> = match &schema["type"] {
        Value::String(one) => vec![one.as_str()],
        Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    assert!(
        types.is_empty()
            || types.contains(&kind)
            || (kind == "integer" && types.contains(&"number")),
        "{at}: a {kind}, where the schema says {types:?}"
    );
    match value {
        Value::Object(fields) => {
            let properties = schema["properties"].as_object();
            for (name, field) in fields {
                match properties.and_then(|p| p.get(name)) {
                    Some(property) => conforms(field, property, &format!("{at}.{name}")),
                    None => match &schema["additionalProperties"] {
                        Value::Bool(false) => panic!("{at}: {name} is not in the schema"),
                        Value::Object(_) => conforms(
                            field,
                            &schema["additionalProperties"],
                            &format!("{at}.{name}"),
                        ),
                        _ => {}
                    },
                }
            }
            for required in schema["required"].as_array().into_iter().flatten() {
                let name = required.as_str().unwrap();
                assert!(fields.contains_key(name), "{at}: {name} is required");
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                conforms(item, &schema["items"], &format!("{at}[{i}]"));
            }
        }
        _ => {}
    }
}

/// `tool`'s output schema, Go's.
fn output_schema(tool: &str) -> Value {
    let go: Vec<Value> = serde_json::from_str(GO_TOOLS).unwrap();
    go.into_iter()
        .find(|t| t["name"] == tool)
        .map(|t| t["outputSchema"].clone())
        .expect("Go has the tool")
}

/// A call's structured result, held to Go's output schema for the tool.
async fn answered(client: &mut Client, tool: &str, arguments: Value) -> Value {
    let result = client.call(tool, arguments).await;
    let value = structured(&result).clone();
    conforms(&value, &output_schema(tool), tool);
    value
}

#[test]
fn test_a_status_change_answers_the_task_moved() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        let added = answered(&mut client, "task_add", json!({"text": "Ship it"})).await;
        let harp = added["task"]["harp_id"].clone();
        let moved = answered(
            &mut client,
            "task_set_status",
            json!({"harp_id": harp, "status": "Deferred", "trigger": "the v2 API ships"}),
        )
        .await;
        assert_eq!(moved["task"]["status"], "Deferred");
        assert_eq!(moved["task"]["trigger"], "the v2 API ships");
        assert_eq!(moved["project_id"], "jolly-mute-magma");
        assert_eq!(moved["path"], added["path"]);
        let done = answered(
            &mut client,
            "task_set_status",
            json!({"harp_id": harp, "status": "Done"}),
        )
        .await;
        assert_eq!(done["task"]["checked"], true);
    });
}

#[test]
fn test_an_edit_answers_the_task_with_its_text_replaced() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        let added = answered(
            &mut client,
            "task_add",
            json!({"text": "Ship it", "status": "In Progress"}),
        )
        .await;
        let harp = added["task"]["harp_id"].clone();
        let edited = answered(
            &mut client,
            "task_edit",
            json!({"harp_id": harp, "text": "Ship it all"}),
        )
        .await;
        assert_eq!(edited["task"]["text"], "Ship it all");
        assert_eq!(
            edited["task"]["status"], "In Progress",
            "the status untouched"
        );
        assert_eq!(
            edited["task"]["text_hash"],
            tasks_application::text_hash("Ship it all")
        );
    });
}

#[test]
fn test_a_tag_answers_the_task_with_its_tags_added_then_removed() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        let added = answered(
            &mut client,
            "task_add",
            json!({"text": "Ship it", "tags": ["old", "triage:kind=chore"]}),
        )
        .await;
        let harp = added["task"]["harp_id"].clone();
        let tagged = answered(
            &mut client,
            "task_tag",
            json!({"harp_id": harp, "add": ["new", "both", "triage:kind=defect"], "remove": ["old", "both"]}),
        )
        .await;
        assert_eq!(
            tagged["task"]["tags"],
            json!(["new", "triage:kind=defect"]),
            "a scalar's new value displaces the old; a tag in both lists ends up removed"
        );
    });
}

// Each write's refusal is a tool error with the diagnostic's envelope, as
// task_add's is.
#[test]
fn test_a_refused_change_is_a_tool_error_carrying_its_envelope() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        let added = answered(&mut client, "task_add", json!({"text": "Ship it"})).await;
        let harp = added["task"]["harp_id"].clone();
        let refusals = [
            (
                "task_set_status",
                json!({"harp_id": harp, "status": "Deferred"}),
                "tasks-trigger-required",
            ),
            (
                "task_edit",
                json!({"harp_id": "no-such-harp", "text": "x"}),
                "tasks-no-task",
            ),
            ("task_tag", json!({"harp_id": harp}), "tasks-tag-required"),
            (
                "task_tag",
                json!({"harp_id": harp, "add": ["triage:kind=sparkles"]}),
                "tasks-application-tag-not-in-enum",
            ),
        ];
        for (tool, arguments, code) in refusals {
            let refused = client.call(tool, arguments).await;
            assert_eq!(envelope(&refused)["code"], code, "{tool}: {refused}");
            assert!(refused.get("structuredContent").is_none(), "{refused}");
        }
    });
}

// A change where there is no project here changes nothing and says why.
#[test]
fn test_a_change_where_there_is_no_project_is_refused() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let refused = client
            .call(
                "task_set_status",
                json!({"harp_id": "swift-amber", "status": "Done"}),
            )
            .await;
        assert_eq!(
            envelope(&refused)["code"],
            "tasks-application-no-project",
            "{refused}"
        );
    });
}

/// Three tasks added: one To Do tagged `urgent`, one In Progress, one
/// Done tagged `urgent`.
async fn seeded(client: &mut Client) {
    for (text, status, tags) in [
        ("Ship the alpha", "To Do", json!(["urgent"])),
        ("Write the beta\nmore", "In Progress", json!([])),
        ("Ship the gamma", "Done", json!(["urgent"])),
    ] {
        let arguments = json!({"text": text, "status": status, "tags": tags});
        answered(client, "task_add", arguments).await;
    }
}

fn texts(listing: &Value, field: &str, key: &str) -> Vec<String> {
    listing[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row[key].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn test_a_listing_filters_by_term_and_tag_query_and_says_what_it_hid() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        seeded(&mut client).await;
        let shipped = answered(&mut client, "task_list", json!({"term": "SHIP"})).await;
        assert_eq!(texts(&shipped, "tasks", "text"), ["Ship the alpha"]);
        assert_eq!(shipped["hidden_completed"], 1);
        let urgent = answered(
            &mut client,
            "task_list",
            json!({"tag_query": "urgent", "include_completed": true}),
        )
        .await;
        assert_eq!(
            texts(&urgent, "tasks", "text"),
            ["Ship the alpha", "Ship the gamma"]
        );
        assert_eq!(urgent.get("hidden_completed"), None);
        let open = answered(&mut client, "task_list", json!({"statuses": ["@open"]})).await;
        assert_eq!(texts(&open, "tasks", "text").len(), 2);
    });
}

#[test]
fn test_a_listing_is_compact_and_limited_when_asked() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        seeded(&mut client).await;
        let listed = answered(
            &mut client,
            "task_list",
            json!({"compact": true, "limit": 1}),
        )
        .await;
        assert_eq!(listed["tasks"], json!(null));
        assert_eq!(
            texts(&listed, "compact_tasks", "headline"),
            ["Ship the alpha"]
        );
        assert_eq!(listed["omitted_by_limit"], 1);
        assert_eq!(listed["hidden_completed"], 1);
    });
}

// The summary counts every task wherever the listing read, whatever it
// filtered.
#[test]
fn test_a_listing_with_its_summary_counts_every_task() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        seeded(&mut client).await;
        let listed = answered(
            &mut client,
            "task_list",
            json!({"include_summary": true, "term": "alpha"}),
        )
        .await;
        assert_eq!(texts(&listed, "tasks", "text"), ["Ship the alpha"]);
        let summary = &listed["summary"];
        assert_eq!(
            summary["counts"],
            json!({"Done": 1, "In Progress": 1, "To Do": 1})
        );
        assert_eq!(
            summary["in_progress"][0]["project"],
            "/work/tree (jolly-mute-magma)"
        );
    });
}

#[test]
fn test_a_listing_sorts_by_priority_and_by_nothing_else() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        let priority = client.call("task_list", json!({"sort": "priority"})).await;
        assert_eq!(structured(&priority)["tasks"], json!([]));
        let unknown = client.call("task_list", json!({"sort": "age"})).await;
        assert_eq!(envelope(&unknown)["code"], "tasks-application-unknown-sort");
        let query = client
            .call("task_list", json!({"tag_query": "a/and/and"}))
            .await;
        assert_eq!(envelope(&query)["code"], "tasks-application-tag-query");
    });
}

#[test]
fn test_a_global_listing_reads_every_project_and_needs_no_notice() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        seeded(&mut client).await;
        let listed = answered(&mut client, "task_list", json!({"global": true})).await;
        assert_eq!(listed["global"], true);
        assert_eq!(listed["project_count"], 1);
        assert_eq!(listed["path"], "");
        assert_eq!(listed.get("notice"), None);
        assert_eq!(
            texts(&listed, "tasks", "project_id"),
            ["jolly-mute-magma"; 2]
        );
    });
}

// Go's fallback: outside every work root, with no project of its own,
// a listing reads every project, and says why.
#[test]
fn test_a_listing_outside_every_work_root_reads_every_project_and_says_why() {
    run(async {
        let world = Arc::new(World {
            rootless: Some("/tmp/somewhere".to_string()),
            ..World::default()
        });
        let (mut client, _) = connect(world, settings()).await;
        seeded(&mut client).await;
        let listed = answered(&mut client, "task_list", json!({"include_summary": true})).await;
        assert_eq!(listed["global"], true);
        assert_eq!(listed["project_count"], 1);
        assert!(
            listed["notice"]
                .as_str()
                .is_some_and(|n| n.contains("/tmp/somewhere") && n.contains("every project")),
            "{listed}"
        );
        assert_eq!(listed.get("warning"), None);
        assert_eq!(texts(&listed, "tasks", "text").len(), 2);
        assert_eq!(
            listed["summary"]["counts"]["Done"], 1,
            "the summary read every project too"
        );
    });
}

#[test]
fn test_task_list_where_there_is_no_project_in_a_work_root_lists_nothing() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let listed = answered(&mut client, "task_list", json!({"global": false})).await;
        assert_eq!(listed["tasks"], json!([]));
        assert_eq!(listed.get("global"), None);
    });
}

// ------------------------------------------------------------ the resources

async fn read(client: &mut Client, uri: &str) -> Value {
    client.ask("resources/read", json!({"uri": uri})).await
}

/// A resource's one content, checked to be JSON text at its URI.
fn content(answer: &Value, uri: &str) -> String {
    let contents = answer["result"]["contents"].as_array().expect("contents");
    assert_eq!(contents.len(), 1, "{answer}");
    assert_eq!(contents[0]["uri"], uri);
    assert_eq!(contents[0]["mimeType"], "application/json");
    contents[0]["text"].as_str().unwrap().to_string()
}

#[test]
fn test_the_resources_are_gos() {
    let go: Value = serde_json::from_str(GO_RESOURCES).unwrap();
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let listed = client.ask("resources/list", json!({})).await;
        assert_eq!(listed["result"]["resources"], go, "{listed}");
    });
}

#[test]
fn test_the_tag_schema_reads_as_gos_byte_for_byte() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let answer = read(&mut client, "taskloom://tag-schema").await;
        let text = content(&answer, "taskloom://tag-schema");
        assert!(
            text == GO_TAG_SCHEMA,
            "the tag schema differs from Go's:\n{text}"
        );
    });
}

#[test]
fn test_the_vocabulary_reads_the_tags_in_use_now() {
    run(async {
        let (mut client, _) = connect(bound(), settings()).await;
        let empty = read(&mut client, "taskloom://tag-vocabulary").await;
        let doc: Value =
            serde_json::from_str(&content(&empty, "taskloom://tag-vocabulary")).unwrap();
        assert_eq!(
            doc,
            json!({"project_id": "jolly-mute-magma", "project_dir": "/work/tree", "tags": []})
        );
        seeded(&mut client).await;
        let read_again = read(&mut client, "taskloom://tag-vocabulary").await;
        let doc: Value =
            serde_json::from_str(&content(&read_again, "taskloom://tag-vocabulary")).unwrap();
        assert_eq!(
            doc["tags"],
            json!([{"tag": "urgent", "active": 1, "total": 2}])
        );
    });
}

#[test]
fn test_an_unknown_resource_is_a_protocol_error_carrying_its_envelope() {
    run(async {
        let (mut client, _) = connect(Arc::default(), settings()).await;
        let answer = read(&mut client, "taskloom://frob").await;
        let error = &answer["error"];
        assert_eq!(error["code"], -32002, "{answer}");
        assert_eq!(error["message"], "no resource is at taskloom://frob");
        assert_eq!(error["data"]["code"], "tasks-mcp-unknown-resource");
    });
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
