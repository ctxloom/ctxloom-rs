//! `deps check` over the fakes: it reads, it never calls an unchecked entry
//! current.

use angzarr_diagnostic::Diagnostic;

use crate::fakes::{demo, Fixture, ENVELOPE};
use crate::{AppError, Outcome};

const URL: &str = "file:///srv/origin.git";

fn ok<T>(outcome: Outcome<T>) -> T {
    outcome.result.unwrap_or_else(|e| panic!("{e:?}"))
}

fn demo_ref() -> String {
    Fixture::reference(URL, "demo")
}

fn pulled() -> (Fixture, String) {
    let fx = Fixture::new();
    let sha = fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    ok(fx.deps.pull(false, true));
    (fx, sha)
}

// deps.feature: Check on an empty closure has nothing to check
#[test]
fn test_an_empty_closure_has_nothing_to_check_and_asks_no_remote() {
    let fx = Fixture::new();
    let report = ok(fx.deps.check());
    assert_eq!(report.entries, 0);
    assert!(report.updates.is_empty() && report.unchecked.is_empty());
    assert!(fx.remotes.calls().is_empty());
}

// deps.feature: Check reports an available advance and changes nothing
#[test]
fn test_an_advance_is_reported_and_nothing_is_written() {
    let (fx, sha) = pulled();
    let later = fx.remotes.commit(URL, &demo("1.1.0", "two"));
    let (lock, writes) = (fx.files.lock_text(), fx.files.lock_writes());
    let report = ok(fx.deps.check());
    assert_eq!(report.entries, 1);
    assert_eq!(report.updates.len(), 1);
    let update = &report.updates[0];
    assert_eq!(update.reference, demo_ref());
    assert_eq!(
        (update.kind, update.selector.as_str()),
        ("bundle", "branch")
    );
    assert_eq!(
        (update.current_sha.as_str(), update.latest_sha.as_str()),
        (sha.as_str(), later.as_str())
    );
    assert_eq!(
        (update.requested.as_str(), update.resolved.as_str()),
        ("", "")
    );
    assert!(report.unchecked.is_empty() && report.refresh_failures.is_empty());
    assert_eq!(fx.files.lock_text(), lock);
    assert_eq!(fx.files.lock_writes(), writes);
}

#[test]
fn test_a_current_entry_is_not_an_update() {
    let (fx, _) = pulled();
    let report = ok(fx.deps.check());
    assert_eq!(report.entries, 1);
    assert!(report.updates.is_empty() && report.unchecked.is_empty());
}

// version_constraint.go IsPin: a commit or an exact tag is never outdated
#[test]
fn test_a_pinned_entry_is_never_reported_outdated() {
    let fx = Fixture::new();
    let sha = fx.origin("origin", URL);
    fx.remotes.tag(URL, "v1.0.0", &sha);
    fx.profile("dev", &[&format!("{}@{sha}", demo_ref())]);
    ok(fx.deps.pull(false, true));
    fx.remotes.commit(URL, &demo("2.0.0", "two"));
    let report = ok(fx.deps.check());
    assert_eq!(report.entries, 1);
    assert!(report.updates.is_empty() && report.unchecked.is_empty());
}

// deps.feature: A check that could not reach the remote does not call it up to
// date
#[test]
fn test_an_unreachable_remote_leaves_its_entries_unchecked() {
    let (fx, _) = pulled();
    fx.remotes.commit(URL, &demo("1.1.0", "two"));
    fx.remotes.set_unreachable(URL, true);
    let report = ok(fx.deps.check());
    assert!(
        report.updates.is_empty(),
        "the stale clone is not read: {report:?}"
    );
    assert_eq!(report.refresh_failures.len(), 1);
    assert_eq!(report.refresh_failures[0].url, URL);
    assert!(report.refresh_failures[0].error.contains("could not read"));
    assert_eq!(report.unchecked.len(), 1);
    let row = &report.unchecked[0];
    assert_eq!(
        (row.reference.as_str(), row.url.as_str(), row.reason),
        (demo_ref().as_str(), URL, "not_refreshed")
    );
    assert!(row.error.contains("could not read"));
}

#[test]
fn test_an_entry_whose_constraint_cannot_be_resolved_is_unchecked_with_the_reason() {
    let (fx, sha) = pulled();
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {}:\n    sha: {sha}\n    url: {URL}\n    requested_version: no-such-branch\n    kind: branch\n",
        demo_ref()
    ));
    let report = ok(fx.deps.check());
    assert_eq!(report.unchecked.len(), 1);
    let row = &report.unchecked[0];
    assert_eq!(
        (row.reason, row.constraint.as_str()),
        ("unresolvable", "no-such-branch")
    );
    assert!(row.error.contains("no-such-branch"), "{}", row.error);
}

#[test]
fn test_a_range_that_cannot_be_matched_is_unchecked_not_current() {
    let (fx, sha) = pulled();
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {}:\n    sha: {sha}\n    url: {URL}\n    requested_version: ^1.0\n    version: v1.0.0\n    kind: version\n",
        demo_ref()
    ));
    let report = ok(fx.deps.check());
    assert!(report.updates.is_empty());
    assert_eq!(report.unchecked[0].reason, "unresolvable");
    assert!(report.unchecked[0].error.contains("^1.0"));
}

#[test]
fn test_an_entry_with_no_commit_is_counted_not_checked() {
    let (fx, _) = pulled();
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {}:\n    sha: \"\"\n    url: {URL}\n",
        demo_ref()
    ));
    fx.remotes.commit(URL, &demo("1.1.0", "two"));
    let before = fx.remotes.calls().len();
    let report = ok(fx.deps.check());
    assert_eq!((report.entries, report.skipped_empty), (1, 1));
    assert!(report.updates.is_empty() && report.unchecked.is_empty());
    assert_eq!(
        fx.remotes.calls().len(),
        before,
        "nothing to compare, nothing fetched"
    );
}

#[test]
fn test_a_repository_no_remote_is_registered_for_is_not_fetched() {
    let (fx, sha) = pulled();
    let stranger = Fixture::reference("file:///srv/stranger.git", "x");
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {stranger}:\n    sha: {sha}\n    url: file:///srv/stranger.git\n  {}:\n    sha: {sha}\n    url: {URL}\n",
        demo_ref()
    ));
    fx.remotes.serve("file:///srv/stranger.git");
    fx.profile("dev", &[&demo_ref(), &stranger]);
    let report = ok(fx.deps.check());
    assert_eq!(report.refresh_failures.len(), 1);
    assert!(report.refresh_failures[0]
        .error
        .contains("remote not registered"));
    assert!(!fx
        .remotes
        .calls()
        .contains(&"refresh file:///srv/stranger.git".to_string()));
    assert_eq!(report.unchecked.len(), 1);
}

// deps_check.go closureLock: check never offers an update an upgrade would
// remove
#[test]
fn test_only_the_entries_the_closure_reaches_are_checked() {
    let (fx, sha) = pulled();
    let stale = Fixture::reference(URL, "stale");
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {stale}:\n    sha: {sha}\n    url: {URL}\n  {}:\n    sha: {sha}\n    url: {URL}\n",
        demo_ref()
    ));
    assert_eq!(ok(fx.deps.check()).entries, 1);
    // A closure that could not be walked leaves the whole lock to check.
    fx.register("gone", "file:///nonexistent/gone");
    let kit = Fixture::reference("file:///nonexistent/gone", "kit");
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            (
                "profiles/dev.yaml",
                &format!("bundles:\n  - {}\n", demo_ref()),
            ),
            (
                "profiles/orphan.yaml",
                &format!("parents:\n  - {kit}#profiles/p\n"),
            ),
        ],
    );
    assert_eq!(ok(fx.deps.check()).entries, 2);
}

#[test]
fn test_a_corrupt_lock_is_refused() {
    let (fx, _) = pulled();
    fx.files.set_lock("not: [yaml");
    assert_eq!(
        fx.deps.check().result.expect_err("refused").code(),
        "deps-unreadable"
    );
}

// deps_check.go checkSingleDependency
#[test]
fn test_one_reference_is_checked_against_its_lock_entry() {
    let (fx, sha) = pulled();
    let current = ok(fx.deps.check_reference("origin/demo"));
    assert_eq!(current.reference, "origin/demo");
    assert!(current.in_lockfile && current.up_to_date);
    assert_eq!(
        (current.current_sha.as_str(), current.latest_sha.as_str()),
        (sha.as_str(), sha.as_str())
    );
    let later = fx.remotes.commit(URL, &demo("1.1.0", "two"));
    let behind = ok(fx.deps.check_reference(&demo_ref()));
    assert!(behind.in_lockfile && !behind.up_to_date);
    assert_eq!(behind.latest_sha, later);
}

#[test]
fn test_a_reference_that_is_not_locked_is_never_up_to_date() {
    let (fx, _) = pulled();
    let other = Fixture::reference(URL, "other");
    let checked = ok(fx.deps.check_reference(&other));
    assert!(!checked.in_lockfile && !checked.up_to_date);
    assert_eq!(checked.current_sha, "");
}

#[test]
fn test_a_reference_that_cannot_be_checked_is_refused_not_answered_from_a_stale_clone() {
    let (fx, _) = pulled();
    assert!(matches!(
        fx.deps.check_reference("demo").result.expect_err("local"),
        AppError::NotRemoteReference { .. }
    ));
    assert!(matches!(
        fx.deps
            .check_reference(&Fixture::reference("file:///srv/stranger.git", "x"))
            .result
            .expect_err("unregistered"),
        AppError::NotRegistered { .. }
    ));
    fx.remotes.set_unreachable(URL, true);
    match fx
        .deps
        .check_reference(&demo_ref())
        .result
        .expect_err("unreachable")
    {
        AppError::CloneNotRefreshed { url, detail } => {
            assert_eq!(url, URL);
            assert!(detail.contains("could not read"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_a_reference_whose_constraint_cannot_be_resolved_says_so() {
    let (fx, sha) = pulled();
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {}:\n    sha: {sha}\n    url: {URL}\n    requested_version: nope\n",
        demo_ref()
    ));
    assert!(matches!(
        fx.deps
            .check_reference(&demo_ref())
            .result
            .expect_err("unresolvable"),
        AppError::Unreadable { .. }
    ));
}
