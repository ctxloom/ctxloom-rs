//! The tag schema: Go's `tagschema` cases, and what this one refuses that
//! Go only refused on first use.

use annals_diagnostic::Diagnostic;

use super::{Range, TagSchema, Why, DEFAULT_TAG_SCHEMA, FACETS};

fn parse(declarations: &[&str]) -> TagSchema {
    TagSchema::parse(declarations).expect("a valid schema")
}

fn why(declaration: &str) -> Why {
    TagSchema::parse([declaration]).expect_err(declaration).why
}

#[test]
fn test_an_absent_schema_declares_nothing() {
    let schema = TagSchema::default();
    assert!(!schema.is_scalar("triage:type"));
    assert_eq!(schema.enum_of("triage:type"), None);
    assert_eq!(schema.range_of("triage:type"), None);
    assert_eq!(schema.type_of("triage:type"), None);
    assert_eq!(schema.hide_facts().count(), 0);
    assert!(schema.priority_fns().is_empty() && schema.decay_fns().is_empty());
}

#[test]
fn test_an_arity_declares_its_target_scalar() {
    let schema = parse(&[r#"tagma.arity:"triage:type"=scalar"#]);
    assert!(schema.is_scalar("triage:type"));
    assert!(!schema.is_scalar("triage:other"));
}

// Every arity declared is listed, scalar or not, as Go's
// `Targets(ArityFacet)` lists them: the tag-schema resource reports each.
#[test]
fn test_every_declared_arity_is_listed_by_target_scalar_or_not() {
    let schema = parse(&[r#"tagma.arity:"b:y"=scalar"#, r#"tagma.arity:"a:x"=multi"#]);
    let arities: Vec<(&str, bool)> = schema.arities().collect();
    assert_eq!(arities, [("a:x", false), ("b:y", true)]);
    assert_eq!(TagSchema::default().arities().count(), 0);
}

#[test]
fn test_each_facet_is_kept_by_its_target() {
    let schema = parse(&[
        r#"tagma.arity:"triage:impact"=scalar"#,
        r#"tagma.priority_fn:"triage:impact"="{{impact}} * {{age_factor}}""#,
        r#"tagma.decay_fn:"triage:impact"="1 + {{age_days}} / 365""#,
        r#"tagma.type:"triage:blocks-release"=semver"#,
        r#"tagma.range:"triage:impact"="0,5""#,
    ]);
    assert!(schema.is_scalar("triage:impact"));
    assert_eq!(
        schema.priority_fns()["triage:impact"],
        "{{impact}} * {{age_factor}}"
    );
    assert_eq!(
        schema.decay_fns()["triage:impact"],
        "1 + {{age_days}} / 365"
    );
    assert_eq!(schema.type_of("triage:blocks-release"), Some("semver"));
    assert_eq!(
        schema.types().collect::<Vec<_>>(),
        [("triage:blocks-release", "semver")]
    );
    assert_eq!(
        schema.range_of("triage:impact"),
        Some(Range { min: 0.0, max: 5.0 })
    );
}

// Go's TestParse_LastDeclarationWins: any arity but scalar is not scalar,
// as tagma's SPEC §8 has it.
#[test]
fn test_the_last_declaration_of_a_facet_and_target_wins() {
    let schema = parse(&[
        r#"tagma.arity:"triage:type"=scalar"#,
        r#"tagma.arity:"triage:type"=multi"#,
    ]);
    assert!(!schema.is_scalar("triage:type"));
    let schema = parse(&[
        r#"tagma.enum:"triage:kind"="a,b""#,
        r#"tagma.enum:"triage:kind"="c""#,
    ]);
    assert_eq!(schema.enum_of("triage:kind"), Some(&["c".to_string()][..]));
}

#[test]
fn test_an_enums_members_are_trimmed_and_empties_dropped() {
    let schema = parse(&[r#"tagma.enum:"triage:type"="correctness,security, docs ,,build""#]);
    assert_eq!(
        schema.enum_of("triage:type").unwrap(),
        ["correctness", "security", "docs", "build"]
    );
}

#[test]
fn test_hide_facts_are_true_or_false_and_anything_else_is_skipped() {
    let schema = parse(&[
        r#"tagma.hide:"triage:cwe"=true"#,
        r#"tagma.hide:"*:secret"=false"#,
        r#"tagma.hide:"triage:other"="yes""#,
    ]);
    assert_eq!(
        schema.hide_facts().collect::<Vec<_>>(),
        [("*:secret", false), ("triage:cwe", true)]
    );
}

// Go's TestMalformedDeclarationPolicy, TestParse_RejectsMalformed and the
// arity-target and facet-membership cases.
#[test]
fn test_a_malformed_declaration_is_refused_and_named() {
    assert!(matches!(why("not a tag at all"), Why::Unparseable { .. }));
    assert_eq!(why("bare-key=value"), Why::NoNamespace);
    for not_a_facet in [
        r#"notmagma.arity:"triage:kind"=scalar"#,
        r#"tagma:"triage:kind"=scalar"#,
        r#"tagmax.arity:"triage:kind"=scalar"#,
        r#"tagma.:"triage:kind"=scalar"#,
    ] {
        assert!(
            matches!(why(not_a_facet), Why::NotAFacet { .. }),
            "{not_a_facet}"
        );
    }
    for unknown in [
        r#"tagma.arty:"triage:kind"=scalar"#,
        r#"tagma.priority-fn:"triage:kind"="1""#,
        r#"tagma.enums:"triage:kind"="a,b""#,
        r#"tagma.Arity:"triage:kind"=scalar"#,
    ] {
        assert!(
            matches!(why(unknown), Why::UnknownFacet { .. }),
            "{unknown}"
        );
    }
    assert_eq!(why(r#"tagma.arity:"triage:kind""#), Why::NoValue);
    for target in [
        "tagma.arity:area=scalar",
        "tagma.arity:repo=scalar",
        r#"tagma.arity:"triage:kind=defect"=scalar"#,
        r#"tagma.arity:"!!"=scalar"#,
    ] {
        assert!(matches!(why(target), Why::ArityTarget { .. }), "{target}");
    }
    let error = TagSchema::parse([" tagma.enums:k=v "]).unwrap_err();
    assert_eq!(error.declaration, "tagma.enums:k=v", "named, trimmed");
    assert!(error.to_string().contains("tagma.enums:k=v"), "{error}");
    assert!(error.to_string().contains(&FACETS.join(", ")), "{error}");
}

// Go refused these only when a tag first met them; here the schema is
// refused when it is read, so no write or lint runs against it.
#[test]
fn test_an_unreadable_enum_or_range_is_refused_when_parsed() {
    for empty in [
        r#"tagma.enum:"triage:type"=",""#,
        r#"tagma.enum:"triage:type"=" , , ""#,
        r#"tagma.enum:"triage:type"="""#,
    ] {
        assert!(matches!(why(empty), Why::EnumEmpty { .. }), "{empty}");
    }
    for range in [
        r#"tagma.range:"triage:effort"=x"#,
        r#"tagma.range:"triage:effort"="0,5,9""#,
        r#"tagma.range:"triage:effort"="5,0""#,
        r#"tagma.range:"triage:effort"="0,inf""#,
        r#"tagma.range:"triage:effort"="NaN,1""#,
        r#"tagma.range:"triage:effort"="0""#,
    ] {
        assert!(matches!(why(range), Why::Range { .. }), "{range}");
    }
    assert_eq!(
        parse(&[r#"tagma.range:"t:x"=" -1.5 , 2e1 ""#]).range_of("t:x"),
        Some(Range {
            min: -1.5,
            max: 20.0
        })
    );
}

#[test]
fn test_every_facet_is_accepted() {
    for facet in FACETS {
        let value = if facet == "enum" {
            "a"
        } else if facet == "range" {
            r#""0,1""#
        } else {
            "true"
        };
        let declaration = format!(r#"tagma.{facet}:"triage:kind"={value}"#);
        assert!(TagSchema::parse([&declaration]).is_ok(), "{declaration}");
    }
}

#[test]
fn test_the_default_schema_parses_and_a_project_extends_it() {
    let schema = TagSchema::resolved(&[]).unwrap();
    assert_eq!(schema, parse(&DEFAULT_TAG_SCHEMA));
    for scalar in [
        "triage:level",
        "triage:verdict",
        "triage:audited",
        "triage:kind",
        "triage:effort",
        "triage:blocks-release",
    ] {
        assert!(schema.is_scalar(scalar), "{scalar}");
    }
    assert_eq!(schema.type_of("triage:blocks-release"), Some("semver"));
    assert_eq!(
        schema.range_of("triage:level"),
        Some(Range { min: 1.0, max: 5.0 })
    );
    assert_eq!(schema.priority_fns().len(), 1);
    assert_eq!(schema.decay_fns().len(), 1);

    let extended = TagSchema::resolved(&[
        r#"tagma.range:"triage:level"="1,9""#.to_string(),
        r#"tagma.arity:"area:x"=scalar"#.to_string(),
    ])
    .unwrap();
    assert_eq!(
        extended.range_of("triage:level"),
        Some(Range { min: 1.0, max: 9.0 })
    );
    assert!(extended.is_scalar("area:x") && extended.is_scalar("triage:kind"));
}

#[test]
fn test_each_refusal_has_its_code_and_args() {
    for sample in super::samples() {
        assert!(sample.code().starts_with("tasks-application-schema-"));
        assert_eq!(sample.args()[0].0, "declaration");
    }
}
