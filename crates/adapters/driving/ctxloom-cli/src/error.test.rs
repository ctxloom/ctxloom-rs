//! The command line's own errors: their codes, args and exit codes (D28).

use angzarr_diagnostic::Diagnostic;

use super::CliError;

#[test]
fn a_usage_error_is_a_refusal_with_its_detail() {
    let error = CliError::Usage {
        detail: "unexpected argument".to_string(),
    };
    assert_eq!((error.code(), error.exit_code()), ("ctxloom-cli-usage", 2));
    assert_eq!(error.args()[0].0, "ext_err_txt");
}

#[test]
fn an_unknown_verb_and_an_unknown_format_are_refusals() {
    let verb = CliError::UnknownVerb {
        verb: "x".to_string(),
        command: "ctxloom".to_string(),
        choices: "version".to_string(),
    };
    assert_eq!(
        (verb.code(), verb.exit_code()),
        ("ctxloom-cli-unknown-verb", 2)
    );
    let names: Vec<&str> = verb.args().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["verb", "command", "choices"]);
    let format = CliError::Format {
        value: "yaml".to_string(),
    };
    assert_eq!(
        (format.code(), format.exit_code()),
        ("ctxloom-cli-unknown-format", 2)
    );
    assert_eq!(format.args()[0].0, "value");
}

#[test]
fn an_output_error_fails_with_1() {
    let error = CliError::Output {
        detail: "broken pipe".to_string(),
    };
    assert_eq!((error.code(), error.exit_code()), ("ctxloom-cli-output", 1));
}

// A command's own failure is its diagnostic, with the exit code it asks for.
#[test]
fn a_commands_failure_is_its_diagnostic_and_its_exit_code() {
    let inner = CliError::Format {
        value: "yaml".to_string(),
    };
    let error = CliError::Command {
        error: Box::new(inner),
        exit: 7,
    };
    assert_eq!(error.exit_code(), 7);
    assert_eq!(error.code(), "ctxloom-cli-unknown-format");
    assert_eq!(error.args().len(), 1);
}

// A catalog test elsewhere renders these: one value of each error here.
#[test]
fn the_samples_are_one_of_each_error_here() {
    let codes: Vec<String> = super::diagnostic_samples()
        .iter()
        .map(|sample| sample.code().to_string())
        .collect();
    assert_eq!(
        codes,
        [
            "ctxloom-cli-usage",
            "ctxloom-cli-unknown-verb",
            "ctxloom-cli-unknown-format",
            "ctxloom-cli-output"
        ]
    );
}
