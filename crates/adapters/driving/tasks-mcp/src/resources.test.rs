//! Each resource's document, byte for byte as Go's reads, and Go's JSON
//! encoding of what they hold.

use std::path::PathBuf;

use serde_json::json;
use tasks_application::{Project, Reach, TagCount, TagSchema, Vocabulary};

use super::{go_float, go_indented, offered, schema_text, vocabulary_text, TAG_SCHEMA};

/// What Go's resources read, as `scripts/go-companion-fixtures.sh`
/// extracted them: the tag schema with nothing configured, and the
/// vocabulary of a project seeded with four tasks (its id and directory
/// left as PROJECT_ID and PROJECT_DIR).
const GO_TAG_SCHEMA: &str = include_str!("../fixtures/go/tag-schema.json");
const GO_TAG_VOCABULARY: &str = include_str!("../fixtures/go/tag-vocabulary.json");

fn catalog() -> presentation::Catalog {
    presentation::Catalog::en_us(&[crate::EN_US]).unwrap()
}

fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::from("/s/state.db"),
    }
}

fn count(tag: &str, active: usize, total: usize) -> TagCount {
    TagCount {
        tag: tasks_application::parse_tag(tag).unwrap().unwrap(),
        active,
        total,
    }
}

#[test]
fn test_the_resources_offered_are_the_tag_schema_and_vocabulary() {
    let uris: Vec<String> = offered().into_iter().map(|r| r.uri).collect();
    assert_eq!(uris, [TAG_SCHEMA, super::TAG_VOCABULARY]);
}

#[test]
fn test_the_default_tag_schema_reads_as_gos_byte_for_byte() {
    let text = schema_text(&TagSchema::resolved(&[]).unwrap());
    assert!(
        text == GO_TAG_SCHEMA,
        "the tag schema differs from Go's:\n{text}"
    );
}

// Go's tagSchemaDoc: a target is one entry whatever facets name it; an
// arity declared other than scalar is an entry with no `scalar`; hiding
// and types are not reported.
#[test]
fn test_each_target_is_one_entry_carrying_every_facet_reported() {
    let schema = TagSchema::parse([
        r#"tagma.arity:"a:x"=multi"#,
        r#"tagma.range:"a:x"="0.5,2""#,
        r#"tagma.priority_fn:"b:y"="1""#,
        r#"tagma.decay_fn:"b:y"="2""#,
        r#"tagma.enum:"b:y"="p,q""#,
        r#"tagma.hide:"c:z"=true"#,
        r#"tagma.type:"d:w"=semver"#,
    ])
    .unwrap();
    let doc: serde_json::Value = serde_json::from_str(&schema_text(&schema)).unwrap();
    assert_eq!(
        doc,
        json!({"targets": [
            {"target": "a:x", "range": {"min": 0.5, "max": 2}},
            {"target": "b:y", "enum": ["p", "q"], "priority_fn": "1", "decay_fn": "2"},
        ]})
    );
    let empty: serde_json::Value =
        serde_json::from_str(&schema_text(&TagSchema::default())).unwrap();
    assert_eq!(empty, json!({"targets": []}));
}

#[test]
fn test_the_vocabulary_of_a_project_reads_as_gos_byte_for_byte() {
    let vocabulary = Vocabulary {
        reach: Reach::Project(project()),
        tags: vec![
            count("release", 0, 2),
            count("triage:kind=defect", 1, 1),
            count("urgent", 2, 3),
            count("version=1.2.3", 1, 1),
        ],
    };
    let go = GO_TAG_VOCABULARY
        .replace("PROJECT_ID", "jolly-mute-magma")
        .replace("PROJECT_DIR", "/work/tree");
    let text = vocabulary_text(&vocabulary, &catalog());
    assert!(text == go, "the vocabulary differs from Go's:\n{text}");
}

#[test]
fn test_a_vocabulary_read_nowhere_says_why_and_lists_no_tags() {
    let nowhere = Vocabulary {
        reach: Reach::NoProject,
        tags: Vec::new(),
    };
    let doc: serde_json::Value =
        serde_json::from_str(&vocabulary_text(&nowhere, &catalog())).unwrap();
    assert_eq!(
        doc,
        json!({"warning": "no project here yet: nothing to list", "tags": []})
    );
    let every = Vocabulary {
        reach: Reach::Every {
            projects: 2,
            fallback: Some("/tmp/somewhere".to_string()),
        },
        tags: vec![count("urgent", 1, 1)],
    };
    let doc: serde_json::Value =
        serde_json::from_str(&vocabulary_text(&every, &catalog())).unwrap();
    assert!(doc.get("project_id").is_none(), "{doc}");
    assert!(
        doc["warning"]
            .as_str()
            .is_some_and(|w| w.starts_with("no project here: /tmp/somewhere is in")),
        "{doc}"
    );
    assert_eq!(
        doc["tags"],
        json!([{"tag": "urgent", "active": 1, "total": 1}])
    );
    let asked = Vocabulary {
        reach: Reach::Every {
            projects: 2,
            fallback: None,
        },
        tags: Vec::new(),
    };
    let doc: serde_json::Value =
        serde_json::from_str(&vocabulary_text(&asked, &catalog())).unwrap();
    assert_eq!(doc, json!({"tags": []}));
}

// Go's floatEncoder: shortest form, plain from 1e-6 to 1e21, else an
// exponent with its sign and no leading zero.
#[test]
fn test_a_float_is_written_as_gos_encoder_writes_it() {
    for (value, go) in [
        (0.0, "0"),
        (-0.0, "-0"),
        (5.0, "5"),
        (20_250_101.0, "20250101"),
        (0.5, "0.5"),
        (-1.25, "-1.25"),
        (1e-6, "0.000001"),
        (1e-7, "1e-7"),
        (1.5e-10, "1.5e-10"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (-2.5e300, "-2.5e+300"),
    ] {
        assert_eq!(go_float(value), go, "{value:e}");
    }
}

#[test]
fn test_a_string_is_escaped_as_gos_encoder_escapes_it() {
    let text = go_indented(&json!({"s": "a<b>c&d\u{2028}e\u{2029}f\"g\n"}));
    assert_eq!(
        text,
        "{\n  \"s\": \"a\\u003cb\\u003ec\\u0026d\\u2028e\\u2029f\\\"g\\n\"\n}"
    );
    assert_eq!(
        go_indented(&json!({"a": [], "b": {}})),
        "{\n  \"a\": [],\n  \"b\": {}\n}"
    );
}
