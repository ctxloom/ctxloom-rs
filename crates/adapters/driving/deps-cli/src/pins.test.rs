use std::collections::BTreeMap;

use super::*;
use deps_application::{Change, Compat, ExecDelta, ExecSpec, FileChange, ItemChange};

const IDENTITY: &str = "ctxloom+git://h/r//bundles/demo";

fn change(from_sha: &str, to_sha: &str) -> PinChange {
    PinChange {
        identity: IDENTITY.to_string(),
        url: "https://h/r".to_string(),
        from_sha: from_sha.to_string(),
        to_sha: to_sha.to_string(),
        from_version: String::new(),
        to_version: String::new(),
        compat: None,
        items: Vec::new(),
        files: Vec::new(),
    }
}

fn spec(command: &str, key: &str) -> ExecSpec {
    ExecSpec {
        command: command.to_string(),
        env: BTreeMap::from([("API_KEY".to_string(), key.to_string())]),
        ..ExecSpec::default()
    }
}

// deps.feature: A first pin shows everything the bundle brings in, executables
// included
#[test]
fn test_a_first_pin_is_headed_first_pin_and_lists_what_it_adds() {
    let mut first = change("", "0379964abcdef0123456789abcdef0123456789a");
    first.items = vec![
        ItemChange {
            kind: "mcp",
            name: "srv".to_string(),
            change: Change::Added,
            exec: Some(ExecDelta {
                before: None,
                after: Some(spec("fixture-mcp-one", "<8d352d65>")),
            }),
        },
        ItemChange {
            kind: "fragment",
            name: "demo-frag".to_string(),
            change: Change::Added,
            exec: None,
        },
    ];
    first.files = vec![
        FileChange {
            path: "mcp/srv.yaml".to_string(),
            change: Change::Added,
            diff: String::new(),
        },
        FileChange {
            path: "scripts/run.sh".to_string(),
            change: Change::Added,
            diff: "--- a/scripts/run.sh\n+++ b/scripts/run.sh\n@@ -0,0 +1,2 @@\n+#!/bin/sh\n+echo SCRIPT-ONE\n".to_string(),
        },
    ];
    assert_eq!(
        pin_changes_text(&[first.clone()]),
        format!(
            "{IDENTITY}  first pin -> unversioned  (new -> 0379964)
  + mcp srv
      command: fixture-mcp-one
      env:
          API_KEY: <8d352d65>
  + fragment demo-frag
  + file mcp/srv.yaml
  + file scripts/run.sh
      --- a/scripts/run.sh
      +++ b/scripts/run.sh
      @@ -0,0 +1,2 @@
      +#!/bin/sh
      +echo SCRIPT-ONE
"
        )
    );
    let new = new_pins_text(&[first]);
    assert!(
        new.starts_with("New pins, with everything each one brings in:\n"),
        "{new}"
    );
    assert_eq!(
        NEW_PINS_HEADER,
        "New pins, with everything each one brings in:"
    );
}

#[test]
fn test_no_new_pin_is_no_text_at_all() {
    assert_eq!(new_pins_text(&[]), "");
    assert_eq!(pin_changes_text(&[]), "");
}

// deps.feature: An upgrade shows what an MCP server and a script run before
// and after
#[test]
fn test_a_modified_server_shows_before_and_after() {
    let mut moved = change(
        "1111111aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "2222222bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    moved.from_version = "v1.0.0".to_string();
    moved.to_version = "v1.1.0".to_string();
    moved.items = vec![ItemChange {
        kind: "mcp",
        name: "srv".to_string(),
        change: Change::Modified,
        exec: Some(ExecDelta {
            before: Some(spec("fixture-mcp-one", "<8d352d65>")),
            after: Some(spec("fixture-mcp-two", "<70381576>")),
        }),
    }];
    assert_eq!(
        pin_changes_text(&[moved]),
        format!(
            "{IDENTITY}  v1.0.0 -> v1.1.0  (1111111 -> 2222222)
  ~ mcp srv
      command: fixture-mcp-one -> fixture-mcp-two
      env:
        ~ API_KEY: <8d352d65> -> <70381576>
"
        )
    );
}

#[test]
fn test_env_names_are_marked_by_how_they_changed() {
    let mut moved = change("1111111", "2222222");
    let before = ExecSpec {
        command: "srv".to_string(),
        args: vec!["--a".to_string()],
        env: BTreeMap::from([
            ("KEPT".to_string(), "<00000001>".to_string()),
            ("GONE".to_string(), "<00000002>".to_string()),
        ]),
        ..ExecSpec::default()
    };
    let after = ExecSpec {
        command: "srv".to_string(),
        args: vec!["--b".to_string(), "c d".to_string()],
        env: BTreeMap::from([
            ("KEPT".to_string(), "<00000001>".to_string()),
            ("NEW".to_string(), "<00000003>".to_string()),
        ]),
        url: "https://x.test".to_string(),
        headers: BTreeMap::from([("H".to_string(), "<00000004>".to_string())]),
    };
    moved.items = vec![ItemChange {
        kind: "mcp",
        name: "srv".to_string(),
        change: Change::Modified,
        exec: Some(ExecDelta {
            before: Some(before),
            after: Some(after),
        }),
    }];
    let text = pin_changes_text(&[moved]);
    for line in [
        "      command: srv\n",
        "      args: \"--a\" -> \"--b\" \"c d\"\n",
        "          KEPT: <00000001>\n",
        "        - GONE: <00000002>\n",
        "        + NEW: <00000003>\n",
        "      url: (none) -> https://x.test\n",
    ] {
        assert!(text.contains(line), "{line:?} in {text}");
    }
    assert!(
        text.contains("      headers:\n        + H: <00000004>\n"),
        "{text}"
    );
}

#[test]
fn test_a_removed_file_and_a_removed_item_are_minus_lines() {
    let mut moved = change("1111111", "2222222");
    moved.items = vec![ItemChange {
        kind: "command",
        name: "old".to_string(),
        change: Change::Removed,
        exec: None,
    }];
    moved.files = vec![FileChange {
        path: "prompts/old.md".to_string(),
        change: Change::Removed,
        diff: String::new(),
    }];
    let text = pin_changes_text(&[moved]);
    assert!(
        text.contains("  - command old\n  - file prompts/old.md\n"),
        "{text}"
    );
}

#[test]
fn test_every_change_is_headed_by_its_own_line() {
    let a = change("", "aaaaaaaaaaaa");
    let mut b = change("bbbbbbbbbbbb", "cccccccccccc");
    b.identity = "ctxloom+git://h/r//bundles/other".to_string();
    let text = pin_changes_text(&[a, b]);
    assert_eq!(
        text,
        format!("{IDENTITY}  first pin -> unversioned  (new -> aaaaaaa)\nctxloom+git://h/r//bundles/other  unversioned -> unversioned  (bbbbbbb -> ccccccc)\n")
    );
}

// An item that is gone shows its env as it was, not name by name as removed.
#[test]
fn test_a_removed_server_lists_its_env_plainly() {
    let mut gone = change("1111111", "2222222");
    gone.items = vec![ItemChange {
        kind: "mcp",
        name: "srv".to_string(),
        change: Change::Removed,
        exec: Some(ExecDelta {
            before: Some(spec("fixture-mcp-one", "<8d352d65>")),
            after: None,
        }),
    }];
    let text = pin_changes_text(&[gone]);
    assert!(
        text.contains("      env:\n          API_KEY: <8d352d65>\n"),
        "{text}"
    );
    assert!(!text.contains("- API_KEY"), "{text}");
}

// Owner ruling, 2026-11-04: a pin change is disclosed as breaking, compatible
// or unknown.
#[test]
fn test_a_move_is_headed_with_what_it_is_worth() {
    for (compat, word) in [
        (Compat::Breaking, "breaking"),
        (Compat::Compatible, "compatible"),
        (Compat::Unknown, "unknown"),
    ] {
        let mut moved = change(
            "1111111aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "2222222bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        moved.from_version = "v1.0.0".to_string();
        moved.to_version = "v2.0.0".to_string();
        moved.compat = Some(compat);
        assert_eq!(
            pin_changes_text(&[moved]),
            format!("{IDENTITY}  v1.0.0 -> v2.0.0  (1111111 -> 2222222)  [{word}]\n"),
            "{word}"
        );
    }
}

#[test]
fn test_a_first_pin_is_not_classified() {
    let first = change("", "0379964abcdef0123456789abcdef0123456789a");
    assert_eq!(first.compat, None);
    assert!(!pin_changes_text(&[first]).contains('['));
}
