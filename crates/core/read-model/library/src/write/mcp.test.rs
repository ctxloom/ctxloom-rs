use std::collections::BTreeMap;

use super::*;

fn server(command: &str, args: &[&str]) -> McpServer {
    McpServer {
        command: command.to_string(),
        args: args.iter().map(ToString::to_string).collect(),
        ..McpServer::default()
    }
}

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn test_the_files_are_beside_each_other_in_the_kind_directory() {
    assert_eq!(content_path("tools"), "mcp/tools.yaml");
    assert_eq!(meta_path("tools"), "mcp/.tools.meta.yaml");
}

// captured from a Go run: bundle edit --add-mcp tree-sitter
#[test]
fn test_the_placeholder_launches_its_own_name() {
    assert_eq!(
        content_file(&placeholder("tree-sitter")),
        "command: tree-sitter\n"
    );
    assert_eq!(meta_file(&placeholder("tree-sitter")), None);
}

// operations.BundleMCPInput.patch
#[test]
fn test_a_patch_changes_exactly_what_it_names() {
    let base = McpServer {
        command: "echo".to_string(),
        args: vec!["KEEP".to_string()],
        notes: "KEEP-NOTES".to_string(),
        ..McpServer::default()
    };
    let none = McpPatch::default();
    assert_eq!(patched(&base, &none), base);
    let set = McpPatch {
        command: Some("search-mcp".to_string()),
        args: Some(vec!["--stdio".to_string()]),
        env: Some(map(&[("LOG_LEVEL", "debug")])),
        ..McpPatch::default()
    };
    let out = patched(&base, &set);
    assert_eq!(out.command, "search-mcp");
    assert_eq!(out.args, ["--stdio"]);
    assert_eq!(out.env, map(&[("LOG_LEVEL", "debug")]));
    assert_eq!(out.notes, "KEEP-NOTES", "what was not passed is kept");
}

#[test]
fn test_a_patch_naming_an_empty_value_clears_the_field() {
    let base = McpServer {
        command: "echo".to_string(),
        args: vec!["a".to_string()],
        env: map(&[("A", "1")]),
        url: "https://x".to_string(),
        headers: map(&[("H", "v")]),
        tags: vec!["t".to_string()],
        served_by: "session-endpoint".to_string(),
        notes: "n".to_string(),
        installation: "i".to_string(),
    };
    let clear = McpPatch {
        command: Some(String::new()),
        args: Some(Vec::new()),
        env: Some(BTreeMap::new()),
        url: Some(String::new()),
        headers: Some(BTreeMap::new()),
        tags: Some(Vec::new()),
        served_by: Some(String::new()),
        notes: Some(String::new()),
        installation: Some(String::new()),
    };
    assert_eq!(patched(&base, &clear), McpServer::default());
}

// loader: a server has exactly one target
#[test]
fn test_a_server_has_exactly_one_target() {
    assert_eq!(fault(&server("echo", &["a"])), None);
    assert_eq!(
        fault(&McpServer {
            url: "https://mcp.example.com/v1".to_string(),
            ..McpServer::default()
        }),
        None
    );
    assert_eq!(
        fault(&McpServer {
            served_by: "session-endpoint".to_string(),
            ..McpServer::default()
        }),
        None
    );
    let none = fault(&McpServer::default()).expect("none is a fault");
    assert_eq!(none, "mcp server: none of command, url or served_by is set");
    let two = fault(&McpServer {
        command: "echo".to_string(),
        url: "https://x".to_string(),
        ..McpServer::default()
    })
    .expect("two is a fault");
    assert!(
        two.starts_with(
            "mcp server: more than one of command (with args), url and served_by is set"
        ),
        "{two}"
    );
    let scheme = fault(&McpServer {
        url: "ftp://x".to_string(),
        ..McpServer::default()
    })
    .unwrap();
    assert_eq!(
        scheme,
        "mcp server: url scheme must be http or https: \"ftp://x\""
    );
    let endpoint = fault(&McpServer {
        served_by: "elsewhere".to_string(),
        ..McpServer::default()
    })
    .unwrap();
    assert_eq!(
        endpoint,
        "mcp server: served_by must be session-endpoint, got \"elsewhere\""
    );
}

// captured from a Go run: mcp server set
#[test]
fn test_the_content_file_lists_how_the_server_is_reached() {
    let remote = McpServer {
        url: "https://mcp.example.com/v1".to_string(),
        headers: map(&[("X-Team", "core")]),
        ..McpServer::default()
    };
    assert_eq!(
        content_file(&remote),
        "url: https://mcp.example.com/v1\nheaders:\n  X-Team: core\n"
    );
    let full = McpServer {
        command: "search-mcp".to_string(),
        args: vec!["--stdio".to_string(), "x y".to_string()],
        env: map(&[("B", "2"), ("A", "1")]),
        ..McpServer::default()
    };
    assert_eq!(
        content_file(&full),
        "command: search-mcp\nargs:\n  - --stdio\n  - x y\nenv:\n  A: 1\n  B: 2\n"
    );
    assert_eq!(
        content_file(&McpServer {
            served_by: "session-endpoint".to_string(),
            ..McpServer::default()
        }),
        "served_by: session-endpoint\n"
    );
    assert_eq!(content_file(&McpServer::default()), "{}\n");
}

#[test]
fn test_the_sidecar_says_what_is_said_about_the_server() {
    let said = McpServer {
        tags: vec!["search-index".to_string()],
        notes: "Team search index".to_string(),
        installation: "Ask core for a seat".to_string(),
        ..server("echo", &[])
    };
    assert_eq!(
        meta_file(&said).as_deref(),
        Some("tags:\n  - search-index\nnotes: Team search index\ninstallation: Ask core for a seat\n")
    );
    assert_eq!(meta_file(&server("echo", &[])), None);
    assert_eq!(
        meta_file(&McpServer {
            notes: "only notes".to_string(),
            ..McpServer::default()
        })
        .as_deref(),
        Some("notes: only notes\n")
    );
}

#[test]
fn test_an_editor_is_given_the_server_and_what_it_leaves_is_read_back() {
    let said = McpServer {
        command: "echo".to_string(),
        args: vec!["KEEP-THIS-ARGUMENT".to_string()],
        env: map(&[("A", "1")]),
        tags: vec!["t".to_string()],
        notes: "KEEP-THESE-NOTES".to_string(),
        ..McpServer::default()
    };
    let text = editor_text(&said);
    assert_eq!(
        text,
        "command: echo\nargs:\n  - KEEP-THIS-ARGUMENT\nenv:\n  A: 1\ntags:\n  - t\nnotes: KEEP-THESE-NOTES\n"
    );
    assert_eq!(parse_edited(&text).unwrap(), said);
    assert_eq!(
        parse_edited(&text.replace("echo", "EDITED-BY-TEST"))
            .unwrap()
            .command,
        "EDITED-BY-TEST"
    );
}

#[test]
fn test_what_an_editor_leaves_must_be_a_server() {
    assert!(parse_edited("").unwrap_err().contains("empty"));
    assert!(parse_edited("  \n").unwrap_err().contains("empty"));
    assert!(parse_edited("command: [unclosed\n")
        .unwrap_err()
        .contains("invalid YAML"));
    assert!(parse_edited("command: echo\nnot_a_key: 1\n")
        .unwrap_err()
        .contains("not_a_key"));
    assert!(parse_edited("- a\n- b\n").is_err());
}
