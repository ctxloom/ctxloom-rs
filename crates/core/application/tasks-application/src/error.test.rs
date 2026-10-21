//! The errors' English, their wrappers' delegation, and the catalog.

use annals_diagnostic::{Arg, Diagnostic, Report};

use super::{diagnostic_samples, AppError};

fn cause() -> Report {
    Report::new("tasks-text-required", "a task needs text").arg("field", "text")
}

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_a_wrapper_is_its_causes_diagnostic() {
    for wrapper in [
        AppError::Rejected(cause()),
        AppError::Failed(cause()),
        AppError::Identity(cause()),
    ] {
        assert_eq!(wrapper.code(), "tasks-text-required");
        assert_eq!(wrapper.to_string(), "a task needs text");
        assert_eq!(arg_names(&wrapper), ["field"]);
    }
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    assert_eq!(
        AppError::GaveUp {
            attempts: 10,
            error: Report::new("x-busy", "the database is locked"),
        }
        .to_string(),
        "gave up after 10 attempts: the database is locked"
    );
    assert_eq!(
        AppError::HarpsExhausted { attempts: 100 }.to_string(),
        "no free task id after 100 tries"
    );
    assert_eq!(
        AppError::Vanished {
            harp_id: "swift-amber".to_string()
        }
        .to_string(),
        "task swift-amber was written but cannot be read back"
    );
    assert_eq!(
        AppError::NoProject.to_string(),
        "there is no project here yet, so no task to change"
    );
}

#[test]
fn test_each_error_gives_its_args_values() {
    let gave_up = AppError::GaveUp {
        attempts: 7,
        error: cause(),
    };
    let args = gave_up.args();
    assert!(matches!(args[0], ("attempts", Arg::Uint(7))));
    assert!(matches!(&args[1], ("error", Arg::Nested(e)) if e.code() == "tasks-text-required"));
    let exhausted = AppError::HarpsExhausted { attempts: 100 }.args();
    assert!(matches!(exhausted[0], ("attempts", Arg::Uint(100))));
    let vanished = AppError::Vanished {
        harp_id: "calm-otter".to_string(),
    }
    .args();
    assert!(matches!(&vanished[0], ("harp_id", Arg::Str(s)) if s == "calm-otter"));
    assert!(AppError::NoProject.args().is_empty());
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let ftl = include_str!("../locales/en-US/tasks-application.ftl");
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
