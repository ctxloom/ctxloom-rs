//! The errors' English and the catalog.

use annals_diagnostic::{Arg, Diagnostic};

use super::{diagnostic_samples, StoreOpenError};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    let english: Vec<String> = diagnostic_samples().iter().map(|d| d.to_string()).collect();
    assert_eq!(
        english,
        [
            "cannot create the store's directory /home/u/.ctxloom/projects/jolly-mute-magma: permission denied",
            "cannot start a runtime to check the store: too many open files",
        ]
    );
}

#[test]
fn test_each_error_gives_its_args_values() {
    let error = StoreOpenError::CreateDir {
        path: "p".to_string(),
        detail: "d".to_string(),
    };
    assert!(
        matches!(&error.args()[..], [("path", Arg::Str(p)), ("detail", Arg::Str(d))] if p == "p" && d == "d"),
        "{error:?}"
    );
    let error = StoreOpenError::NoRuntime {
        detail: "d".to_string(),
    };
    assert!(
        matches!(&error.args()[..], [("detail", Arg::Str(d))] if d == "d"),
        "{error:?}"
    );
    assert_eq!(error.code(), "task-store-sqlite-no-runtime");
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
