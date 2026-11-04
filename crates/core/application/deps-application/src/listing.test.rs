//! `deps list`, `deps hold` and `deps unhold` over the fakes.

use angzarr_diagnostic::Diagnostic;

use crate::fakes::{demo, Fixture};
use crate::{AppError, Notice, Outcome};

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

// deps.feature: A project with nothing installed reports an empty closure
#[test]
fn test_nothing_installed_lists_empty() {
    let fx = Fixture::new();
    let listing = ok(fx.deps.list());
    assert_eq!((listing.count, listing.deps.len()), (0, 0));
}

// deps.feature: The listing names each dependency, its commit and its origin
#[test]
fn test_the_listing_names_each_dependency_its_commit_and_its_origin() {
    let (fx, sha) = pulled();
    let listing = ok(fx.deps.list());
    assert_eq!(listing.count, 1);
    let row = &listing.deps[0];
    assert_eq!(row.name, "demo");
    assert_eq!(row.reference, demo_ref());
    assert_eq!(
        (row.sha.as_str(), row.origin.as_str(), row.url.as_str()),
        (sha.as_str(), "origin", URL)
    );
    assert_eq!((row.constraint.as_str(), row.held), ("", false));
}

#[test]
fn test_rows_are_sorted_by_name_then_reference_and_carry_the_constraint() {
    let (fx, sha) = pulled();
    fx.files.set_lock(&format!(
        "schema_version: 3\nbundles:\n  {b2}:\n    sha: {sha}\n    url: {URL}\n  {a}:\n    sha: {sha}\n    url: {URL}\n    requested_version: main\n  {b1}:\n    sha: {sha}\n    url: file:///srv/other.git\n",
        a = Fixture::reference(URL, "a"),
        b1 = Fixture::reference("file:///srv/other.git", "b"),
        b2 = Fixture::reference(URL, "b"),
    ));
    let rows = ok(fx.deps.list()).deps;
    let order: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r.name.as_str(), r.url.as_str()))
        .collect();
    assert_eq!(
        order,
        vec![("a", URL), ("b", URL), ("b", "file:///srv/other.git")]
    );
    assert_eq!(rows[0].constraint, "main");
    assert_eq!(rows[2].origin, "", "no remote is registered for it");
}

// deps.feature: The listing is offline and answers when nothing else can
#[test]
fn test_the_listing_needs_no_registry_and_no_remote() {
    let (fx, _) = pulled();
    fx.files.set_registry("not: [yaml");
    fx.remotes.set_unreachable(URL, true);
    let before = fx.remotes.calls().len();
    let rows = ok(fx.deps.list()).deps;
    assert_eq!(rows[0].origin, "");
    assert_eq!(fx.remotes.calls().len(), before);
}

#[test]
fn test_a_corrupt_lock_is_refused_by_the_listing() {
    let (fx, _) = pulled();
    fx.files.set_lock("not: [yaml");
    assert_eq!(
        fx.deps.list().result.expect_err("refused").code(),
        "deps-unreadable"
    );
}

// deps.feature: A held dependency is marked held in the listing
#[test]
fn test_a_hold_is_recorded_once_by_short_or_canonical_name_and_released() {
    let (fx, sha) = pulled();
    let writes = fx.files.lock_writes();
    let held = ok(fx.deps.hold("origin/demo", true));
    assert_eq!((held.status, held.name.as_str()), ("held", "origin/demo"));
    assert!(ok(fx.deps.list()).deps[0].held);
    assert_eq!(fx.files.lock_writes(), writes + 1);
    ok(fx.deps.hold(&demo_ref(), true));
    assert_eq!(
        fx.files.lock_writes(),
        writes + 1,
        "a hold already held writes nothing"
    );
    let released = ok(fx.deps.hold("origin/demo", false));
    assert_eq!(released.status, "released");
    assert!(!ok(fx.deps.list()).deps[0].held);
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
    assert!(!fx.files.lock_text().expect("a lock").contains("held"));
}

// deps.feature (bundle.feature): Holding a local bundle reports it is not
// lockfile-tracked
#[test]
fn test_holding_a_bundle_of_this_project_holds_nothing_and_says_so() {
    let (fx, _) = pulled();
    fx.files.author("mine", &[("bundle.yaml", "version: 1\n")]);
    for held in [true, false] {
        let outcome = fx.deps.hold("mine", held);
        let notices = outcome.notices.clone();
        let result = ok(outcome);
        assert_eq!(
            (result.status, result.name.as_str()),
            ("not_tracked", "mine")
        );
        let verb = if held { "hold" } else { "unhold" };
        assert!(
            matches!(&notices[..], [Notice::Text { text, .. }] if text.contains(&format!("nothing to {verb}"))),
            "{notices:?}"
        );
    }
}

#[test]
fn test_a_name_that_is_nothing_is_refused_never_reported_held() {
    let (fx, _) = pulled();
    match fx.deps.hold("typo", true).result.expect_err("refused") {
        AppError::NothingToHold { name, verb } => {
            assert_eq!((name.as_str(), verb.as_str()), ("typo", "hold"))
        }
        other => panic!("{other:?}"),
    }
    match fx.deps.hold("typo", false).result.expect_err("refused") {
        AppError::NothingToHold { verb, .. } => assert_eq!(verb, "unhold"),
        other => panic!("{other:?}"),
    }
}

// lockfile_guard_test.go: a hold never overwrites a lock it cannot read
#[test]
fn test_a_hold_over_a_corrupt_lock_is_refused_and_the_file_stands() {
    let (fx, _) = pulled();
    fx.files.set_lock("not: [yaml");
    assert!(fx.deps.hold("origin/demo", true).result.is_err());
    assert_eq!(fx.files.lock_text().as_deref(), Some("not: [yaml"));
}

#[test]
fn test_a_lock_entry_of_another_remote_is_held_by_its_own_name() {
    let (fx, _) = pulled();
    fx.origin("other", "file:///srv/other.git");
    fx.profile(
        "dev",
        &[
            &demo_ref(),
            &Fixture::reference("file:///srv/other.git", "demo"),
        ],
    );
    ok(fx.deps.pull(false, true));
    ok(fx.deps.hold("other/demo", true));
    let rows = ok(fx.deps.list()).deps;
    let held: Vec<(&str, bool)> = rows.iter().map(|r| (r.origin.as_str(), r.held)).collect();
    assert_eq!(held, vec![("origin", false), ("other", true)]);
    let _ = demo("1.0.0", "x");
}
