//! The errors' English and the catalog.

use annals_diagnostic::Diagnostic;

use registration_application::RegistrationError;

use super::{diagnostic_samples, ManageError, ManageNotice};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_go_says_it_this_way() {
    let warning = ManageNotice::Unresolvable {
        command: "taskloom".to_string(),
    };
    assert_eq!(
        warning.to_string(),
        "warning: \"taskloom\" does not resolve on this process's PATH: exec: \"taskloom\": executable file not found in $PATH"
    );
    assert_eq!(arg_names(&warning), ["command"]);
    assert_eq!(
        ManageNotice::NothingToRemove.to_string(),
        "nothing to remove (no agent backends detected; name one with --engine)"
    );
    let app = RegistrationError::NoBackends {
        known: "claude-code".to_string(),
    };
    assert_eq!(ManageError::App(app.clone()).code(), app.code());
    assert_eq!(ManageError::App(app.clone()).to_string(), app.to_string());
    assert_eq!(arg_names(&ManageError::App(app)), ["known"]);
}

#[test]
fn test_each_diagnostics_english_is_its_catalog_message() {
    let catalog = presentation::Catalog::en_us(&[crate::EN_US, registration_application::EN_US])
        .expect("the catalog loads");
    for sample in diagnostic_samples() {
        assert_eq!(catalog.render(&*sample), Ok(sample.to_string()));
    }
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
        used.dedup();
        let mut args = arg_names(&*sample);
        args.sort_unstable();
        assert_eq!(used, args, "{}", sample.code());
    }
}
