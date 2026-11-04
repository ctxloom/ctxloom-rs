//! Every error and notice names itself, shows its args, and reads in English.

use angzarr_diagnostic::{Arg, Diagnostic};

use super::{diagnostic_samples, version_args, AppError, Notice, EN_US};

angzarr_presentation::english_for!(&[EN_US.1]);

fn english(d: &dyn Diagnostic) -> String {
    d.english()
}

// plan §2 (14.37): a diagnostic's code is crate-prefixed, its message is in
// the catalog (not the bare key), and every sample is a distinct variant.
#[test]
fn test_every_sample_has_a_prefixed_code_and_an_english_message() {
    let samples = diagnostic_samples();
    assert!(samples.len() >= 40, "{} samples", samples.len());
    let mut codes = Vec::new();
    for sample in &samples {
        let code = sample.code().to_string();
        let own = code.starts_with("library-application-");
        let nested = code.starts_with("library-") || code.starts_with("ctxloom-");
        assert!(own || nested, "{code}");
        let said = english(sample.as_ref());
        assert!(!said.is_empty() && said != code, "{code}: {said}");
        codes.push(code);
    }
    let mut own: Vec<&String> = codes
        .iter()
        .filter(|c| c.starts_with("library-application-"))
        .collect();
    let total = own.len();
    own.sort();
    own.dedup();
    assert_eq!(own.len(), total, "a code is used twice");
}

// config_version_refusal_test.go (owner override, D15): the three refusals
// carry the path and the numbers, in their display and their English.
#[test]
fn test_the_generation_refusals_show_their_path_and_numbers() {
    let newer = AppError::ConfigNewer {
        path: "/p/c.yaml".to_string(),
        found: 9,
        current: 8,
    };
    assert_eq!(newer.code(), "library-application-config-newer");
    assert_eq!(
        newer.to_string(),
        "library-application-config-newer path=/p/c.yaml found=9 current=8"
    );
    assert!(english(&newer).contains("schema_version 9"));
    let older = AppError::ConfigOlder {
        path: "/p/c.yaml".to_string(),
        found: 6,
        oldest: 7,
    };
    assert_eq!(older.code(), "library-application-config-older");
    assert_eq!(
        older.to_string(),
        "library-application-config-older path=/p/c.yaml found=6 oldest=7"
    );
    let unreadable = AppError::ConfigVersionUnreadable {
        path: "/p/c.yaml".to_string(),
    };
    assert_eq!(
        unreadable.code(),
        "library-application-config-version-unreadable"
    );
    assert_eq!(
        unreadable.to_string(),
        "library-application-config-version-unreadable path=/p/c.yaml"
    );
    for error in [&newer, &older, &unreadable] {
        let names: Vec<&str> = version_args(error).iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            error.args().iter().map(|(n, _)| *n).collect::<Vec<_>>()
        );
        assert!(!names.is_empty());
    }
    assert!(version_args(&AppError::NoBundlesDir).is_empty());
    assert_eq!(
        version_args(&older)
            .into_iter()
            .map(|(_, a)| matches!(a, Arg::Uint(_) | Arg::Int(_)))
            .collect::<Vec<_>>(),
        [false, true, true]
    );
}

// An error carries its args into its display and its English.
#[test]
fn test_an_app_error_shows_its_args() {
    let error = AppError::UnknownSection {
        section: "nonsense".to_string(),
        available: "a, b".to_string(),
    };
    assert_eq!(error.code(), "library-application-unknown-section");
    assert!(error
        .to_string()
        .starts_with("library-application-unknown-section "));
    assert!(error.to_string().contains("nonsense"));
    assert!(error.args().iter().any(|(n, _)| *n == "section"));
    assert!(english(&error).contains("nonsense"));
}

// Notices: code, args, display.
#[test]
fn test_notices_name_themselves_and_show_their_args() {
    let finding = Notice::Finding {
        text: "a thing".to_string(),
        remedy: "do a thing".to_string(),
    };
    assert_eq!(finding.code(), "library-application-notice-finding");
    assert_eq!(
        finding.to_string(),
        "library-application-notice-finding text=\"a thing\" remedy=\"do a thing\""
    );
    let keyless = Notice::ConfigKeyless {
        path: "/p/c.yaml".to_string(),
    };
    assert_eq!(keyless.code(), "library-application-notice-config-keyless");
    assert_eq!(
        keyless.to_string(),
        "library-application-notice-config-keyless path=/p/c.yaml"
    );
    let root = Notice::RootIgnored {
        root: "/nowhere".to_string(),
    };
    assert_eq!(root.code(), "library-application-notice-root-ignored");
    assert_eq!(
        root.to_string(),
        "library-application-notice-root-ignored root=/nowhere"
    );
    assert!(english(&root).contains("/nowhere"));
    assert!(english(&finding).contains("a thing"));
}
