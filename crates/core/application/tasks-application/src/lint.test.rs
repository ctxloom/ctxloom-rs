//! Lint over rows and schemas: Go's lint_test.go cases, as far as they
//! can arise here.

use std::path::PathBuf;

use annals::next::Timestamp;
use annals_diagnostic::Diagnostic;

use super::{lint, Finding, Violation, SCHEMA_SUBJECT};
use crate::view::{Row, Task};
use crate::{Project, TagSchema};

fn project(id: &str) -> Project {
    Project {
        id: id.to_string(),
        root: String::new(),
        store: PathBuf::new(),
    }
}

/// The task `harp` in `project`, holding `tags`.
fn row(project_id: &str, harp: &str, tags: &[&str]) -> Row {
    Row {
        project: project(project_id),
        task: Task {
            harp_id: harp.to_string(),
            text: "x".to_string(),
            status: "Done".to_string(),
            checked: true,
            text_hash: String::new(),
            trigger: String::new(),
            origin_session: String::new(),
            tags: tags
                .iter()
                .map(|t| crate::parse_tag(t).unwrap().unwrap())
                .collect(),
            created_at: Timestamp::default(),
        },
    }
}

fn schema(declarations: &[&str]) -> TagSchema {
    TagSchema::parse(declarations).unwrap()
}

/// Each violation as `(project, subject, English)`.
fn said(violations: &[Violation]) -> Vec<(Option<&str>, &str, String)> {
    violations
        .iter()
        .map(|v| {
            (
                v.project.as_deref(),
                v.harp_id.as_str(),
                v.finding.to_string(),
            )
        })
        .collect()
}

#[test]
fn test_a_value_outside_its_enum_is_a_violation_whatever_the_tasks_status() {
    let schema = schema(&[r#"tagma.enum:"area:x"="a,b""#]);
    let (found, checked) = lint(&[row("p", "done-task", &["area:x=c", "area:x=a"])], &schema);
    assert_eq!(
        said(&found),
        [(
            Some("p"),
            "done-task",
            "area:x=c is not one of the declared enum values: a, b".to_string()
        )]
    );
    assert_eq!(checked, 1);
    assert_eq!(
        found[0].finding.code(),
        "tasks-application-lint-not-in-enum"
    );
}

#[test]
fn test_a_range_targets_value_must_be_a_finite_number_within_it() {
    let schema = schema(&[r#"tagma.range:"area:n"="1,5""#]);
    let rows = [
        row("p", "a-task", &["area:n=9"]),
        row("p", "b-task", &["area:n=NaN"]),
        row("p", "c-task", &["area:n=inf", "area:n=lots"]),
        row("p", "d-task", &["area:n=1", "area:n=5", "area:n=2.5"]),
    ];
    let (found, checked) = lint(&rows, &schema);
    let codes: Vec<(&str, &str)> = found
        .iter()
        .map(|v| (v.harp_id.as_str(), v.finding.code()))
        .collect();
    assert_eq!(
        codes,
        [
            ("a-task", "tasks-application-lint-out-of-range"),
            ("b-task", "tasks-application-lint-not-a-number"),
            ("c-task", "tasks-application-lint-not-a-number"),
            ("c-task", "tasks-application-lint-not-a-number"),
        ]
    );
    assert_eq!(
        found[0].finding.to_string(),
        "area:n=9 is outside the declared range [1, 5]"
    );
    assert_eq!(checked, 1);
}

#[test]
fn test_a_scalar_target_holding_two_values_is_a_violation() {
    let schema = schema(&[r#"tagma.arity:"area:s"=scalar"#]);
    let rows = [
        row(
            "p",
            "two",
            &["area:s=1", "area:s=2", "area:t=1", "area:t=2"],
        ),
        row("p", "one", &["area:s=1", "area:s"]),
    ];
    let (found, checked) = lint(&rows, &schema);
    assert_eq!(
        said(&found),
        [(
            Some("p"),
            "two",
            "area:s carries 2 distinct values (1, 2) but is declared arity=scalar".to_string()
        )]
    );
    assert_eq!(checked, 0, "arity is no enum or range");
    let args = found[0].finding.args();
    assert!(matches!(
        args[1],
        ("count", annals_diagnostic::Arg::Uint(2))
    ));
}

// Go's TestLint_FormulaEnumReference: a formula naming a value its
// target's enum lacks is the schema's defect, reported once.
#[test]
fn test_a_formula_naming_a_value_no_enum_declares_is_the_schemas_violation() {
    let schema = schema(&[
        r#"tagma.enum:"area:k"="a,b""#,
        r#"tagma.priority_fn:"area:k"="{{area:k=a}} + {{ area:k=zz }} + {{area:k=zz}} + {{area:k=*}} + {{area:other=q}} + {{age_days}}""#,
        r#"tagma.decay_fn:"area:k"="{{area:k=yy}}""#,
    ]);
    let (found, _) = lint(&[], &schema);
    assert_eq!(
        said(&found),
        [
            (
                None,
                SCHEMA_SUBJECT,
                "decay_fn on area:k references the placeholder {{area:k=yy}}, but yy is not one of area:k's declared enum values: a, b".to_string()
            ),
            (
                None,
                SCHEMA_SUBJECT,
                "priority_fn on area:k references the placeholder {{area:k=zz}}, but zz is not one of area:k's declared enum values: a, b".to_string()
            ),
        ],
        "sorted by English, the repeat once"
    );
}

#[test]
fn test_the_default_schemas_formulas_name_only_declared_values() {
    let schema = TagSchema::resolved(&[]).unwrap();
    let (found, checked) = lint(&[], &schema);
    assert!(found.is_empty(), "{:?}", said(&found));
    assert!(checked > 0);
}

#[test]
fn test_violations_sort_by_project_then_task_and_nothing_checked_is_counted() {
    let schema = schema(&[
        r#"tagma.enum:"area:x"="a""#,
        r#"tagma.range:"area:x"="0,1""#,
    ]);
    let rows = [
        row("zulu", "a-task", &["area:x=b"]),
        row("alpha", "z-task", &["area:x=b"]),
    ];
    let (found, checked) = lint(&rows, &schema);
    let order: Vec<(Option<&str>, &str)> = found
        .iter()
        .map(|v| (v.project.as_deref(), v.harp_id.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            (Some("alpha"), "z-task"),
            (Some("alpha"), "z-task"),
            (Some("zulu"), "a-task"),
            (Some("zulu"), "a-task")
        ]
    );
    assert_eq!(checked, 1, "one target, two facets");
    assert_eq!(lint(&rows, &TagSchema::default()), (Vec::new(), 0));
}

#[test]
fn test_each_finding_names_its_args() {
    let finding = Finding::FormulaEnum {
        facet: "priority_fn".to_string(),
        target: "t".to_string(),
        placeholder: "{{r=v}}".to_string(),
        reference: "r".to_string(),
        value: "v".to_string(),
        members: "a".to_string(),
    };
    let names: Vec<&str> = finding.args().into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        [
            "facet",
            "target",
            "placeholder",
            "reference",
            "value",
            "members"
        ]
    );
}
