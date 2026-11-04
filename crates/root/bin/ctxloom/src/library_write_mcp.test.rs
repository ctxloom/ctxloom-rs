//! Go's acceptance scenarios for the mcp noun's write side (S2),
//! `tests/acceptance/features/cli/mcp.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. Not here: `materialize` (S9), and
//! the listing of ctxloom's own server (its loadout text waits for the cut
//! list, D8/D24).
//!
//! `mcp server edit` answers in text only (Go's format debt): an explicit
//! `--format json` is refused before anything is written.

use super::library::{reports_as, Editor, Project};

const DEMO: &str = ".ctxloom/content/bundles/v2/demo";

/// Go's `the project already has the bundle "demo"` with the `mcp` section the
/// scenarios declare (`tools`: command echo, one argument, notes), as the
/// tree Go keeps it in.
fn demo_with_tools(project: &Project, description: &str) {
    project.write(
        &format!("{DEMO}/bundle.yaml"),
        &format!("schema_version: 4\nversion: 1.0.0\ndescription: {description}\n"),
    );
    project.write(
        &format!("{DEMO}/mcp/tools.yaml"),
        "command: echo\nargs:\n  - KEEP-THIS-ARGUMENT\n",
    );
    project.write(
        &format!("{DEMO}/mcp/.tools.meta.yaml"),
        "notes: KEEP-THESE-NOTES\n",
    );
}

// mcp.feature: Scenario: Editing a bundle's MCP server writes the editor's
// result back
#[test]
fn test_editing_a_bundles_mcp_server_writes_the_editors_result_back() {
    let project = Project::with_editor(Editor::Command);
    demo_with_tools(&project, "mcp edit fixture");
    let ran = project.run(&["mcp", "server", "edit", "demo#mcp/tools"]);
    ran.succeeds().contains("Updated MCP server");
    assert_eq!(
        ran.stdout,
        "Updated MCP server \"tools\" in bundle \"demo\"\n"
    );
    project
        .file_contains(&format!("{DEMO}/mcp/tools.yaml"), "EDITED-BY-TEST")
        .file_contains(&format!("{DEMO}/mcp/tools.yaml"), "KEEP-THIS-ARGUMENT")
        .file_contains(&format!("{DEMO}/mcp/.tools.meta.yaml"), "KEEP-THESE-NOTES");
}

// mcp.feature: Scenario: Editing an MCP server that does not exist fails
// instead of creating one
#[test]
fn test_editing_an_mcp_server_that_does_not_exist_fails_instead_of_creating_one() {
    let project = Project::with_editor(Editor::Command);
    project
        .run(&["mcp", "server", "edit", "demo#mcp/absent"])
        .fails()
        .contains("not found");
    demo_with_tools(&project, "x");
    let before = project.tree(DEMO);
    project
        .run(&["mcp", "server", "edit", "demo#mcp/absent"])
        .fails()
        .contains("MCP server not found: absent");
    assert_eq!(project.tree(DEMO), before);
}

// cli/bundle_items.go runBundleMCPEdit (documented condition): an editor that
// changes nothing says so; one that empties the file, or leaves what is not a
// server, is refused and the server survives; json is not offered.
#[test]
fn test_an_mcp_edit_that_changes_nothing_or_breaks_the_server_leaves_it_alone() {
    let project = Project::initialized();
    demo_with_tools(&project, "x");
    let before = project.tree(DEMO);
    assert_eq!(
        project
            .run(&["mcp", "server", "edit", "demo#mcp/tools"])
            .stdout,
        "No changes made.\n"
    );
    assert_eq!(project.tree(DEMO), before);
    project
        .run(&[
            "mcp",
            "server",
            "edit",
            "demo#mcp/tools",
            "--format",
            "json",
        ])
        .fails()
        .contains("does not support it yet");
    let empties = Project::with_editor(Editor::Empties);
    demo_with_tools(&empties, "x");
    let before = empties.tree(DEMO);
    empties
        .run(&["mcp", "server", "edit", "demo#mcp/tools"])
        .fails()
        .contains("aborted: the edited MCP config is empty; bundle \"demo\" was not changed");
    assert_eq!(empties.tree(DEMO), before);
    let marker = Project::with_editor(Editor::Marker);
    demo_with_tools(&marker, "x");
    let before = marker.tree(DEMO);
    marker
        .run(&["mcp", "server", "edit", "demo#mcp/tools"])
        .fails();
    assert_eq!(marker.tree(DEMO), before);
}

// mcp.feature: Scenario: A name the bundle lacks is created as a remote
// server
#[test]
fn test_a_name_the_bundle_lacks_is_created_as_a_remote_server() {
    let project = Project::initialized();
    demo_with_tools(&project, "mcp set fixture");
    let manifest = project.dir().join(DEMO).join("bundle.yaml");
    let ran = project.run(&[
        "mcp",
        "server",
        "set",
        "demo#mcp/search",
        "--url",
        "https://mcp.example.com/v1",
        "--header",
        "X-Team=core",
        "--tag",
        "search-index",
        "--notes",
        "Team search index",
        "--installation",
        "Ask core for a seat",
    ]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "status"), "created");
    assert_eq!(reports_as(&doc, "bundle"), "demo");
    assert_eq!(reports_as(&doc, "name"), "search");
    assert_eq!(reports_as(&doc, "path"), manifest.to_string_lossy());
    // Go's bytes.
    assert_eq!(
        project.read(&format!("{DEMO}/mcp/search.yaml")).as_deref(),
        Some("url: https://mcp.example.com/v1\nheaders:\n  X-Team: core\n")
    );
    assert_eq!(
        project.read(&format!("{DEMO}/mcp/.search.meta.yaml")).as_deref(),
        Some("tags:\n  - search-index\nnotes: Team search index\ninstallation: Ask core for a seat\n")
    );
    project.file_contains(&format!("{DEMO}/mcp/tools.yaml"), "KEEP-THIS-ARGUMENT");
}

// mcp.feature: Scenario: Updating an existing server replaces its arguments
// and keeps what was not passed
#[test]
fn test_updating_an_existing_server_replaces_its_arguments_and_keeps_what_was_not_passed() {
    let project = Project::initialized();
    demo_with_tools(&project, "mcp set fixture");
    let ran = project.run(&[
        "mcp",
        "server",
        "set",
        "demo#mcp/tools",
        "--command",
        "search-mcp",
        "--arg=--stdio",
        "--env",
        "LOG_LEVEL=debug",
    ]);
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "status"), "updated");
    project
        .file_contains(&format!("{DEMO}/mcp/tools.yaml"), "command: search-mcp")
        .file_contains(&format!("{DEMO}/mcp/tools.yaml"), "--stdio")
        .file_contains(&format!("{DEMO}/mcp/tools.yaml"), "LOG_LEVEL: debug")
        .file_lacks(&format!("{DEMO}/mcp/tools.yaml"), "KEEP-THIS-ARGUMENT")
        .file_contains(&format!("{DEMO}/mcp/.tools.meta.yaml"), "KEEP-THESE-NOTES");
    let text = project.run(&[
        "mcp",
        "server",
        "set",
        "demo#mcp/tools",
        "--notes",
        "n",
        "--format",
        "text",
    ]);
    assert_eq!(text.stdout, "Set MCP server \"tools\" in bundle \"demo\"\n");
}

// mcp.feature: Scenario: Clearing the old target in the same call moves a
// server to the session endpoint
#[test]
fn test_clearing_the_old_target_in_the_same_call_moves_a_server_to_the_session_endpoint() {
    let project = Project::initialized();
    demo_with_tools(&project, "mcp set fixture");
    project
        .run(&[
            "mcp",
            "server",
            "set",
            "demo#mcp/tools",
            "--command=",
            "--arg=",
            "--served-by",
            "session-endpoint",
        ])
        .succeeds();
    assert_eq!(
        project.read(&format!("{DEMO}/mcp/tools.yaml")).as_deref(),
        Some("served_by: session-endpoint\n")
    );
}

// mcp.feature: Scenario: A set that would leave two targets is refused and
// saves nothing
#[test]
fn test_a_set_that_would_leave_two_targets_is_refused_and_saves_nothing() {
    let project = Project::initialized();
    demo_with_tools(&project, "mcp set fixture");
    let before = project.tree(DEMO);
    project
        .run(&[
            "mcp",
            "server",
            "set",
            "demo#mcp/tools",
            "--url",
            "https://mcp.example.com/v1",
        ])
        .fails()
        .contains("more than one of command (with args), url and served_by is set");
    project
        .file_lacks(&format!("{DEMO}/mcp/tools.yaml"), "mcp.example.com")
        .file_contains(&format!("{DEMO}/mcp/tools.yaml"), "command: echo");
    assert_eq!(project.tree(DEMO), before);
    project
        .run(&["mcp", "server", "set", "nodemo#mcp/tools", "--command", "x"])
        .fails()
        .contains("bundle not found: nodemo");
}

// mcp.feature: Scenario: A client pointed at the bare noun is told there is no
// stdio server to point at
#[test]
fn test_a_client_pointed_at_the_bare_noun_is_told_there_is_no_stdio_server_to_point_at() {
    let project = Project::initialized();
    let ran = project.run(&["mcp"]);
    ran.fails()
        .contains("no stdio MCP server")
        .contains("session's endpoint");
}

// mcp.feature: Scenario: Asking the bare noun for JSON is refused and points
// at the listing leaf
#[test]
fn test_asking_the_bare_noun_for_json_is_refused_and_points_at_the_listing_leaf() {
    let project = Project::initialized();
    project
        .run(&["--format", "json", "mcp"])
        .fails()
        .contains("ctxloom mcp server list");
}
