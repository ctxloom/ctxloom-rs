//! Ported from Go's `internal/shared/keymatch/keymatch_test.go` (the matcher);
//! `key_line` has no Go counterpart (Go's YAML nodes carry their lines).

use super::*;

// keymatch_test.go: Nearest suggests within the budget and never beyond it
#[test]
fn test_nearest_suggests_within_the_budget_only() {
    let known = ["hooks", "mcp", "notes"];
    assert_eq!(nearest("hoooks", &known), "hooks");
    assert_eq!(nearest("note", &known), "notes");
    assert_eq!(nearest("zzzzzz", &known), "");
    // The budget is a third of the key's length, at least one: one edit
    // allowed for a short key, none for a longer distance.
    assert_eq!(nearest("mc", &known), "mcp");
    assert_eq!(nearest("cp", &known), "mcp");
    assert_eq!(nearest("xyz", &known), "");
}

// keymatch.go: the first of equally near keys wins
#[test]
fn test_the_first_of_equally_near_keys_wins() {
    assert_eq!(nearest("tag", &["tags", "tab"]), "tags");
}

// the line a key is on, by its path
#[test]
fn test_key_line_finds_nested_keys() {
    let text = "version: 1\nmcp:\n  m:\n    command: x\n    note: y\nhooks:\n  pre_tool:\n    - command: a\n      bogus: 1\n";
    assert_eq!(key_line(text, &["version"]), 1);
    assert_eq!(key_line(text, &["mcp", "m", "note"]), 5);
    assert_eq!(key_line(text, &["hooks", "pre_tool", "[]", "bogus"]), 9);
    assert_eq!(key_line(text, &["nope"]), 1);
}

// key_line: blank and comment lines between a parent and its child are skipped
#[test]
fn test_key_line_steps_over_blank_and_comment_lines() {
    let text = "mcp:\n# top comment\n\n  m:\n\n# c\n    command: x\n";
    assert_eq!(key_line(text, &["mcp", "m", "command"]), 7);
    assert_eq!(key_line(text, &["mcp", "m"]), 4);
}

// angzarr-yaml reads no `---`; the first non-comment line may be one, whatever
// blank or comment lines come before it.
#[test]
fn test_a_document_start_after_blank_and_comment_lines_is_taken_in_stride() {
    for text in [
        "---\na: b\n",
        "# c\n---\na: b\n",
        "\n---\na: b\n",
        "\n# c\n\n---\na: b\n",
    ] {
        let value = parse(text).unwrap_or_else(|e| panic!("{text:?}: {e:?}"));
        assert_eq!(
            value.get("a").and_then(Value::as_str),
            Some("b"),
            "{text:?}"
        );
    }
}

// decode: a boolean takes YAML 1.1's spellings in any case; a null is false
#[test]
fn test_a_boolean_takes_every_yaml_spelling() {
    let s = |t: &str| Value::Scalar(t.to_string());
    for yes in ["true", "TRUE", "yes", "on", "y", "Y"] {
        assert_eq!(boolean(&s(yes)), Ok(true), "{yes}");
    }
    for no in ["false", "False", "no", "off", "n", "N"] {
        assert_eq!(boolean(&s(no)), Ok(false), "{no}");
    }
    assert_eq!(boolean(&Value::Null), Ok(false));
    assert_eq!(
        boolean(&s("maybe")),
        Err("expected a boolean, found \"maybe\"".to_string())
    );
    assert_eq!(
        boolean(&Value::Seq(Vec::new())),
        Err("expected a boolean, found a list".to_string())
    );
}

// decode: a refusal names the kind of value it found
#[test]
fn test_a_refusal_names_the_kind_of_value_found() {
    assert_eq!(kind(&Value::Null), "nothing");
    assert_eq!(kind(&Value::Scalar("x".into())), "a scalar");
    assert_eq!(kind(&Value::Seq(Vec::new())), "a list");
    assert_eq!(kind(&Value::Map(Vec::new())), "a mapping");
}

// keymatch.go: the budget is a third of the key's length, one at least, two at most
#[test]
fn test_the_budget_is_a_third_of_the_keys_length_between_one_and_two() {
    assert_eq!(nearest("abc", &["axx"]), "");
    assert_eq!(nearest("abc", &["axc"]), "axc");
    assert_eq!(nearest("abcdef", &["abcxyf"]), "abcxyf");
    assert_eq!(nearest("abcdef", &["abxyzf"]), "");
    assert_eq!(nearest("abcdefghi", &["abcdefgxy"]), "abcdefgxy");
    assert_eq!(nearest("abcdefghi", &["abcdexyzi"]), "");
    assert_eq!(nearest("abcdefghijkl", &["abcdefghixyz"]), "");
    assert_eq!(nearest("abcdefghijkl", &["abcdefghijxy"]), "abcdefghijxy");
}
