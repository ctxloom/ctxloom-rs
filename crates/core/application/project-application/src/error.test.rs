//! The errors' English, the wrapper's delegation, and the catalog.

use annals_diagnostic::{Arg, Diagnostic, Report};

use super::{diagnostic_samples, ProjectError};

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_the_wrapper_is_its_causes_diagnostic() {
    let wrapper =
        ProjectError::Port(Report::new("project-registry-io", "/x: denied").arg("path", "/x"));
    assert_eq!(wrapper.code(), "project-registry-io");
    assert_eq!(wrapper.to_string(), "/x: denied");
    assert_eq!(arg_names(&wrapper), ["path"]);
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    let english: Vec<String> = diagnostic_samples().iter().map(|d| d.to_string()).collect();
    assert_eq!(
        english,
        [
            "no project tpyo-id is registered or has a database: nothing to repoint to",
            "project jolly-mute-magma is still the project of /work/old, which exists: pass --share if the two trees are meant to share it",
            "jolly-mute-magma is not an orphaned database: a live registered path, or this directory, still points at it, or it does not exist",
        ]
    );
}

#[test]
fn test_each_error_gives_its_args_values() {
    let values = |d: &dyn Diagnostic| -> Vec<String> {
        d.args()
            .into_iter()
            .map(|(_, arg)| match arg {
                Arg::Str(s) => s,
                other => panic!("{other:?}"),
            })
            .collect()
    };
    let samples = diagnostic_samples();
    assert_eq!(values(&*samples[0]), ["tpyo-id"]);
    assert_eq!(values(&*samples[1]), ["jolly-mute-magma", "/work/old"]);
    assert_eq!(values(&*samples[2]), ["jolly-mute-magma"]);
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
