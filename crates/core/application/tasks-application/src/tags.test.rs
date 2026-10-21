//! Tag strings parsed at the edge of a write.

use super::{parse, parse_all, Tag, TagRefusal};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn tag(namespace: Option<&str>, key: &str, value: Option<&str>) -> Tag {
    Tag {
        namespace: namespace.map(str::to_string),
        key: key.to_string(),
        value: value.map(str::to_string),
    }
}

#[test]
fn test_a_tag_parses_into_its_components() {
    assert_eq!(
        parse(" geo:lat=57.64 "),
        Some(Ok(tag(Some("geo"), "lat", Some("57.64"))))
    );
    assert_eq!(parse("urgent"), Some(Ok(tag(None, "urgent", None))));
}

#[test]
fn test_an_empty_tag_is_no_tag() {
    assert_eq!(parse("   "), None);
    assert_eq!(parse(""), None);
}

#[test]
fn test_a_tag_outside_the_grammar_is_refused_with_why() {
    let Some(Err(TagRefusal::Unparseable { tag, detail })) = parse(" a b ") else {
        panic!("refused");
    };
    assert_eq!(tag, "a b", "trimmed");
    assert!(detail.contains("\"a b\""), "{detail}");
}

#[test]
fn test_every_tag_is_parsed_in_order_the_empty_ones_left_out() {
    assert_eq!(
        parse_all(&strings(&["b", " ", "a=1"])),
        Ok(vec![tag(None, "b", None), tag(None, "a", Some("1"))])
    );
    assert!(matches!(
        parse_all(&strings(&["ok", "not/ok", "a b"])),
        Err(TagRefusal::Unparseable { tag, .. }) if tag == "not/ok"
    ));
}

#[test]
fn test_a_refusal_renders_its_english() {
    let refusal = TagRefusal::Unparseable {
        tag: "a b".to_string(),
        detail: "why".to_string(),
    };
    assert_eq!(
        refusal.to_string(),
        "the tag a b is not a tag in tagma's grammar: why"
    );
}
