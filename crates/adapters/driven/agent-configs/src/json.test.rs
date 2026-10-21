//! The reader's spans and refusals, and the splices against hew's.

use std::collections::BTreeMap;

use super::{append_member, encode, parse, parse_first, quote, remove_member, Kind, Value};

fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

fn entry() -> Value {
    let mut entry = BTreeMap::new();
    entry.insert("command".to_string(), s("taskloom"));
    entry.insert("args".to_string(), Value::Array(vec![s("mcp")]));
    Value::Object(entry)
}

/// `src` with `"k": entry()` appended to its top-level object.
fn appended(src: &str) -> String {
    let doc = parse(src.as_bytes()).expect("JSON");
    let edit = append_member(src.as_bytes(), &doc, "k", &entry());
    String::from_utf8(edit.apply(src.as_bytes())).unwrap()
}

/// `src` with its top-level member `key` removed.
fn removed(src: &str, key: &str) -> String {
    let doc = parse(src.as_bytes()).expect("JSON");
    let (index, _) = doc.member(key).expect("the member");
    let edit = remove_member(src.as_bytes(), &doc, index);
    String::from_utf8(edit.apply(src.as_bytes())).unwrap()
}

#[test]
fn test_members_carry_their_spans_and_commas() {
    let src = br#"{ "a" : 1, "b":[true ,null], "c": {"d": -0.5e+3} }"#;
    let doc = parse(src).unwrap();
    let members = doc.members().unwrap();
    assert_eq!(
        members.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(
        &src[members[0].key_start..members[0].value.end],
        br#""a" : 1"#
    );
    assert_eq!(members[0].comma, Some(9));
    assert_eq!(members[2].comma, None);
    assert_eq!(
        &src[members[1].value.start..members[1].value.end],
        b"[true ,null]"
    );
    let Kind::Array(elements) = &members[1].value.kind else {
        panic!("an array")
    };
    assert_eq!(elements[0].value.kind, Kind::Bool);
    assert_eq!(elements[0].comma, Some(21));
    assert_eq!(elements[1].value.kind, Kind::Null);
    assert_eq!((doc.start, doc.end), (0, src.len()));
}

#[test]
fn test_strings_are_decoded_escapes_and_surrogates_included() {
    let mut src = br#"["a\"b\\c\/\b\f\n\r\t", "\u00e9\ud83d\ude00", "\ud800x", "na"#.to_vec();
    src.extend_from_slice(b"\xc3\xafve\"]");
    let doc = parse(&src).unwrap();
    let Kind::Array(elements) = doc.kind else {
        panic!()
    };
    let strings: Vec<String> = elements
        .into_iter()
        .map(|e| match e.value.kind {
            Kind::String(s) => s,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(strings[0], "a\"b\\c/\u{8}\u{c}\n\r\t");
    assert_eq!(strings[1], "\u{e9}\u{1f600}");
    assert_eq!(strings[2], "\u{fffd}x", "a lone surrogate is replaced");
    assert_eq!(strings[3], "na\u{ef}ve");
}

#[test]
fn test_what_is_not_json_is_refused_where_it_is() {
    for bad in [
        "",
        "  \n",
        "{",
        "{\"a\" 1}",
        "{\"a\": 1,}",
        "[1,]",
        "{a: 1}",
        "// c\n{}",
        "\"\\x\"",
        "\"\\u12g4\"",
        "\"a\nb\"",
        "01",
        "1.",
        "1e",
        "-",
        "tru",
        "{} x",
        "{\"a\": 1 \"b\": 2}",
        "[1 2]",
    ] {
        assert!(parse(bad.as_bytes()).is_err(), "{bad:?}");
    }
    let error = parse(b"{\n  \"a\": 1,\n}").unwrap_err();
    assert_eq!((error.line, error.column), (3, 1));
    assert_eq!(
        error.to_string(),
        "expected a string key at line 3, column 1"
    );
    let deep = "[".repeat(600) + &"]".repeat(600);
    assert!(parse(deep.as_bytes()).is_err(), "too deep");
}

#[test]
fn test_the_first_value_is_read_whatever_follows_and_whitespace_is_none() {
    assert_eq!(parse_first(b" \n\t").unwrap(), None);
    let first = parse_first(b"{\"a\": 1} x").unwrap().expect("a value");
    assert_eq!(first.member("a").map(|(i, _)| i), Some(0));
    assert!(parse_first(b"{\"a\" ").is_err());
}

#[test]
fn test_values_are_written_in_hews_compact_style_keys_sorted() {
    assert_eq!(
        encode(&entry()),
        r#"{ "args": ["mcp"], "command": "taskloom" }"#
    );
    assert_eq!(encode(&Value::Array(vec![])), "[]");
    assert_eq!(encode(&Value::Object(BTreeMap::new())), "{}");
    assert_eq!(encode(&Value::Array(vec![s("a"), s("b")])), r#"["a", "b"]"#);
    assert_eq!(
        quote("a\"\\\n\r\t\u{1}\u{e9}"),
        "\"a\\\"\\\\\\n\\r\\t\\u0001\u{e9}\""
    );
}

#[test]
fn test_a_member_appended_takes_its_siblings_layout() {
    let e = r#""k": { "args": ["mcp"], "command": "taskloom" }"#;
    assert_eq!(appended("{\"a\": 1}"), format!("{{\"a\": 1, {e}}}"));
    assert_eq!(
        appended("{\n  \"a\": 1\n}\n"),
        format!("{{\n  \"a\": 1,\n  {e}\n}}\n")
    );
    assert_eq!(
        appended("{\n\t\"a\": 1,\n\t\"b\": 2\n}"),
        format!("{{\n\t\"a\": 1,\n\t\"b\": 2,\n\t{e}\n}}")
    );
    assert_eq!(appended("{}"), format!("{{ {e} }}"));
    assert_eq!(appended("{ }"), format!("{{ {e}  }}"));
    assert_eq!(appended("{\n}\n"), format!("{{\n  {e}\n}}\n"));
    assert_eq!(
        appended("{\n    \n  }"),
        format!("{{\n  {e}\n}}"),
        "a seeded block keeps its outer indent and the file's step"
    );
}

#[test]
fn test_a_member_removed_takes_one_comma_and_its_own_gap() {
    let block = "{\n  \"a\": 1,\n  \"k\": 2,\n  \"z\": 3\n}";
    assert_eq!(removed(block, "k"), "{\n  \"a\": 1,\n  \"z\": 3\n}");
    assert_eq!(removed(block, "a"), "{\n  \"k\": 2,\n  \"z\": 3\n}");
    assert_eq!(removed(block, "z"), "{\n  \"a\": 1,\n  \"k\": 2\n}");
    assert_eq!(removed("{\"a\": 1, \"k\": 2}", "k"), "{\"a\": 1}");
    assert_eq!(removed("{\"k\": 2, \"a\": 1}", "k"), "{ \"a\": 1}");
    assert_eq!(removed("{ \"k\": 2 }", "k"), "{}");
    assert_eq!(removed("{\"k\": 2 }", "k"), "{ }");
    assert_eq!(removed("{\n  \"k\": 2\n}", "k"), "{\n}");
    assert_eq!(removed("{\"a\": 1 , \"k\": 2}", "k"), "{\"a\": 1 }");
}

#[test]
fn test_an_append_then_a_removal_gives_the_bytes_back() {
    for src in [
        "{}",
        "{ }",
        "{\n}\n",
        "{\"a\": 1}",
        "{\n  \"a\": 1\n}\n",
        "{\r\n  \"a\": 1\r\n}\r\n",
        "{\n    \"a\": {\n        \"b\": 2\n    }\n}",
    ] {
        assert_eq!(removed(&appended(src), "k"), src, "{src:?}");
    }
}

#[test]
fn test_a_block_seed_takes_the_files_first_content_indent() {
    let seeded = |src: &str| {
        let doc = parse(src.as_bytes()).unwrap();
        let (_, inner) = doc.member("s").unwrap();
        let edit = append_member(src.as_bytes(), &inner.value, "k", &s("v"));
        String::from_utf8(edit.apply(src.as_bytes())).unwrap()
    };
    // A blank indented line and a lone "\r" carry no content; the first
    // indented line that does sets the step.
    assert_eq!(
        seeded("{\n   \n \r\n\t\"s\": {\n\t}\n}"),
        "{\n   \n \r\n\t\"s\": {\n\t\t\"k\": \"v\"\n\t}\n}"
    );
    assert_eq!(
        seeded("{\"s\": {\n}}"),
        "{\"s\": {\n  \"k\": \"v\"\n}}",
        "no indent: two spaces"
    );
    assert_eq!(seeded("{\"s\": {}}"), "{\"s\": { \"k\": \"v\" }}");
}

#[test]
fn test_nesting_is_read_to_its_limit_and_no_deeper() {
    let at = |depth: usize| "[".repeat(depth) + &"]".repeat(depth);
    assert!(parse(at(super::MAX_DEPTH + 1).as_bytes()).is_ok());
    assert!(parse(at(super::MAX_DEPTH + 2).as_bytes()).is_err());
    let objects = |depth: usize| "{\"a\": ".repeat(depth) + "1" + &"}".repeat(depth);
    assert!(parse(objects(super::MAX_DEPTH).as_bytes()).is_ok());
    assert!(
        parse(objects(super::MAX_DEPTH + 1).as_bytes()).is_err(),
        "objects count toward the limit too"
    );
}

#[test]
fn test_a_space_is_not_escaped_and_an_empty_array_is_bare() {
    assert_eq!(quote(" \u{1f}"), "\" \\u001f\"");
    assert_eq!(encode(&Value::Array(vec![])), "[]");
    let mut empty = BTreeMap::new();
    empty.insert("a".to_string(), Value::Array(vec![]));
    assert_eq!(encode(&Value::Object(empty)), "{ \"a\": [] }");
    let doc = parse(b"{\"a\": {}, \"bb\": 1}").unwrap();
    assert_eq!(doc.members().unwrap()[1].key_start, 10);
}
