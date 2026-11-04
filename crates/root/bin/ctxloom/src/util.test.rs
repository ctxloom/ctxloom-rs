//! Go's `cli/util.feature` (release/0.7), ported (S9): `util config-write`,
//! which merges a JSON patch into a foreign config file. Each test names its
//! scenario.
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

// util.feature: Scenario: A third write still replaces ctxloom's entry after
// one field changed
#[test]
fn test_a_third_write_still_replaces_ctxloom_entry_after_one_field_changed() {
    let project = Project::initialized();
    project.write("vendor-tool.json", VENDOR);

    write(&project, "/first/ctxloom").succeeds();
    project.file_contains("vendor-tool.json", "/first/ctxloom");

    write(&project, "/second/ctxloom").succeeds();
    project.file_contains("vendor-tool.json", "/second/ctxloom");

    write(&project, "/third/ctxloom").succeeds();
    project.file_contains("vendor-tool.json", "/third/ctxloom");

    project
        .file_lacks("vendor-tool.json", "/first/ctxloom")
        .file_lacks("vendor-tool.json", "/second/ctxloom")
        .file_counts("vendor-tool.json", "/third/ctxloom", 1)
        .file_contains("vendor-tool.json", "https://example.com/vendor.schema.json")
        .file_contains("vendor-tool.json", "keep-me-verbatim")
        .file_contains("vendor-tool.json", "vendor-server")
        .file_contains("vendor-tool.json", "--serve");
    // The args array was replaced, not appended to (util.feature's comment on
    // the hew `Sel.Set` defect).
    let doc = project.json_file("vendor-tool.json");
    assert_eq!(
        doc["servers"]["ctxloom"]["args"],
        serde_json::json!(["mcp", "serve"])
    );
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
