//! The errors' English and the catalog.

use annals_diagnostic::Diagnostic;

use super::diagnostic_samples;

fn arg_names(d: &dyn Diagnostic) -> Vec<&'static str> {
    d.args().into_iter().map(|(name, _)| name).collect()
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    let english: Vec<String> = diagnostic_samples().iter().map(|d| d.to_string()).collect();
    let path = "/home/u/.taskloom/config.yaml";
    assert_eq!(
        english,
        [
            format!("cannot read {path}: permission denied"),
            format!("{path} is not taskloom's config: unknown field `homeing`"),
            format!("{path} is schema_version 2, newer than the 1 this taskloom reads"),
            "TASKLOOM_CONFIG_TAGS sets tags, which is no config key taskloom knows (known: homing, schema_version, tag_schema)".to_string(),
            "--config-set tag_schema: expected <path>=<value>".to_string(),
            "--config-set: \"a,b has an unterminated quote".to_string(),
        ]
    );
}

#[test]
fn test_each_error_has_its_own_code() {
    let mut codes: Vec<&str> = diagnostic_samples().iter().map(|d| d.code()).collect();
    let all = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), all);
    assert!(codes.iter().all(|c| c.starts_with("tasks-config-")));
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
