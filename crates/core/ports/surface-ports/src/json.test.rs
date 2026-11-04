use super::*;

#[test]
fn test_numbers_keep_the_digits_they_were_written_in() {
    let text = r#"{"a": 1234567890123456789, "b": -9223372036854775808, "c": 18446744073709551615, "d": 1.50e3}"#;
    let doc = Json::parse(text).unwrap();
    let again = doc.pretty();
    for digits in [
        "1234567890123456789",
        "-9223372036854775808",
        "18446744073709551615",
        "1.50e3",
    ] {
        assert!(again.contains(digits), "{again}");
    }
}

#[test]
fn test_members_keep_the_order_they_were_written_in() {
    let doc = Json::parse(r#"{"z": 1, "a": 2, "m": 3}"#).unwrap();
    assert_eq!(doc.pretty(), "{\n  \"z\": 1,\n  \"a\": 2,\n  \"m\": 3\n}");
}

#[test]
fn test_a_document_is_written_as_go_indents_it() {
    let doc = Json::object([
        ("empty", Json::Array(vec![])),
        ("none", Json::object::<&str>([])),
        ("list", Json::strings(["a", "b"])),
    ]);
    assert_eq!(
        doc.pretty(),
        "{\n  \"empty\": [],\n  \"none\": {},\n  \"list\": [\n    \"a\",\n    \"b\"\n  ]\n}"
    );
}

#[test]
fn test_strings_round_trip_with_their_escapes() {
    let doc = Json::parse(r#""a\"b\\c\nd\u00e9\ud83d\ude00""#).unwrap();
    assert_eq!(doc, Json::text("a\"b\\c\nd\u{e9}\u{1F600}"));
    let again = Json::parse(&doc.pretty()).unwrap();
    assert_eq!(again, doc);
}

#[test]
fn test_control_characters_are_written_escaped() {
    assert_eq!(Json::text("a\u{1}b").pretty(), "\"a\\u0001b\"");
}

#[test]
fn test_text_that_is_not_one_document_is_refused() {
    for bad in [
        "",
        "{",
        "[1,]",
        "{\"a\" 1}",
        "1 2",
        "tru",
        "\"open",
        "{\"a\":}",
        "-",
        "[1 2]",
    ] {
        assert!(Json::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn test_a_document_nested_too_deeply_is_refused_not_overflowed() {
    let deep = "[".repeat(1000) + &"]".repeat(1000);
    assert!(Json::parse(&deep).is_err());
}

#[test]
fn test_a_key_written_twice_keeps_the_last_value_in_the_first_place() {
    let doc = Json::parse(r#"{"a": 1, "b": 2, "a": 3}"#).unwrap();
    assert_eq!(doc.pretty(), "{\n  \"a\": 3,\n  \"b\": 2\n}");
}

#[test]
fn test_members_are_read_set_and_removed() {
    let mut doc = Json::parse(r#"{"a": 1}"#).unwrap();
    assert!(doc.set("b", Json::text("x")));
    assert!(doc.set("a", Json::Null));
    assert_eq!(doc.get("b").and_then(Json::as_str), Some("x"));
    assert_eq!(doc.remove("a"), Some(Json::Null));
    assert_eq!(doc.get("a"), None);
    assert!(!Json::Null.set("k", Json::Null));
    assert!(doc.get_mut("b").is_some());
}
