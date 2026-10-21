//! The errors' English, the wrapper's delegation, and the catalog.

use annals_diagnostic::{Arg, Diagnostic};
use tasks_application::AppError;

use super::{diagnostic_samples, McpError, McpNotice};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

fn catalog() -> presentation::Catalog {
    let mut sources = vec![crate::EN_US];
    sources.extend(tasks_application::CATALOGS);
    presentation::Catalog::en_us(&sources).expect("the catalog loads")
}

#[test]
fn test_a_use_case_error_is_its_own_diagnostic() {
    let app = AppError::HarpsExhausted { attempts: 100 };
    let error = McpError::App(app.clone());
    assert_eq!(error.code(), app.code());
    assert_eq!(error.to_string(), app.to_string());
    assert_eq!(arg_names(&error), ["attempts"]);
}

#[test]
fn test_each_error_names_what_went_wrong() {
    let unknown = McpError::UnknownResource {
        uri: "taskloom://frob".to_string(),
    };
    assert_eq!(unknown.to_string(), "no resource is at taskloom://frob");
    assert!(matches!(
        &unknown.args()[..],
        [("uri", Arg::Str(u))] if u == "taskloom://frob"
    ));
    let arguments = McpError::Arguments {
        tool: "task_add".to_string(),
        detail: "missing field `text`".to_string(),
    };
    assert_eq!(
        arguments.to_string(),
        "invalid arguments to task_add: missing field `text`"
    );
    assert_eq!(arg_names(&arguments), ["tool", "detail"]);
    let crashed = McpError::Crashed {
        detail: "panicked".to_string(),
    };
    assert_eq!(arg_names(&crashed), ["detail"]);
    assert_eq!(McpNotice::NoProject.code(), "tasks-mcp-no-project");
    assert!(McpNotice::NoProject.args().is_empty());
    let every = McpNotice::EveryProject {
        directory: "/tmp/x".to_string(),
    };
    assert_eq!(every.code(), "tasks-mcp-every-project");
    assert!(every
        .to_string()
        .starts_with("no project here: /tmp/x is in no git"));
    assert!(matches!(
        &every.args()[..],
        [("directory", Arg::Str(d))] if d == "/tmp/x"
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
    let catalog = catalog();
    for sample in diagnostic_samples() {
        assert_eq!(catalog.render(&*sample), Ok(sample.to_string()));
    }
}
