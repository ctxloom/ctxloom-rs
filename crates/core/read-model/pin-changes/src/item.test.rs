use std::collections::BTreeMap;

use library::model::Bundle;
use library::tree::read_tree;

use super::*;
use crate::{Change, ExecDelta, ExecSpec};

fn bundle(files: &[(&str, &str)]) -> Bundle {
    let mut tree: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    tree.insert(
        "bundle.yaml".to_string(),
        b"schema_version: 4\nversion: 1.0.0\n".to_vec(),
    );
    for (path, body) in files {
        tree.insert(path.to_string(), body.as_bytes().to_vec());
    }
    read_tree("demo", &tree).expect("a bundle")
}

fn changed(from: &Bundle, to: &Bundle) -> Vec<(&'static str, String, Change)> {
    diff_items(from, to)
        .into_iter()
        .map(|i| (i.kind, i.name, i.change))
        .collect()
}

// pin_change.go valueFingerprint: sha256("secret-SCRIPT-ONE") starts 8d352d65.
#[test]
fn test_a_value_is_stood_in_for_by_the_start_of_its_digest() {
    assert_eq!(fingerprint("secret-SCRIPT-ONE"), "<8d352d65>");
    assert_eq!(fingerprint("secret-SCRIPT-TWO"), "<70381576>");
    assert_eq!(fingerprint(""), "<e3b0c442>");
}

#[test]
fn test_a_bundle_against_nothing_adds_everything_in_kind_order() {
    let to = bundle(&[
        ("fragments/zeta.md", "z\n"),
        ("fragments/alpha.md", "a\n"),
        ("prompts/review.md", "r\n"),
        ("mcp/srv.yaml", "command: srv\n"),
        ("hooks/pre_tool/check.yaml", "command: check\n"),
        ("profiles/dev.yaml", "bundles: [x]\n"),
        (
            "skills/runner/SKILL.md",
            "---\nname: runner\ndescription: d\n---\nbody\n",
        ),
    ]);
    assert_eq!(
        changed(&Bundle::default(), &to),
        vec![
            ("hook", "pre_tool/0".to_string(), Change::Added),
            ("mcp", "srv".to_string(), Change::Added),
            ("skill", "runner".to_string(), Change::Added),
            ("command", "review".to_string(), Change::Added),
            ("fragment", "alpha".to_string(), Change::Added),
            ("fragment", "zeta".to_string(), Change::Added),
            ("profile", "dev".to_string(), Change::Added),
        ]
    );
}

#[test]
fn test_added_removed_and_modified_items_are_told_apart() {
    let from = bundle(&[
        ("fragments/same.md", "s\n"),
        ("fragments/edited.md", "before\n"),
        ("fragments/gone.md", "g\n"),
    ]);
    let to = bundle(&[
        ("fragments/same.md", "s\n"),
        ("fragments/edited.md", "after\n"),
        ("fragments/new.md", "n\n"),
    ]);
    assert_eq!(
        changed(&from, &to),
        vec![
            ("fragment", "edited".to_string(), Change::Modified),
            ("fragment", "gone".to_string(), Change::Removed),
            ("fragment", "new".to_string(), Change::Added),
        ]
    );
    assert!(changed(&to, &to).is_empty());
}

// mcpExec: the command, args, env and headers, the values fingerprinted.
#[test]
fn test_an_mcp_server_shows_what_it_runs_and_never_a_value() {
    let from = bundle(&[(
        "mcp/srv.yaml",
        "command: fixture-mcp-one\nargs: [--stdio]\nenv:\n  API_KEY: secret-SCRIPT-ONE\n",
    )]);
    let to = bundle(&[(
        "mcp/srv.yaml",
        "command: fixture-mcp-two\nargs: [--stdio]\nenv:\n  API_KEY: secret-SCRIPT-TWO\n",
    )]);
    let items = diff_items(&from, &to);
    assert_eq!(items.len(), 1);
    let delta = items[0].exec.clone().expect("what it runs");
    assert_eq!(
        delta,
        ExecDelta {
            before: Some(ExecSpec {
                command: "fixture-mcp-one".to_string(),
                args: vec!["--stdio".to_string()],
                env: BTreeMap::from([("API_KEY".to_string(), "<8d352d65>".to_string())]),
                ..ExecSpec::default()
            }),
            after: Some(ExecSpec {
                command: "fixture-mcp-two".to_string(),
                args: vec!["--stdio".to_string()],
                env: BTreeMap::from([("API_KEY".to_string(), "<70381576>".to_string())]),
                ..ExecSpec::default()
            }),
        }
    );
    let shown = format!("{items:?}");
    assert!(!shown.contains("secret-SCRIPT"), "{shown}");
}

#[test]
fn test_a_remote_mcp_server_shows_its_url_and_fingerprinted_headers() {
    let to = bundle(&[(
        "mcp/hosted.yaml",
        "url: https://mcp.example.test/sse\nheaders:\n  Authorization: Bearer secret\n",
    )]);
    let items = diff_items(&Bundle::default(), &to);
    let after = items[0].exec.clone().expect("exec").after.expect("after");
    assert_eq!(after.url, "https://mcp.example.test/sse");
    assert_eq!(after.headers["Authorization"], fingerprint("Bearer secret"));
    assert!(items[0].exec.as_ref().expect("exec").before.is_none());
}

// hookExec: a hook that runs a command shows it; one that runs none has no
// spec.
#[test]
fn test_a_hook_shows_its_command_and_args() {
    let from = bundle(&[(
        "hooks/pre_tool/check.yaml",
        "command: check\nargs: [--fast]\n",
    )]);
    let to = bundle(&[(
        "hooks/pre_tool/check.yaml",
        "command: check\nargs: [--slow]\n",
    )]);
    let items = diff_items(&from, &to);
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].kind, items[0].change), ("hook", Change::Modified));
    let delta = items[0].exec.clone().expect("exec");
    let before = delta.before.expect("before");
    assert_eq!(
        (before.command.as_str(), before.args),
        ("check", vec!["--fast".to_string()])
    );
    let after = delta.after.expect("after");
    assert_eq!(
        (after.command.as_str(), after.args),
        ("check", vec!["--slow".to_string()])
    );
}

#[test]
fn test_an_item_that_runs_nothing_has_no_exec() {
    let to = bundle(&[("fragments/a.md", "a\n")]);
    assert!(diff_items(&Bundle::default(), &to)[0].exec.is_none());
}
