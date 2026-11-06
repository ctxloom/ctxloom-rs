//! A test error's code and arguments: what the catalog renders it from.

use super::*;

#[test]
fn each_test_error_has_its_stable_code_and_its_arguments() {
    let samples = diagnostic_samples();
    let seen: Vec<(String, Vec<String>)> = samples
        .iter()
        .map(|d| {
            (
                d.code().to_string(),
                d.args().iter().map(|(name, _)| name.to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                "ctxloom-wiring-seed".to_string(),
                vec!["ext_err_txt".to_string()]
            ),
            (
                "ctxloom-wiring-test-refused".to_string(),
                vec!["verb".to_string()]
            ),
            (
                "ctxloom-wiring-test-findings".to_string(),
                vec!["count".to_string()]
            ),
        ]
    );
}

#[test]
fn a_test_error_displays_as_its_code_and_arguments() {
    let shown = TestError::Findings { count: 7 }.to_string();
    assert!(shown.contains("ctxloom-wiring-test-findings"), "{shown}");
    assert!(shown.contains('7'), "{shown}");
}
