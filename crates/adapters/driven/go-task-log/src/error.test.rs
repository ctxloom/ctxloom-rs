//! The errors' English and the catalog.

use annals_diagnostic::{Arg, Diagnostic};

use super::diagnostic_samples;

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    let english: Vec<String> = diagnostic_samples().iter().map(|d| d.to_string()).collect();
    assert_eq!(
        english,
        [
            "HOME is not set, so Go's task logs cannot be found",
            "cannot read /home/u/.ctxloom/tasks/jolly-mute-magma.jsonl: permission denied",
            "/home/u/.ctxloom/tasks/jolly-mute-magma.jsonl:3: malformed task event: expected value at line 1 column 1",
            "/home/u/.ctxloom/tasks/jolly-mute-magma.jsonl:4: record has no op field",
            "/home/u/.ctxloom/tasks/jolly-mute-magma.jsonl:5: unrecognized op archive: the log was likely written by a newer taskloom",
        ]
    );
}

#[test]
fn test_each_error_gives_its_args_values() {
    let samples = diagnostic_samples();
    let values: Vec<Vec<String>> = samples
        .iter()
        .map(|d| {
            d.args()
                .into_iter()
                .map(|(_, arg)| match arg {
                    Arg::Str(s) => s,
                    Arg::Uint(n) => n.to_string(),
                    other => panic!("unexpected arg {other:?}"),
                })
                .collect()
        })
        .collect();
    let path = "/home/u/.ctxloom/tasks/jolly-mute-magma.jsonl";
    assert_eq!(
        values,
        [
            vec![],
            vec![path, "permission denied"],
            vec![path, "3", "expected value at line 1 column 1"],
            vec![path, "4"],
            vec![path, "5", "archive"],
        ]
    );
}

#[test]
fn test_each_error_has_its_own_code() {
    let codes: Vec<&str> = diagnostic_samples().iter().map(|d| d.code()).collect();
    assert_eq!(
        codes,
        [
            "go-task-log-no-home",
            "go-task-log-unreadable",
            "go-task-log-malformed",
            "go-task-log-no-op",
            "go-task-log-unknown-op",
        ]
    );
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let ftl = crate::EN_US.1;
    let resource = fluent_syntax::parser::parse(ftl).expect("the catalog parses");
    let samples = diagnostic_samples();
    let mut catalog: Vec<&str> = resource
        .body
        .iter()
        .filter_map(|e| match e {
            Entry::Message(m) => Some(m.id.name),
            _ => None,
        })
        .collect();
    catalog.sort_unstable();
    let mut sampled: Vec<&str> = samples.iter().map(|s| s.code()).collect();
    sampled.sort_unstable();
    assert_eq!(catalog, sampled, "catalog messages and samples");
    for sample in samples {
        let message = resource
            .body
            .iter()
            .find_map(|e| match e {
                Entry::Message(m) if m.id.name == sample.code() => Some(m),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no message for {}", sample.code()));
        let mut used: Vec<&str> = message
            .value
            .iter()
            .flat_map(|p| &p.elements)
            .filter_map(|el| match el {
                PatternElement::Placeable {
                    expression: Expression::Inline(InlineExpression::VariableReference { id }),
                } => Some(id.name),
                _ => None,
            })
            .collect();
        used.sort_unstable();
        let mut args = arg_names(&*sample);
        args.sort_unstable();
        assert_eq!(used, args, "{}", sample.code());
    }
}
