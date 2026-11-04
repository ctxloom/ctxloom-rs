//! Plain tests for what util.feature does not cover (the feature runs in
//! `gherkin_surfaces.test.rs`): `util config-write`'s record reversal and
//! its refusals, from Go's help text and the feature's opening paragraph.
//!
//! plan supersedes Go (8.2, "`util config-write` ... keep byte-for-byte"):
//! ctxloom-rs reads and writes JSON documents (what every foreign file in
//! Go's acceptance corpus is); a YAML or TOML target is refused by name until
//! those formats are ported.

use super::library::Project;

const VENDOR: &str = "{\n  \"$schema\": \"https://example.com/vendor.schema.json\",\n  \"servers\": {\n    \"vendor-owned\": {\n      \"command\": \"vendor-server\",\n      \"args\": [\"--serve\"],\n      \"env\": {\"VENDOR_TOKEN\": \"keep-me-verbatim\"}\n    }\n  }\n}\n";

fn write(project: &Project, command: &str) -> super::Ran {
    let file = project.dir().join("vendor-tool.json");
    project.run_input(
        &[
            "util",
            "config-write",
            "--file",
            file.to_str().expect("utf-8"),
        ],
        &format!(
            r#"{{"servers": {{"ctxloom": {{"command": "{command}", "args": ["mcp", "serve"]}}}}}}"#
        ),
        &[],
    )
}

// util.feature: "backs the file up before touching it, refuses rather than
// overwrite a file it cannot parse"
#[test]
fn test_config_write_refuses_a_file_it_cannot_parse_and_leaves_it_alone() {
    let project = Project::initialized();
    project.write("vendor-tool.json", "{ this is not json");
    write(&project, "/first/ctxloom").fails();
    assert_eq!(
        project.read("vendor-tool.json").as_deref(),
        Some("{ this is not json")
    );
}

// util.feature (the opening paragraph): "ctxloom records what it applied, and
// the NEXT write reverses that record before applying the new set" -- so a
// second write that leaves a key out takes the key the first added back out,
// and the vendor's own entry is not disturbed.
#[test]
fn test_a_write_takes_back_what_the_previous_write_added() {
    let project = Project::initialized();
    project.write("vendor-tool.json", VENDOR);
    let file = project.dir().join("vendor-tool.json");
    let file = file.to_str().expect("utf-8");
    project
        .run_input(
            &["util", "config-write", "--file", file],
            r#"{"servers": {"ctxloom": {"command": "/first/ctxloom", "env": {"A": "1"}}}}"#,
            &[],
        )
        .succeeds();
    project
        .run_input(
            &["util", "config-write", "--file", file],
            r#"{"servers": {"ctxloom": {"command": "/second/ctxloom"}}}"#,
            &[],
        )
        .succeeds();
    let doc = project.json_file("vendor-tool.json");
    assert_eq!(doc["servers"]["ctxloom"]["command"], "/second/ctxloom");
    assert!(doc["servers"]["ctxloom"].get("env").is_none(), "{doc}");
    assert_eq!(
        doc["servers"]["vendor-owned"]["env"]["VENDOR_TOKEN"],
        "keep-me-verbatim"
    );
}

// util.feature (long help, rule 3): a missing file starts from an empty
// object, and the result says so.
#[test]
fn test_config_write_creates_a_missing_file_and_reports_it() {
    let project = Project::initialized();
    let file = project.dir().join("new-tool.json");
    let ran = project.run_input(
        &[
            "util",
            "config-write",
            "--file",
            file.to_str().expect("utf-8"),
        ],
        r#"{"a": {"b": 1}}"#,
        &[],
    );
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(doc["created"], true);
    assert_eq!(doc["verified"], true);
    assert_eq!(doc["mergedKeys"], serde_json::json!(["a"]));
    assert_eq!(project.json_file("new-tool.json")["a"]["b"], 1);
}

// util.feature (long help, rule 1): --file must be absolute.
#[test]
fn test_config_write_refuses_a_relative_path() {
    let project = Project::initialized();
    project
        .run_input(
            &["util", "config-write", "--file", "vendor-tool.json"],
            r#"{"a": 1}"#,
            &[],
        )
        .fails()
        .contains("absolute");
}
