//! The errors' codes, exit codes, args and messages.

use angzarr_diagnostic::Diagnostic;
use sessions_application::AppError;

use super::*;

angzarr_presentation::english_for!(&[EN_US.1]);

fn bad_age() -> ReclaimError {
    ReclaimError::BadAge {
        flag: "--older-than".into(),
        value: "soon".into(),
    }
}

fn no_session() -> AppError {
    AppError::NoSession {
        harp: "amber-quiet-heron".into(),
    }
}

#[test]
fn a_bad_age_is_a_usage_error_with_its_own_code_and_args() {
    let error = bad_age();
    assert_eq!(error.exit_code(), 2);
    assert_eq!(error.code(), "reclaim-application-bad-age");
    let args: Vec<&str> = error.args().iter().map(|(k, _)| *k).collect();
    assert_eq!(args, ["flag", "value"]);
    let english = error.english();
    assert!(english.contains("--older-than") && english.contains("soon"));
    assert!(error.to_string().contains("bad-age"));
}

#[test]
fn a_sessions_failure_keeps_its_code_args_and_exit_code() {
    let cause = no_session();
    let error = ReclaimError::from(cause.clone());
    assert_eq!(error.exit_code(), cause.exit_code());
    assert_eq!(error.code(), cause.code());
    assert_eq!(error.args().len(), cause.args().len());
    assert!(!error.args().is_empty());
}

#[test]
fn a_cache_failure_exits_one_with_its_causes_code_and_args() {
    let error = ReclaimError::Cache(Box::new(bad_age()));
    assert_eq!(error.exit_code(), 1);
    assert_eq!(error.code(), "reclaim-application-bad-age");
    assert_eq!(error.args().len(), 2);
}

#[test]
fn the_catalog_has_a_message_for_every_sample() {
    let samples = diagnostic_samples();
    assert!(!samples.is_empty());
    for sample in samples {
        assert!(EN_US.1.contains(&format!("{} =", sample.code())));
    }
}

#[test]
fn an_unreadable_configuration_exits_one_with_its_causes_code_and_args() {
    let error = ReclaimError::Config(Box::new(bad_age()));
    assert_eq!(error.exit_code(), 1);
    assert_eq!(error.code(), "reclaim-application-bad-age");
    assert_eq!(error.args().len(), 2);
}
