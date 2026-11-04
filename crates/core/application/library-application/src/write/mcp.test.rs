//! `mcp.feature`'s write scenarios at the application port.

use std::collections::BTreeMap;
use std::sync::Arc;

use library::write::mcp::McpPatch;

use crate::fakes::{plain, Mem, Setup, CONFIG, ENVELOPE};
use crate::write::views::McpEdited;
use crate::Library;

fn world() -> (Library, Arc<Mem>) {
    let (library, mem) = Setup {
        bundles: &[(
            "demo",
            &[
                ENVELOPE,
                (
                    "mcp/tools.yaml",
                    "command: echo\nargs:\n  - KEEP-THIS-ARGUMENT\n",
                ),
                ("mcp/.tools.meta.yaml", "notes: KEEP-THESE-NOTES\n"),
            ],
        )],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    mem.set_editor(|_, content| Ok(content.replace("command: echo", "command: EDITED-BY-TEST")));
    (library, mem)
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

fn patch() -> McpPatch {
    McpPatch::default()
}

// mcp.feature: A name the bundle lacks is created as a remote server
#[test]
fn test_set_creates_a_server_the_bundle_lacks_with_its_sidecar() {
    let (library, mem) = world();
    let done = library
        .mcp_set(
            &plain(),
            "demo#mcp/search",
            &McpPatch {
                url: Some("https://mcp.example.com/v1".to_string()),
                headers: Some(BTreeMap::from([("X-Team".to_string(), "core".to_string())])),
                tags: Some(vec!["search-index".to_string()]),
                notes: Some("Team search index".to_string()),
                installation: Some("Ask core for a seat".to_string()),
                ..patch()
            },
        )
        .result
        .unwrap();
    assert_eq!(
        (
            done.status.as_str(),
            done.bundle.as_str(),
            done.name.as_str(),
            done.path.as_str()
        ),
        (
            "created",
            "demo",
            "search",
            "/p/.ctxloom/content/bundles/v2/demo/bundle.yaml"
        )
    );
    assert_eq!(
        mem.text("demo", "mcp/search.yaml").unwrap(),
        "url: https://mcp.example.com/v1\nheaders:\n  X-Team: core\n"
    );
    assert_eq!(
        mem.text("demo", "mcp/.search.meta.yaml").unwrap(),
        "tags:\n  - search-index\nnotes: Team search index\ninstallation: Ask core for a seat\n"
    );
    assert_eq!(
        mem.text("demo", "mcp/tools.yaml").unwrap(),
        "command: echo\nargs:\n  - KEEP-THIS-ARGUMENT\n"
    );
}

// mcp.feature: Updating an existing server replaces its arguments and keeps what was not passed
#[test]
fn test_set_updates_what_it_names_and_keeps_the_rest() {
    let (library, mem) = world();
    let done = library
        .mcp_set(
            &plain(),
            "demo#mcp/tools",
            &McpPatch {
                command: Some("search-mcp".to_string()),
                args: Some(vec!["--stdio".to_string()]),
                env: Some(BTreeMap::from([(
                    "LOG_LEVEL".to_string(),
                    "debug".to_string(),
                )])),
                ..patch()
            },
        )
        .result
        .unwrap();
    assert_eq!(done.status, "updated");
    assert_eq!(
        mem.text("demo", "mcp/tools.yaml").unwrap(),
        "command: search-mcp\nargs:\n  - --stdio\nenv:\n  LOG_LEVEL: debug\n"
    );
    assert_eq!(
        mem.text("demo", "mcp/.tools.meta.yaml").unwrap(),
        "notes: KEEP-THESE-NOTES\n"
    );
}

// mcp.feature: Clearing the old target in the same call moves a server to the session endpoint
#[test]
fn test_set_can_clear_the_old_target_and_name_the_new_one_together() {
    let (library, mem) = world();
    library
        .mcp_set(
            &plain(),
            "demo#mcp/tools",
            &McpPatch {
                command: Some(String::new()),
                args: Some(vec![]),
                served_by: Some("session-endpoint".to_string()),
                ..patch()
            },
        )
        .result
        .unwrap();
    assert_eq!(
        mem.text("demo", "mcp/tools.yaml").unwrap(),
        "served_by: session-endpoint\n"
    );
}

// mcp.feature: A set that would leave two targets is refused and saves nothing
#[test]
fn test_a_set_that_leaves_two_targets_or_none_is_refused_and_saves_nothing() {
    let (library, mem) = world();
    let two = McpPatch {
        url: Some("https://mcp.example.com/v1".to_string()),
        ..patch()
    };
    let said = err(library.mcp_set(&plain(), "demo#mcp/tools", &two));
    assert!(
        said.contains("McpInvalid") && said.contains("more than one of command"),
        "{said}"
    );
    let none = McpPatch {
        command: Some(String::new()),
        args: Some(vec![]),
        ..patch()
    };
    assert!(err(library.mcp_set(&plain(), "demo#mcp/tools", &none))
        .contains("none of command, url or served_by"));
    let bad_url = McpPatch {
        command: Some(String::new()),
        args: Some(vec![]),
        url: Some("ftp://x".to_string()),
        ..patch()
    };
    assert!(err(library.mcp_set(&plain(), "demo#mcp/tools", &bad_url))
        .contains("url scheme must be http or https"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_set_refuses_a_reference_that_is_not_one_and_a_bundle_nobody_has() {
    let (library, mem) = world();
    let p = McpPatch {
        notes: Some("n".to_string()),
        ..patch()
    };
    for bad in [
        "demo",
        "demo#fragments/x",
        "demo#mcp/",
        "demo#mcp/a/b",
        "#mcp/x",
        "demo#mcp/.x",
    ] {
        assert!(
            err(library.mcp_set(&plain(), bad, &p)).contains("McpReference"),
            "{bad}"
        );
    }
    assert!(err(library.mcp_set(&plain(), "nodemo#mcp/tools", &p)).contains("BundleNotFound"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_set_with_a_patch_that_changes_nothing_writes_nothing() {
    let (library, mem) = world();
    let same = McpPatch {
        command: Some("echo".to_string()),
        notes: Some("KEEP-THESE-NOTES".to_string()),
        ..patch()
    };
    let done = library
        .mcp_set(&plain(), "demo#mcp/tools", &same)
        .result
        .unwrap();
    assert_eq!(done.status, "updated");
    assert_eq!(mem.writes(), 0);
}

// mcp.feature: Editing a bundle's MCP server writes the editor's result back
#[test]
fn test_edit_writes_the_editors_result_back_and_keeps_the_rest() {
    let (library, mem) = world();
    let done = library.mcp_edit(&plain(), "demo#mcp/tools").result.unwrap();
    assert_eq!(
        done,
        McpEdited::Updated {
            name: "tools".to_string(),
            bundle: "demo".to_string()
        }
    );
    assert_eq!(
        mem.text("demo", "mcp/tools.yaml").unwrap(),
        "command: EDITED-BY-TEST\nargs:\n  - KEEP-THIS-ARGUMENT\n"
    );
    assert_eq!(
        mem.text("demo", "mcp/.tools.meta.yaml").unwrap(),
        "notes: KEEP-THESE-NOTES\n"
    );
    assert_eq!(
        mem.edits(),
        [(
            "tools.yaml".to_string(),
            "command: echo\nargs:\n  - KEEP-THIS-ARGUMENT\nnotes: KEEP-THESE-NOTES\n".to_string()
        )]
    );
}

#[test]
fn test_an_edit_that_changes_nothing_empties_or_breaks_the_server_writes_nothing() {
    let (library, mem) = world();
    mem.set_editor(|_, content| Ok(content.to_string()));
    assert_eq!(
        library.mcp_edit(&plain(), "demo#mcp/tools").result.unwrap(),
        McpEdited::Unchanged
    );
    mem.set_editor(|_, _| Ok(String::new()));
    assert!(err(library.mcp_edit(&plain(), "demo#mcp/tools")).contains("McpEditEmpty"));
    mem.set_editor(|_, content| Ok(format!("{content}not_a_key: 1\n")));
    assert!(err(library.mcp_edit(&plain(), "demo#mcp/tools")).contains("McpEditInvalid"));
    mem.set_editor(|_, _| Ok("url: https://x\ncommand: echo\n".to_string()));
    assert!(err(library.mcp_edit(&plain(), "demo#mcp/tools")).contains("McpInvalid"));
    assert_eq!(mem.writes(), 0);
}

// mcp.feature: Editing an MCP server that does not exist fails instead of creating one
#[test]
fn test_edit_of_a_server_nobody_ships_fails_before_the_editor_runs() {
    let (library, mem) = world();
    assert!(err(library.mcp_edit(&plain(), "demo#mcp/absent")).contains("McpServerNotFound"));
    assert!(err(library.mcp_edit(&plain(), "nodemo#mcp/tools")).contains("BundleNotFound"));
    assert!(err(library.mcp_edit(&plain(), "demo")).contains("McpReference"));
    assert!(mem.edits().is_empty());
    assert_eq!(mem.writes(), 0);
}
