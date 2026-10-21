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

// ------------------------------------------------------------ admission
// Go's validateTag cases (operations_test.go).

use super::{admit, admit_all, collapse};
use crate::schema::TagSchema;

fn refused(spelled: &str, schema: &TagSchema) -> Option<TagRefusal> {
    match parse(spelled)? {
        Err(refusal) => Some(refusal),
        Ok(tag) => admit(&tag, schema).err(),
    }
}

fn triage() -> TagSchema {
    TagSchema::parse([
        r#"tagma.enum:"triage:kind"="defect,capability,chore""#,
        r#"tagma.range:"triage:effort"="0,5""#,
        r#"tagma.type:"triage:blocks-release"=semver"#,
    ])
    .unwrap()
}

#[test]
fn test_an_operator_word_is_refused_in_any_case() {
    for word in [
        "and", "AND", "And", "or", "OR", "not", "NOT", " and ", "\"and\"",
    ] {
        assert!(
            matches!(
                refused(word, &TagSchema::default()),
                Some(TagRefusal::OperatorWord { .. })
            ),
            "{word}"
        );
    }
    // Only the bare word: with a namespace or a value, a query can match it.
    for fine in ["x:and", "and=1", "android", "cannot", "note", "north"] {
        assert_eq!(refused(fine, &TagSchema::default()), None, "{fine}");
    }
}

#[test]
fn test_a_tag_outside_the_grammar_is_refused() {
    for bad in ["urgent*", "+", "*", "foo/bar", "has space"] {
        assert!(
            matches!(
                refused(bad, &TagSchema::default()),
                Some(TagRefusal::Unparseable { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn test_ordinary_tags_are_admitted() {
    for tag in [
        "urgent",
        "release",
        "android",
        "cannot",
        "note",
        "north",
        "v1.2",
        "a-b_c",
        "release+",
        "1.0.0+build.5",
        "+1",
        "-1",
        "tagmatic:foo=bar",
    ] {
        assert_eq!(refused(tag, &triage()), None, "{tag}");
    }
}

#[test]
fn test_the_tagma_namespace_is_the_schemas() {
    for tag in [
        r#"tagma.arity:"triage:type"=scalar"#,
        "tagma:foo",
        "tagma.priority_fn:x=y",
    ] {
        let Some(TagRefusal::ReservedNamespace { namespace, .. }) =
            refused(tag, &TagSchema::default())
        else {
            panic!("{tag} refused");
        };
        assert!(namespace.starts_with("tagma"), "{namespace}");
    }
}

#[test]
fn test_an_enum_admits_its_members_only() {
    assert_eq!(refused("triage:kind=defect", &triage()), None);
    let Some(refusal @ TagRefusal::NotInEnum { .. }) = refused("triage:kind=sparkles", &triage())
    else {
        panic!("refused");
    };
    let text = refusal.to_string();
    assert!(
        text.contains("triage:kind") && text.contains("sparkles"),
        "{text}"
    );
    assert!(text.contains("defect, capability, chore"), "{text}");
    // A valueless tag carries nothing to check.
    assert_eq!(refused("triage:kind", &triage()), None);
}

#[test]
fn test_a_range_admits_finite_numbers_within_it() {
    for fine in [
        "triage:effort=3",
        "triage:effort=0",
        "triage:effort=5",
        "triage:effort=2.5",
    ] {
        assert_eq!(refused(fine, &triage()), None, "{fine}");
    }
    assert!(matches!(
        refused("triage:effort=9", &triage()),
        Some(TagRefusal::OutOfRange { .. })
    ));
    assert!(matches!(
        refused("triage:effort=-1", &triage()),
        Some(TagRefusal::OutOfRange { .. })
    ));
    for not_a_number in [
        "triage:effort=notanumber",
        "triage:effort=NaN",
        "triage:effort=nan",
        "triage:effort=Inf",
        "triage:effort=-Inf",
        "triage:effort=\" 3\"",
    ] {
        assert!(
            matches!(
                refused(not_a_number, &triage()),
                Some(TagRefusal::NotANumber { .. })
            ),
            "{not_a_number}"
        );
    }
    let refusal = refused("triage:effort=9", &triage()).unwrap();
    assert!(refusal.to_string().contains("[0, 5]"), "{refusal}");
}

#[test]
fn test_type_semver_admits_strict_versions_only() {
    for fine in [
        "triage:blocks-release=0.7.0",
        "triage:blocks-release=0.7.0-pre001",
    ] {
        assert_eq!(refused(fine, &triage()), None, "{fine}");
    }
    for bad in [
        "triage:blocks-release=notasemver",
        "triage:blocks-release=0.7",
        "triage:blocks-release=v0.7.0",
    ] {
        let Some(refusal @ TagRefusal::NotSemver { .. }) = refused(bad, &triage()) else {
            panic!("{bad} refused");
        };
        assert!(
            refusal.to_string().contains("triage:blocks-release"),
            "{refusal}"
        );
    }
    // A type no check is known for is not checked.
    let other = TagSchema::parse([r#"tagma.type:"x:y"=date"#]).unwrap();
    assert_eq!(refused("x:y=whenever", &other), None);
}

#[test]
fn test_an_absent_schema_checks_no_value() {
    for tag in [
        "triage:kind=sparkles",
        "triage:effort=9",
        "triage:effort=notanumber",
    ] {
        assert_eq!(refused(tag, &TagSchema::default()), None, "{tag}");
    }
}

#[test]
fn test_admitting_every_tag_stops_at_the_first_refused() {
    let given = strings(&["ok", "triage:kind=sparkles", "and"]);
    assert!(matches!(
        admit_all(&given, &triage()),
        Err(TagRefusal::NotInEnum { .. })
    ));
    assert_eq!(
        admit_all(&strings(&["ok", " "]), &triage()),
        Ok(vec![tag(None, "ok", None)])
    );
}

// ------------------------------------------------------------ collapse
// Go's scalarCollapse cases.

fn tags(spelled: &[&str]) -> Vec<Tag> {
    spelled.iter().map(|s| parse(s).unwrap().unwrap()).collect()
}

fn scalar() -> TagSchema {
    TagSchema::parse([r#"tagma.arity:"triage:type"=scalar"#]).unwrap()
}

#[test]
fn test_a_new_scalar_value_displaces_the_held_one() {
    let (add, displaced) = collapse(
        &scalar(),
        &tags(&["triage:type=bug", "other"]),
        tags(&["triage:type=feature"]),
    );
    assert_eq!(add, tags(&["triage:type=feature"]));
    assert_eq!(displaced, tags(&["triage:type=bug"]));
}

#[test]
fn test_the_same_scalar_value_displaces_nothing_however_spelled() {
    let (add, displaced) = collapse(
        &scalar(),
        &tags(&["triage:type=bug"]),
        tags(&["triage:type=\"bug\""]),
    );
    assert_eq!(add, tags(&["triage:type=bug"]));
    assert!(displaced.is_empty());
}

#[test]
fn test_only_the_last_of_several_scalar_values_is_added() {
    let (add, displaced) = collapse(
        &scalar(),
        &[],
        tags(&["triage:type=a", "x", "triage:type=b", "triage:type=c"]),
    );
    assert_eq!(add, tags(&["x", "triage:type=c"]));
    assert!(displaced.is_empty());
}

#[test]
fn test_an_undeclared_or_bare_target_keeps_every_value() {
    let schema = TagSchema::parse([r#"tagma.arity:"triage:type"=scalar"#]).unwrap();
    let given = tags(&["area:x=1", "area:x=2", "type=a", "type=b"]);
    let (add, displaced) = collapse(&schema, &tags(&["area:x=0"]), given.clone());
    assert_eq!(add, given);
    assert!(displaced.is_empty());
    let (add, displaced) = collapse(
        &TagSchema::default(),
        &tags(&["triage:type=a"]),
        tags(&["triage:type=b"]),
    );
    assert_eq!(add, tags(&["triage:type=b"]));
    assert!(displaced.is_empty(), "no schema, no collapse");
}

#[test]
fn test_a_valueless_scalar_tag_displaces_a_valued_one() {
    let (_, displaced) = collapse(&scalar(), &tags(&["triage:type=a"]), tags(&["triage:type"]));
    assert_eq!(displaced, tags(&["triage:type=a"]));
}
