//! The project refresh a pull ends with, over a fake delivery.

use std::cell::RefCell;

use angzarr_diagnostic::Diagnostic;

use crate::fakes::Fixture;
use crate::{AppError, ProjectRefresh, Refresh, RefreshEngine, RefusedKind};

const URL: &str = "file:///srv/origin.git";

/// A delivery that answers what it was given and remembers how it was asked.
struct Delivery {
    answer: RefCell<Option<Result<Refresh, Box<dyn Diagnostic>>>>,
    asked: RefCell<Vec<bool>>,
}

impl Delivery {
    fn answering(answer: Result<Refresh, Box<dyn Diagnostic>>) -> Delivery {
        Delivery {
            answer: RefCell::new(Some(answer)),
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl ProjectRefresh for Delivery {
    fn refresh(&self, write: bool) -> Result<Refresh, Box<dyn Diagnostic>> {
        self.asked.borrow_mut().push(write);
        self.answer.borrow_mut().take().expect("asked once")
    }
}

fn writing_hooks() -> Refresh {
    Refresh {
        target: "/p".to_string(),
        engines: vec![RefreshEngine {
            engine: "claude-code".to_string(),
            wrote: vec!["hooks".to_string()],
            ..RefreshEngine::default()
        }],
    }
}

fn unpulled() -> Fixture {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    fx.profile("dev", &[&Fixture::reference(URL, "demo")]);
    fx
}

fn aborted() -> Box<dyn Diagnostic> {
    Box::new(AppError::Required {
        what: "refusal".to_string(),
    })
}

// deps.feature: A pull without --yes installs the closure and leaves the
// project's engine files alone
#[test]
fn test_a_pull_without_consent_previews_the_refresh_and_keeps_what_it_would_change() {
    let fx = unpulled();
    let delivery = Delivery::answering(Ok(writing_hooks()));
    let pulled = fx
        .deps
        .pull_and_refresh(&delivery, false, true, false)
        .result
        .unwrap();
    assert_eq!(*delivery.asked.borrow(), [false]);
    assert_eq!(pulled.report.installed, 1);
    let skipped = pulled.skipped.expect("a refresh skipped");
    assert_eq!(skipped.engines[0].wrote, ["hooks"]);
    assert!(pulled.failure.is_none());
}

// deps.feature: A pull right after a --yes pull has no refresh to skip
#[test]
fn test_a_preview_that_changes_nothing_is_no_note() {
    let fx = unpulled();
    let delivery = Delivery::answering(Ok(Refresh {
        target: "/p".to_string(),
        engines: vec![RefreshEngine::default()],
    }));
    let pulled = fx
        .deps
        .pull_and_refresh(&delivery, false, true, false)
        .result
        .unwrap();
    assert!(pulled.skipped.is_none());
}

// deps.feature: A refresh the project would refuse is named with its fix, and
// the pull still succeeds
#[test]
fn test_a_preview_that_would_be_refused_is_a_note() {
    let fx = unpulled();
    let delivery = Delivery::answering(Ok(Refresh {
        target: "/p".to_string(),
        engines: vec![RefreshEngine {
            engine: "claude-code".to_string(),
            refused: vec![RefusedKind {
                kind: "hooks".to_string(),
                cause: aborted(),
            }],
            ..RefreshEngine::default()
        }],
    }));
    let pulled = fx
        .deps
        .pull_and_refresh(&delivery, false, true, false)
        .result
        .unwrap();
    assert!(pulled.skipped.is_some());
}

// deps.feature: A pull with --yes also refreshes the project's engine files
#[test]
fn test_a_consented_pull_writes_the_refresh_and_has_nothing_to_skip() {
    let fx = unpulled();
    let delivery = Delivery::answering(Ok(writing_hooks()));
    let pulled = fx
        .deps
        .pull_and_refresh(&delivery, false, true, true)
        .result
        .unwrap();
    assert_eq!(*delivery.asked.borrow(), [true]);
    assert!(pulled.skipped.is_none());
    assert!(pulled.failure.is_none());
}

// behaviour-changes-0.7.0: a refused consented refresh lands the pull and
// exits 3 naming the finding.
#[test]
fn test_a_consented_refresh_that_is_refused_fails_the_command_but_not_the_pull() {
    let fx = unpulled();
    let delivery = Delivery::answering(Err(aborted()));
    let pulled = fx
        .deps
        .pull_and_refresh(&delivery, false, true, true)
        .result
        .unwrap();
    assert_eq!(pulled.report.installed, 1);
    let failure = pulled.failure.expect("a failure");
    assert_eq!(failure.exit_code(), 3);
    assert!(failure.to_string().contains("refusal"), "{failure}");
}

// A preview that cannot be made is said, and is not the pull's failure.
#[test]
fn test_a_preview_that_cannot_be_made_is_a_notice() {
    let fx = unpulled();
    let delivery = Delivery::answering(Err(aborted()));
    let outcome = fx.deps.pull_and_refresh(&delivery, false, true, false);
    assert!(outcome
        .notices
        .iter()
        .any(|n| format!("{n:?}").contains("could not preview the project refresh")));
    let pulled = outcome.result.unwrap();
    assert!(pulled.skipped.is_none() && pulled.failure.is_none());
}

// A pull with nothing to install has nothing to refresh.
#[test]
fn test_a_pull_with_no_remote_references_does_not_refresh() {
    let fx = Fixture::new();
    fx.profile("dev", &[]);
    let delivery = Delivery::answering(Ok(writing_hooks()));
    let pulled = fx
        .deps
        .pull_and_refresh(&delivery, false, true, true)
        .result
        .unwrap();
    assert!(delivery.asked.borrow().is_empty());
    assert_eq!(pulled.report.total, 0);
}
