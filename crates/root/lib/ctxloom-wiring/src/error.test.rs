//! A wiring error's text and arguments: what the catalog renders it from.

use std::fmt::Write as _;

use angzarr_diagnostic::{Arg, Diagnostic};

use super::LibError;

fn text(args: Vec<(&str, Arg)>) -> String {
    let mut text = String::new();
    for (name, arg) in args {
        let _ = write!(text, "{name}={arg:?};");
    }
    text
}

#[test]
fn an_error_displays_as_its_code_and_arguments_not_as_nothing() {
    let shown = LibError::NoHome {
        variable: "HOME".to_string(),
    }
    .to_string();
    assert!(shown.contains("ctxloom-wiring-no-home"), "{shown}");
    assert!(shown.contains("HOME"), "{shown}");
}

#[test]
fn each_error_carries_the_arguments_its_message_names() {
    let no_home = LibError::NoHome {
        variable: "HOME".to_string(),
    };
    assert!(text(no_home.args()).contains("variable"));
    assert!(text(no_home.args()).contains("HOME"));
    let bad = LibError::BadSession {
        session: "jolly".to_string(),
        detail: "why".to_string(),
    };
    let args = text(bad.args());
    assert!(args.contains("session") && args.contains("jolly"), "{args}");
    assert!(
        args.contains("ext_err_txt") && args.contains("why"),
        "{args}"
    );
    let runtime = LibError::NoRuntime {
        detail: "no threads".to_string(),
    };
    let args = text(runtime.args());
    assert!(
        args.contains("ext_err_txt") && args.contains("no threads"),
        "{args}"
    );
    for sample in super::diagnostic_samples() {
        assert!(
            !sample.args().is_empty(),
            "{} carries its arguments",
            sample.code()
        );
    }
}

#[test]
fn the_samples_are_the_same_whether_or_not_test_support_is_built() {
    // A cargo feature is additive: it adds code, never a variant. The test
    // commands' own errors are `TestError`s, not `LibError`s.
    let codes: Vec<String> = super::diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    assert_eq!(
        codes,
        [
            "ctxloom-wiring-no-home",
            "ctxloom-wiring-bad-session",
            "ctxloom-wiring-no-runtime",
            "ctxloom-wiring-store",
            "ctxloom-wiring-store",
        ]
    );
}
