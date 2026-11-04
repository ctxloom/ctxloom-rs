//! The codes, the args and the exit codes of the sessions use cases' errors.
//! Go: `errNoSession` (not_found.go), `harpPurgeRefusal` (session_purge_cmd.go),
//! `reportPlanOnly` and session.feature's "removed nothing" scenarios,
//! release/0.7.

use angzarr_diagnostic::{Diagnostic, Report};
use sessions::SessionRejection;

use super::{diagnostic_samples, AppError, Refusal, RemovedNothing, EN_US};

#[test]
fn the_codes_are_prefixed_with_the_crates_name() {
    let (name, _) = EN_US;
    assert_eq!(name, "sessions-application");
    for sample in diagnostic_samples() {
        assert!(
            sample.code().starts_with("sessions-application-"),
            "{}",
            sample.code()
        );
    }
}

// Go: errNoSession: `no session named "x"`, with a fix naming the listing.
#[test]
fn no_session_names_the_harp() {
    let error = AppError::NoSession {
        harp: "no-such-harp".to_string(),
    };
    assert_eq!(error.code(), "sessions-application-no-session");
    assert_eq!(error.args().len(), 1);
    assert_eq!(error.exit_code(), 1);
}

// A wrapper is its cause's diagnostic.
#[test]
fn a_wrapper_is_its_causes_diagnostic() {
    let rejection = SessionRejection::HarpInvalid {
        harp: "a/b".to_string(),
        reason: "separator".to_string(),
    };
    let error = AppError::Rejected(Report::of(&rejection));
    assert_eq!(error.code(), rejection.code());
    assert_eq!(error.args().len(), rejection.args().len());
    let failed = AppError::Failed(Report::new("x-busy"));
    assert_eq!(failed.code(), "x-busy");
}

// The owner's rule for exit codes: 2 when the request is invalid on its own
// content (a name no session may have), 1 when it ran against state and the
// answer was negative.
#[test]
fn exit_codes_follow_the_rule() {
    let invalid = AppError::Rejected(Report::of(&SessionRejection::HarpInvalid {
        harp: "a/b".to_string(),
        reason: "separator".to_string(),
    }));
    let too_long = AppError::Rejected(Report::of(&SessionRejection::HarpTooLong {
        harp: "x".repeat(30),
        limit: 24,
        length: 30,
    }));
    let forgotten = AppError::Rejected(Report::of(&SessionRejection::Forgotten {
        harp: "h".to_string(),
    }));
    assert_eq!((invalid.exit_code(), too_long.exit_code()), (2, 2));
    assert_eq!(forgotten.exit_code(), 1);
    assert_eq!(AppError::Failed(Report::new("x")).exit_code(), 1);
}

#[test]
fn a_dead_lettered_error_is_its_errors_diagnostic_with_the_letters_beside_it() {
    let inner = AppError::NoSession {
        harp: "h".to_string(),
    };
    let letter = crate::dead_letter_sample("sessions");
    let error = AppError::DeadLettered {
        error: Box::new(inner.clone()),
        dead_letters: vec![letter.clone()],
    };
    assert_eq!(error.code(), inner.code());
    assert_eq!(error.exit_code(), inner.exit_code());
    assert_eq!(error.dead_letters(), [letter]);
}

// Go: harpPurgeRefusal, each ends with what was NOT done.
#[test]
fn the_refusals_have_codes_and_the_arg_their_fix_needs() {
    let dead = Refusal::NotProvenDead {
        harp: "h".to_string(),
        reason: "no lock file".to_string(),
        verb: "session remove".to_string(),
    };
    assert_eq!(dead.code(), "sessions-application-refused-not-dead");
    let kept = Refusal::Kept {
        harp: "h".to_string(),
    };
    assert_eq!(kept.code(), "sessions-application-refused-kept");
    let retained = Refusal::Retained {
        harp: "h".to_string(),
        detail: "x".to_string(),
    };
    assert_eq!(retained.code(), "sessions-application-refused-retained");
    let nothing = RemovedNothing {
        command: "ctxloom session remove h --yes".to_string(),
    };
    assert_eq!(nothing.code(), "sessions-application-removed-nothing");
}

// Every sample says its code (Display is the code then the args, never
// English), its catalog message uses exactly its args, and the samples are
// not empty.
#[test]
fn every_sample_displays_its_code_and_its_message_uses_exactly_its_args() {
    let catalogs: Vec<&str> = crate::CATALOGS.iter().map(|(_, c)| *c).collect();
    let samples = diagnostic_samples();
    assert!(!samples.is_empty());
    for sample in samples {
        let shown = format!("{sample}");
        assert!(shown.starts_with(sample.code()), "{shown}");
        let names: Vec<&str> = sample.args().into_iter().map(|(n, _)| n).collect();
        assert_eq!(shown.contains('='), !names.is_empty(), "{shown}");
        let line = catalogs
            .iter()
            .flat_map(|c| c.lines())
            .find(|l| l.starts_with(&format!("{} =", sample.code())))
            .unwrap_or_else(|| panic!("{} has no message", sample.code()));
        let mut used: Vec<&str> = line
            .split('$')
            .skip(1)
            .map(|r| {
                r.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .unwrap()
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        let mut names = names;
        names.sort_unstable();
        assert_eq!(names, used, "{}", sample.code());
    }
}
