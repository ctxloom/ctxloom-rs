//! The errors' English and the catalog.

use annals_diagnostic::{Diagnostic, Report};

use super::{diagnostic_samples, RegistrationError};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_each_error_is_gos_message() {
    let unknown = RegistrationError::UnknownEngine {
        engine: "foo".to_string(),
        known: "claude-code".to_string(),
    };
    assert_eq!(
        unknown.to_string(),
        "unknown engine \"foo\"; known engines: claude-code"
    );
    assert_eq!(arg_names(&unknown), ["engine", "known"]);
    let failed = RegistrationError::EngineFailed {
        engine: "claude-code".to_string(),
        error: Report::new("x", "broken"),
    };
    assert_eq!(failed.to_string(), "claude-code: broken");
    assert_eq!(arg_names(&failed), ["engine", "error"]);
    assert_eq!(
        arg_names(&RegistrationError::NoBackends {
            known: String::new()
        }),
        ["known"]
    );
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let resource = fluent_syntax::parser::parse(crate::EN_US.1).expect("the catalog parses");
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
