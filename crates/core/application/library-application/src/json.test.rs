//! The configuration's values as JSON and as the lines Go prints them in.

use angzarr_yaml::parse;
use serde_json::json;

use super::{permissions, permissions_line, scalar};

fn doc(text: &str) -> angzarr_yaml::Value {
    parse(text).expect("a document")
}

// mutants: a scalar reads as the type it looks like (Go's yaml decode:
// booleans, integers, else text).
#[test]
fn test_a_scalar_reads_as_what_it_looks_like() {
    assert_eq!(scalar("true"), json!(true));
    assert_eq!(scalar("false"), json!(false));
    assert_eq!(scalar("12"), json!(12));
    assert_eq!(scalar("-3"), json!(-3));
    assert_eq!(scalar("abc"), json!("abc"));
    assert_eq!(scalar("True"), json!("True"));
    assert_eq!(scalar("1.5"), json!("1.5"));
}

// mutants: a permissions block is an object of typed values, nesting kept;
// anything else is `{}`.
#[test]
fn test_a_permissions_block_is_an_object_and_anything_else_is_empty() {
    let block = doc(
        "approver: human\nlimit: 3\nsafe: true\nlist: [a, b]\nnothing:\nclaude-code:\n  mode: plan\n",
    );
    assert_eq!(
        permissions(&block),
        json!({
            "approver": "human",
            "limit": 3,
            "safe": true,
            "list": ["a", "b"],
            "nothing": null,
            "claude-code": {"mode": "plan"},
        })
    );
    assert_eq!(permissions(&doc("[a, b]\n")), json!({}));
    assert_eq!(permissions(&doc("plain\n")), json!({}));
    assert_eq!(permissions(&angzarr_yaml::Value::Null), json!({}));
}

// config show text: the neutral fields, then each engine's block sorted, a
// block rendered the way fmt.Sprint prints a decoded map.
#[test]
fn test_the_permissions_line_lists_neutral_fields_then_sorted_engine_blocks() {
    let block = doc(
        "sandbox: strict\napprover: human\nnetwork: \"\"\ncodex:\n  b: 2\n  a: x\nclaude-code:\n  mode: plan\n  list: [a, b]\n  nested:\n    y: 1\n    x:\n",
    );
    assert_eq!(
        permissions_line(&block),
        "approver=human, sandbox=strict, claude-code={list=[a, b], mode=plan, nested=x=, y=1}, codex={a=x, b=2}"
    );
    assert_eq!(permissions_line(&doc("plain\n")), "");
    assert_eq!(
        permissions_line(&doc("approval_timeout: 5m\nnetwork: off\n")),
        "approval_timeout=5m, network=off"
    );
}
