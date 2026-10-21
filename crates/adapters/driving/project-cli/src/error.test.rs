//! The errors' English, the wrapper's delegation, and the catalog.

use annals_diagnostic::{Arg, Diagnostic};
use project_application::ProjectError;

use super::{diagnostic_samples, CliError};

#[test]
fn test_the_wrapper_is_its_causes_diagnostic() {
    let cause = ProjectError::NotOrphan {
        id: "x".to_string(),
    };
    let wrapper = CliError::Project(cause.clone());
    assert_eq!(wrapper.code(), cause.code());
    assert_eq!(wrapper.to_string(), cause.to_string());
    assert_eq!(wrapper.args().len(), 1);
}

#[test]
fn test_output_renders_its_english_and_its_args() {
    let output = &diagnostic_samples()[0];
    assert_eq!(output.to_string(), "cannot write the output: broken pipe");
    assert!(matches!(&output.args()[..], [("detail", Arg::Str(d))] if d == "broken pipe"));
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let ftl = include_str!("../locales/en-US/project-cli.ftl");
    let resource = fluent_syntax::parser::parse(ftl).expect("the catalog parses");
    for sample in diagnostic_samples() {
        let message = resource
            .body
            .iter()
            .find_map(|e| match e {
                Entry::Message(m) if m.id.name == sample.code() => Some(m),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no message for {}", sample.code()));
        let used: Vec<&str> = message
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
        let args: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        assert_eq!(used, args, "{}", sample.code());
    }
    assert_eq!(resource.body.len(), diagnostic_samples().len());
}
