//! The `loadout` command (S2): ctxloom is its own companion, and `ctxloom
//! loadout --format yaml` prints the loadout document a session consumes
//! (`cmd/ctxloom/loadout.go`, `internal/adapters/cli/loadout_cmd.go` at
//! release/0.7 ca14d6046). YAML is its one fixed output (D28): every other
//! `--format` is refused the way Go refuses it.
//!
//! The loadout's prose is the owner's (D24) and names MCP tools whose cut
//! list is open (`agent_run`, D8), so this build prints the structure and
//! withholds the owner's text: the description, the MCP server's notes and
//! the always-on `isolation-axes` fragment. The tests below pin that the
//! withheld text is not shipped; copying it byte for byte from ca14d6046 is
//! the owner's call once the cut list is known.

use super::library::Project;

// cmd/ctxloom/loadout_test.go TestLoadout_YAML_IsAValidLoadout (documented
// condition): the document has a `run:` bundle with ctxloom's own MCP entry,
// served by the running session's endpoint, with nothing to launch.
#[test]
fn test_the_loadout_is_a_loadout_document_with_ctxloom_served_by_the_session_endpoint() {
    let project = Project::initialized();
    let ran = project.run(&["loadout", "--format", "yaml"]);
    ran.succeeds();
    let doc = angzarr_yaml::parse(&ran.stdout).expect("the loadout parses");
    let run = doc.get("run").expect("a run: bundle");
    assert_eq!(
        run.get("version").and_then(angzarr_yaml::Value::as_str),
        Some("1.0.0")
    );
    assert_eq!(
        run.get("author").and_then(angzarr_yaml::Value::as_str),
        Some("ctxloom")
    );
    let entry = run
        .get("mcp")
        .and_then(|m| m.get("ctxloom"))
        .expect("ctxloom's own MCP entry");
    assert_eq!(
        entry.get("served_by").and_then(angzarr_yaml::Value::as_str),
        Some("session-endpoint")
    );
    assert!(
        entry.get("command").is_none(),
        "a dynamic entry names no command"
    );
    assert!(entry.get("args").is_none());
    assert!(
        doc.get("init").is_none(),
        "ctxloom declares no INIT loadout today"
    );
}

// loadout_cmd.go (documented condition): the command carries its own --format,
// yaml by default; the same document either way.
#[test]
fn test_the_loadout_is_yaml_by_default_and_the_same_document_either_way() {
    let project = Project::initialized();
    let bare = project.run(&["loadout"]);
    bare.succeeds();
    let flagged = project.run(&["loadout", "--format", "yaml"]);
    assert_eq!(bare.stdout, flagged.stdout);
    // It needs no project: a loadout is a probe's answer, read before there
    // is a project.
    let empty = Project::bare();
    assert_eq!(empty.run(&["loadout"]).stdout, bare.stdout);
}

// loadout_cmd.go (documented condition): `loadout --format json` and `text`
// are refused, naming the one format it has; nothing is printed.
#[test]
fn test_every_other_format_is_refused_naming_yaml() {
    let project = Project::initialized();
    for format in ["json", "text", "toml", "markdown"] {
        let ran = project.run(&["loadout", "--format", format]);
        ran.fails()
            .contains(&format!("unknown format \"{format}\" (supported: yaml)"));
        assert_eq!(ran.stdout, "", "{format} printed something");
    }
}

// D24 / D8 (plan): the owner's prose, and the MCP tool it names, are withheld
// until the cut list is known; this test fails the day the text is copied
// without the owner's word (new design, not traceable to a Go scenario).
#[test]
fn test_the_owners_text_is_withheld_until_the_cut_list_is_known() {
    let project = Project::initialized();
    let out = project.run(&["loadout"]).stdout;
    for owned in [
        "isolation-axes",
        "agent_run",
        "Isolation: specify both axes",
        "description:",
        "notes:",
    ] {
        assert!(!out.contains(owned), "the loadout ships {owned:?}: {out}");
    }
}
