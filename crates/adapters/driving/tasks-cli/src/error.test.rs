//! The errors' English, the wrapper's delegation, and the catalog.

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

use super::{diagnostic_samples, CliError};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_a_use_case_error_is_its_own_diagnostic() {
    let app = AppError::HarpsExhausted { attempts: 100 };
    let error = CliError::App(app.clone());
    assert_eq!(error.code(), app.code());
    assert_eq!(error.to_string(), app.to_string());
    assert_eq!(arg_names(&error), ["attempts"]);
}

#[test]
fn test_an_output_failure_says_what_failed() {
    let error = CliError::Output {
        detail: "broken pipe".to_string(),
    };
    assert_eq!(error.to_string(), "cannot write the output: broken pipe");
    assert!(matches!(&error.args()[..], [("detail", Arg::Str(d))] if d == "broken pipe"));
}

#[test]
fn test_an_incomplete_import_says_what_it_left() {
    let error = CliError::Incomplete {
        stopped: 1,
        lost: 2,
    };
    assert_eq!(
        error.to_string(),
        "not everything was imported: 1 projects stopped, 2 records not imported"
    );
    assert!(matches!(
        &error.args()[..],
        [("stopped", Arg::Uint(1)), ("lost", Arg::Uint(2))]
    ));
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

#[test]
fn test_each_diagnostics_english_is_its_catalog_message() {
    let catalog = crate::catalog(&[]).expect("the catalog loads");
    for sample in diagnostic_samples() {
        assert_eq!(catalog.render(&*sample), Ok(sample.to_string()));
    }
}

#[test]
fn test_a_usage_error_is_clap_s_words() {
    let error = CliError::Usage {
        detail: "error: no\n".to_string(),
    };
    assert_eq!(error.code(), "tasks-cli-usage");
    assert_eq!(error.to_string(), "error: no\n");
    assert!(matches!(&error.args()[..], [("detail", Arg::Str(d))] if d == "error: no\n"));
}

#[test]
fn test_a_project_notice_names_its_root_and_id() {
    let notice = crate::Notice::Project {
        root: "/w".to_string(),
        id: "a-b-c".to_string(),
    };
    assert_eq!(notice.to_string(), "project /w (a-b-c)");
    assert_eq!(arg_names(&notice), ["root", "id"]);
    assert_eq!(crate::Notice::NoProject.code(), "tasks-cli-no-project");
}
