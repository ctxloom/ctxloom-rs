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
        "1.50e+3",
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

#[test]
fn test_every_number_form_comes_back_as_written() {
    for digits in [
        "0",
        "-0",
        "0.5",
        "1e+2",
        "-1.25e-3",
        "123456789012345678901234567890",
    ] {
        let doc = Json::parse(digits).unwrap();
        assert_eq!(doc, Json::Number(digits.to_string()));
        assert_eq!(doc.pretty(), digits);
    }
}

#[test]
fn test_text_is_written_unescaped_beyond_quotes_backslashes_and_controls() {
    assert_eq!(
        Json::text("\u{e9}/<>&\u{7f}\u{1F600}").pretty(),
        "\"\u{e9}/<>&\u{7f}\u{1F600}\""
    );
    assert_eq!(Json::text("\t\r\n\"\\").pretty(), r#""\t\r\n\"\\""#);
}

#[test]
fn test_whitespace_around_a_document_is_allowed() {
    assert_eq!(Json::parse(" \n\t[1] \r\n").unwrap().pretty(), "[\n  1\n]");
}

#[test]
fn test_nested_documents_are_indented_per_level() {
    let doc = Json::parse(r#"{"a":[{"b":[]},null,true,false],"c":{}}"#).unwrap();
    assert_eq!(
        doc.pretty(),
        "{\n  \"a\": [\n    {\n      \"b\": []\n    },\n    null,\n    true,\n    false\n  ],\n  \"c\": {}\n}"
    );
}

#[test]
fn test_a_document_at_the_depth_limit_reads() {
    let ok = "[".repeat(100) + &"]".repeat(100);
    assert!(Json::parse(&ok).is_ok());
}

#[test]
fn test_backspace_and_form_feed_are_written_as_go_1_22_writes_them() {
    assert_eq!(Json::text("\u{8}\u{c}").pretty(), "\"\\b\\f\"");
}

#[test]
fn test_an_exponent_is_spelled_lowercase_with_its_sign() {
    for (written, kept) in [("1E5", "1e+5"), ("2.5E-3", "2.5e-3"), ("7e3", "7e+3")] {
        assert_eq!(
            Json::parse(written).unwrap(),
            Json::Number(kept.to_string())
        );
    }
}

#[test]
fn test_a_number_that_is_not_one_is_written_as_null() {
    assert_eq!(Json::Number("12x".to_string()).pretty(), "null");
}

#[test]
fn test_an_object_with_the_reader_s_number_key_is_still_an_object() {
    let doc = Json::parse(r#"{"$serde_json::private::Number": 1}"#).unwrap();
    assert_eq!(
        doc,
        Json::object([("$serde_json::private::Number", Json::Number("1".into()))])
    );
}

#[test]
fn test_an_object_of_one_string_member_is_an_object_not_a_number() {
    let doc = Json::parse(r#"{"a": "12"}"#).unwrap();
    assert_eq!(doc, Json::object([("a", Json::text("12"))]));
}

#[test]
fn test_only_an_array_has_elements() {
    let doc = Json::parse(r#"[1, "a"]"#).unwrap();
    assert_eq!(
        doc.as_array(),
        Some(&vec![Json::Number("1".into()), Json::text("a")])
    );
    assert_eq!(Json::parse("{}").unwrap().as_array(), None);
    assert_eq!(Json::parse("[]").unwrap().as_array(), Some(&vec![]));
}

#[test]
fn test_a_value_json_cannot_hold_says_what_it_expected() {
    use serde::de::value::{BytesDeserializer, Error};
    use serde::Deserialize;
    let err = Json::deserialize(BytesDeserializer::<Error>::new(b"x")).unwrap_err();
    assert!(err.to_string().contains("expected a JSON value"), "{err}");
}
