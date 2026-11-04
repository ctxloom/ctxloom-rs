//! `deps pull` over the fakes: first pins only; nothing moved; the evidence
//! rule.

use angzarr_diagnostic::Diagnostic;

use crate::fakes::{demo, Fixture, BUNDLES, ENVELOPE};
use crate::views::PullReport;
use crate::{AppError, Notice, Outcome};

const URL: &str = "file:///srv/origin.git";

fn ok<T>(outcome: Outcome<T>) -> T {
    outcome.result.unwrap_or_else(|e| panic!("{e:?}"))
}

fn err<T: std::fmt::Debug>(outcome: Outcome<T>) -> AppError {
    outcome.result.expect_err("a refusal")
}

fn demo_ref() -> String {
    Fixture::reference(URL, "demo")
}

/// `dev` drawing on origin's demo bundle, not yet pulled.
fn unpulled() -> (Fixture, String) {
    let fx = Fixture::new();
    let sha = fx.origin("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    (fx, sha)
}

fn pulled() -> (Fixture, String) {
    let (fx, sha) = unpulled();
    ok(fx.deps.pull(false, true));
    (fx, sha)
}

fn pull(fx: &Fixture) -> PullReport {
    ok(fx.deps.pull(false, true))
}

// deps.feature: Referencing a remote bundle and pulling it locks the
// dependency closure
#[test]
fn test_a_first_pull_installs_the_bundle_and_locks_it_at_its_commit() {
    let (fx, sha) = unpulled();
    let report = pull(&fx);
    assert_eq!(report.status, "completed");
    assert_eq!(
        (
            report.total,
            report.installed,
            report.reinstalled,
            report.errors
        ),
        (1, 1, 0, 0)
    );
    assert_eq!(report.synced[0].reference, demo_ref());
    assert_eq!(report.synced[0].status, "installed");
    assert_eq!(
        report.message,
        "Synced 1 items: 1 installed, 0 reinstalled, 0 skipped, 0 failed"
    );
    assert!(report.failure().is_none());
    assert_eq!(
        fx.files.lock_text().expect("a lock"),
        format!(
            "schema_version: 3\nbundles:\n  {}:\n    sha: {sha}\n    url: {URL}\n    kind: branch\n",
            demo_ref()
        )
    );
    let tree = fx.files.installed_at(&demo_ref(), &sha).expect("installed");
    assert!(tree.contains_key("bundle.yaml") && tree.contains_key("fragments/demo-frag.md"));
}

// deps.feature: A first pin shows everything the bundle brings in
#[test]
fn test_a_first_pin_is_disclosed_with_everything_it_brings_in() {
    let (fx, sha) = unpulled();
    let report = pull(&fx);
    assert_eq!(report.changes.len(), 1);
    let change = &report.changes[0];
    assert_eq!(
        (
            change.identity.as_str(),
            change.from_sha.as_str(),
            change.to_sha.as_str()
        ),
        (demo_ref().as_str(), "", sha.as_str())
    );
    assert_eq!(change.items.len(), 1);
    assert_eq!(change.items[0].name, "demo-frag");
    assert_eq!(change.files.len(), 2);
}

// deps.feature: A second pull is incremental
#[test]
fn test_a_second_pull_keeps_the_pin_and_writes_nothing() {
    let (fx, sha) = pulled();
    let writes = fx.files.lock_writes();
    let before = fx.files.lock_text();
    let again = pull(&fx);
    assert_eq!((again.installed, again.reinstalled), (0, 0));
    assert_eq!(again.skipped.len(), 1);
    assert_eq!(again.skipped[0].status, "skipped");
    assert!(again.changes.is_empty());
    assert_eq!(
        fx.files.lock_writes(),
        writes,
        "a pull at unchanged pins rewrites nothing"
    );
    assert_eq!(fx.files.lock_text(), before);
    assert!(fx.files.installed_at(&demo_ref(), &sha).is_some());
}

// deps.feature: A skipped pull leaves old content in place
#[test]
fn test_a_pull_never_moves_an_existing_pin_though_upstream_moved() {
    let (fx, sha) = pulled();
    fx.remotes.commit(URL, &demo("1.1.0", "MARKER"));
    let again = pull(&fx);
    assert_eq!(again.skipped.len(), 1);
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
    assert_eq!(fx.files.installed_identities(), vec![(demo_ref(), sha)]);
}

// deps.feature: A held dependency's content survives even a forced pull
#[test]
fn test_a_forced_pull_reinstalls_each_reference_at_its_pin() {
    let (fx, sha) = pulled();
    ok(fx.deps.hold("origin/demo", true));
    fx.remotes.commit(URL, &demo("2.0.0", "version two"));
    let before = fx.files.lock_text();
    let forced = ok(fx.deps.pull(true, true));
    assert_eq!((forced.installed, forced.reinstalled), (0, 1));
    assert_eq!(forced.synced[0].status, "reinstalled");
    assert_eq!(fx.files.lock_text(), before, "held and unmoved");
    assert!(fx.files.lock_text().expect("a lock").contains("held: true"));
    let tree = fx
        .files
        .installed_at(&demo_ref(), &sha)
        .expect("the pinned tree");
    assert!(
        String::from_utf8_lossy(&tree["fragments/demo-frag.md"].data)
            .contains("Demo fragment content.")
    );
}

#[test]
fn test_a_tree_that_is_gone_is_reinstalled_at_its_pin_without_force() {
    let (fx, sha) = pulled();
    fx.files.uninstall(&demo_ref(), &sha);
    let again = pull(&fx);
    assert_eq!(
        (again.installed, again.reinstalled, again.skipped.len()),
        (0, 1, 0)
    );
    assert!(fx.files.installed_at(&demo_ref(), &sha).is_some());
}

// deps.feature: Pulling straight from an unregistered address is refused
#[test]
fn test_an_unregistered_address_is_refused_and_nothing_is_fetched_or_pinned() {
    let fx = Fixture::new();
    fx.remotes.serve("file:///srv/stranger.git");
    fx.remotes
        .commit("file:///srv/stranger.git", &demo("1.0.0", "x"));
    fx.profile(
        "dev",
        &[&Fixture::reference("file:///srv/stranger.git", "demo")],
    );
    let refused = err(fx.deps.pull(false, true));
    match &refused {
        AppError::NotRegistered { url } => assert_eq!(url, "file:///srv/stranger.git"),
        other => panic!("{other:?}"),
    }
    assert!(fx.files.lock_text().is_none());
    assert!(fx.files.installed_identities().is_empty());
    assert!(
        fx.remotes.calls().is_empty(),
        "never fetched: {:?}",
        fx.remotes.calls()
    );
}

#[test]
fn test_every_unregistered_repository_is_named() {
    let fx = Fixture::new();
    fx.profile(
        "dev",
        &[
            &Fixture::reference("file:///srv/b.git", "x"),
            &Fixture::reference("file:///srv/a.git", "x"),
        ],
    );
    let outcome = fx.deps.pull(false, true);
    assert!(
        matches!(&outcome.result, Err(AppError::NotRegistered { url }) if url == "file:///srv/a.git")
    );
    let said: Vec<&Notice> = outcome.notices.iter().collect();
    assert_eq!(said.len(), 1);
    assert!(matches!(said[0], Notice::Text { text, .. } if text.contains("file:///srv/b.git")));
}

#[test]
fn test_the_same_item_at_two_commits_is_a_conflict_and_pins_nothing() {
    let (fx, sha) = unpulled();
    let second = fx.remotes.commit(URL, &demo("1.1.0", "x"));
    let (a, b) = (
        format!("{}@{sha}", demo_ref()),
        format!("{}@{second}", demo_ref()),
    );
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            ("profiles/one.yaml", &format!("bundles:\n  - {a}\n")),
            ("profiles/two.yaml", &format!("bundles:\n  - {b}\n")),
        ],
    );
    match err(fx.deps.pull(false, true)) {
        AppError::Conflict { text } => {
            assert!(text.contains(&demo_ref()), "{text}");
            assert!(
                text.contains(&format!("{}, {}", &sha[..7], &second[..7])),
                "{text}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(fx.files.lock_text().is_none());
}

// deps.feature: A bundle deleted upstream is removed locally, by name
#[test]
fn test_a_bundle_upstream_stopped_publishing_is_removed_by_name() {
    let (fx, sha) = pulled();
    fx.remotes.delete(
        URL,
        &[
            &format!("{BUNDLES}/demo/bundle.yaml"),
            &format!("{BUNDLES}/demo/fragments/demo-frag.md"),
        ],
    );
    let again = pull(&fx);
    let reconcile = again.reconcile.expect("a reconcile ran");
    assert_eq!(reconcile.gone, vec![demo_ref()]);
    assert!(reconcile.unreachable.is_empty());
    assert!(!fx
        .files
        .lock_text()
        .expect("a lock")
        .contains("//bundles/demo"));
    assert!(
        fx.files.installed_at(&demo_ref(), &sha).is_none(),
        "its tree went with it"
    );
}

// deps.feature: An unreachable remote removes nothing and says it could not
// check
#[test]
fn test_an_unreachable_remote_removes_nothing_and_is_reported_unchecked() {
    let (fx, sha) = pulled();
    fx.remotes
        .delete(URL, &[&format!("{BUNDLES}/demo/bundle.yaml")]);
    fx.remotes.set_unreachable(URL, true);
    let outcome = fx.deps.pull(false, true);
    assert!(outcome.notices.iter().any(
        |n| matches!(n, Notice::Text { text, .. } if text.starts_with(&format!("fetch {URL}: ")))
    ));
    let again = ok(outcome);
    let reconcile = again.reconcile.expect("a reconcile ran");
    assert!(reconcile.gone.is_empty());
    assert_eq!(reconcile.unreachable.len(), 1);
    assert_eq!(reconcile.unreachable[0].url, URL);
    assert_eq!(reconcile.unreachable[0].refs, vec![demo_ref()]);
    assert!(
        reconcile.unreachable[0].reason.contains("could not read"),
        "{:?}",
        reconcile.unreachable
    );
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
    assert!(fx.files.installed_at(&demo_ref(), &sha).is_some());
    // Fetched once, though both the walk and the reconcile asked.
    let refreshes = fx
        .remotes
        .calls()
        .iter()
        .filter(|c| *c == &format!("refresh {URL}"))
        .count();
    assert_eq!(refreshes, 2, "one for the first pull, one for this one");
}

// deps.feature: A pull whose lock could not reach part of the closure names
// what it kept
#[test]
fn test_a_closure_part_that_cannot_be_reached_keeps_its_previous_entries_and_is_named() {
    let (fx, sha) = pulled();
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
                &format!("parents:\n  - {kit}#profiles/parent\n"),
            ),
        ],
    );
    let again = pull(&fx);
    assert!(again.incomplete);
    assert_eq!(again.unreachable, vec![kit]);
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
}

#[test]
fn test_an_incomplete_closure_keeps_the_entries_it_could_not_walk() {
    let (fx, sha) = pulled();
    fx.register("gone", "file:///nonexistent/gone");
    let kit = Fixture::reference("file:///nonexistent/gone", "kit");
    // The profile that reached demo is gone; the only thing left is
    // unreachable. Nothing it owned is erased.
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            (
                "profiles/orphan.yaml",
                &format!("parents:\n  - {kit}#profiles/parent\n"),
            ),
        ],
    );
    let outcome = fx.deps.pull(false, true);
    assert!(outcome.notices.iter().any(|n| matches!(n, Notice::Text { text, .. } if text.contains("preserving 1 existing lockfile entry"))), "{:?}", outcome.notices);
    let again = ok(outcome);
    assert!(again.incomplete);
    assert!(again.removed.is_empty());
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
}

// deps.feature: A pull never moves an existing pin, even when its constraint
// changed
#[test]
fn test_a_changed_constraint_is_named_and_the_pin_stays() {
    let (fx, sha) = pulled();
    fx.remotes.commit(URL, &demo("2.0.0", "two"));
    fx.profile("dev", &[&format!("{}@main", demo_ref())]);
    let again = ok(fx.deps.pull(true, true));
    assert_eq!(again.constraint_changes.len(), 1);
    let change = &again.constraint_changes[0];
    assert_eq!(
        (
            change.identity.as_str(),
            change.pinned.as_str(),
            change.declared.as_str(),
            change.sha.as_str()
        ),
        (demo_ref().as_str(), "", "main", sha.as_str())
    );
    assert!(again.changes.is_empty(), "no new pin");
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
}

#[test]
fn test_a_dependency_no_profile_names_any_more_is_dropped_and_its_tree_pruned() {
    let (fx, _) = pulled();
    fx.origin("other", "file:///srv/other.git");
    fx.profile(
        "dev",
        &[
            &demo_ref(),
            &Fixture::reference("file:///srv/other.git", "demo"),
        ],
    );
    let both = pull(&fx);
    assert_eq!(both.installed, 1);
    fx.profile("dev", &[&demo_ref()]);
    let after = pull(&fx);
    assert_eq!(
        after.removed,
        vec![Fixture::reference("file:///srv/other.git", "demo")]
    );
    assert_eq!(
        after.pruned_checkouts,
        vec![Fixture::reference("file:///srv/other.git", "demo")]
    );
    assert!(!fx.files.lock_text().expect("a lock").contains("other.git"));
}

#[test]
fn test_a_bundle_that_cannot_be_installed_fails_the_pull_and_is_not_locked() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    fx.remotes
        .commit(URL, &[(&format!("{BUNDLES}/demo/fragments/a.md"), "a")]);
    fx.register("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    let report = pull(&fx);
    assert_eq!((report.status, report.errors), ("completed_with_errors", 1));
    assert_eq!(report.failed[0].reference, demo_ref());
    assert!(
        report.failed[0].error.contains("not a bundle"),
        "{}",
        report.failed[0].error
    );
    assert!(report.failed[0].fix.contains("ctxloom deps pull"));
    match report.failure() {
        Some(AppError::PullFailed { count, refs }) => {
            assert_eq!((count, refs.as_str()), (1, demo_ref().as_str()))
        }
        other => panic!("{other:?}"),
    }
    assert!(fx.files.lock_text().is_none());
}

#[test]
fn test_an_install_that_fails_is_reported_per_item() {
    let (fx, _) = unpulled();
    fx.files.fail_installs(true);
    let report = pull(&fx);
    assert_eq!(report.errors, 1);
    assert_eq!(report.failed[0].error, "disk full");
}

#[test]
fn test_a_pull_with_no_remote_reference_has_nothing_to_do() {
    let fx = Fixture::new();
    fx.profile("dev", &["local-bundle"]);
    let report = pull(&fx);
    assert_eq!(report.status, "empty");
    assert_eq!(report.total, 0);
    assert_eq!(report.message, "No remote references found in profiles");
    assert!(fx.files.lock_text().is_none());
    assert!(report.reconcile.is_none());
}

#[test]
fn test_a_pull_without_the_lock_installs_and_leaves_the_lockfile_alone() {
    let (fx, sha) = unpulled();
    let report = ok(fx.deps.pull(false, false));
    assert_eq!(report.installed, 1);
    assert!(fx.files.lock_text().is_none());
    assert!(report.reconcile.is_none());
    assert!(fx.files.installed_at(&demo_ref(), &sha).is_some());
}

// lockfile_guard_test.go: a lock that cannot be read is refused, never
// overwritten
#[test]
fn test_a_corrupt_lock_is_refused_and_left_as_it_is() {
    let (fx, _) = unpulled();
    fx.files
        .set_lock("schema_version: 3\nbundles: [unterminated\n");
    let refused = err(fx.deps.pull(false, true));
    assert_eq!(refused.code(), "deps-unreadable");
    assert_eq!(
        fx.files.lock_text().as_deref(),
        Some("schema_version: 3\nbundles: [unterminated\n")
    );
    assert!(fx.files.installed_identities().is_empty());
}

// depgraph.go: a remote bundle's profile is read out of it and walked
#[test]
fn test_a_remote_profile_parent_pulls_its_siblings_at_its_own_commit() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    let mut files: Vec<(String, String)> = demo("1.0.0", "Demo fragment content.")
        .iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect();
    files.push((
        format!("{BUNDLES}/kit/bundle.yaml"),
        "version: \"1.0.0\"\n".to_string(),
    ));
    files.push((
        format!("{BUNDLES}/kit/profiles/p.yaml"),
        "bundles:\n  - demo\n".to_string(),
    ));
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    let sha = fx.remotes.commit(URL, &refs);
    fx.register("origin", URL);
    let kit = Fixture::reference(URL, "kit");
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            (
                "profiles/dev.yaml",
                &format!("parents:\n  - {kit}#profiles/p\n"),
            ),
        ],
    );
    let report = pull(&fx);
    assert_eq!((report.total, report.installed), (2, 2));
    let lock = fx.files.lock_text().expect("a lock");
    assert!(lock.contains(&kit) && lock.contains(&demo_ref()), "{lock}");
    assert_eq!(lock.matches(&format!("sha: {sha}")).count(), 2);
}

// profiles.go CheckOwnRepo: a remote profile may name only its own repository
#[test]
fn test_a_remote_profile_reaching_another_repository_is_not_followed() {
    let (fx, _) = unpulled();
    fx.remotes.serve("file:///srv/vendor.git");
    let reach = format!("bundles:\n  - {}\n", demo_ref());
    fx.remotes.commit(
        "file:///srv/vendor.git",
        &[
            (
                &format!("{BUNDLES}/kit/bundle.yaml"),
                "version: \"1.0.0\"\n",
            ),
            (&format!("{BUNDLES}/kit/profiles/reach.yaml"), &reach),
        ],
    );
    fx.register("vendor", "file:///srv/vendor.git");
    let kit = Fixture::reference("file:///srv/vendor.git", "kit");
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            (
                "profiles/dev.yaml",
                &format!("parents:\n  - {kit}#profiles/reach\n"),
            ),
        ],
    );
    let outcome = fx.deps.pull(false, true);
    assert!(
        outcome
            .notices
            .iter()
            .any(|n| matches!(n, Notice::Unexpanded { .. })),
        "{:?}",
        outcome.notices
    );
    let report = ok(outcome);
    assert!(report.incomplete, "{report:?}");
    let lock = fx.files.lock_text().expect("a lock");
    assert!(lock.contains(&kit), "the kit itself is pinned");
    assert!(!lock.contains(&demo_ref()), "what it reached for is not");
}

#[test]
fn test_a_bare_name_in_a_local_profile_is_the_projects_own() {
    let fx = Fixture::new();
    fx.profile("dev", &["demo"]);
    assert_eq!(pull(&fx).status, "empty");
}

// A short reference through a registered remote's name is the same bundle.
#[test]
fn test_a_short_reference_pulls_the_bundle_of_that_remote() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    fx.profile("dev", &["origin/demo"]);
    let report = pull(&fx);
    assert_eq!(report.installed, 1);
    assert!(fx.files.lock_text().expect("a lock").contains(&demo_ref()));
}

// constraint_resolve.go: a bare commit is recorded as it is, with no clone
#[test]
fn test_a_commit_constraint_is_pinned_as_written() {
    let (fx, sha) = unpulled();
    let later = fx.remotes.commit(URL, &demo("2.0.0", "two"));
    fx.profile("dev", &[&format!("{}@{sha}", demo_ref())]);
    let report = pull(&fx);
    assert_eq!(report.installed, 1);
    let lock = fx.files.lock_text().expect("a lock");
    assert!(
        lock.contains(&format!("sha: {sha}")) && !lock.contains(&later),
        "{lock}"
    );
    assert!(
        lock.contains("kind: sha") && lock.contains(&format!("requested_version: {sha}")),
        "{lock}"
    );
}

// constraint_resolve.go: a constraint that cannot be resolved is skipped, not
// pinned empty; the lock keeps the last commit
#[test]
fn test_an_unresolvable_constraint_is_skipped_with_the_reason() {
    let (fx, _) = unpulled();
    fx.profile("dev", &[&format!("{}@no-such-branch", demo_ref())]);
    let outcome = fx.deps.pull(false, true);
    assert!(outcome.notices.iter().any(|n| matches!(n, Notice::Text { text, .. } if text.starts_with("could not resolve") && text.contains("no-such-branch"))), "{:?}", outcome.notices);
    let report = ok(outcome);
    assert!(report.incomplete);
    assert!(fx.files.lock_text().is_none());
}

// A version constraint is resolved against the repository's tags.
#[test]
fn test_a_version_constraint_pins_the_tag_that_is_that_version() {
    let (fx, sha) = unpulled();
    fx.remotes.tag(URL, "v1.0.0", &sha);
    fx.remotes.commit(URL, &demo("2.0.0", "two"));
    fx.profile("dev", &[&format!("{}@1.0.0", demo_ref())]);
    assert_eq!(pull(&fx).installed, 1);
    let lock = fx.files.lock_text().expect("a lock");
    assert!(lock.contains(&format!("sha: {sha}")), "{lock}");
    assert!(
        lock.contains("kind: version") && lock.contains("version: v1.0.0"),
        "{lock}"
    );
}

// A parent profile the project does not have leaves the closure incomplete.
#[test]
fn test_a_local_parent_that_is_not_there_makes_the_closure_incomplete() {
    let (fx, sha) = pulled();
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            (
                "profiles/dev.yaml",
                &format!("bundles:\n  - {}\nparents:\n  - ghost\n", demo_ref()),
            ),
            (
                "profiles/other.yaml",
                "parents:\n  - bad name with spaces\n",
            ),
        ],
    );
    let report = pull(&fx);
    assert!(report.incomplete);
    assert_eq!(report.unreachable.len(), 2, "{:?}", report.unreachable);
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
}

// Nothing is said of preserving entries when there were none to preserve.
#[test]
fn test_an_incomplete_closure_with_nothing_locked_says_nothing_of_preserving() {
    let fx = Fixture::new();
    fx.content.bundle(
        "project",
        &[ENVELOPE, ("profiles/dev.yaml", "parents:\n  - ghost\n")],
    );
    let outcome = fx.deps.pull(false, true);
    assert!(
        !outcome
            .notices
            .iter()
            .any(|n| matches!(n, Notice::Text { text, .. } if text.contains("preserving"))),
        "{:?}",
        outcome.notices
    );
}

// A pull that failed to install something prunes nothing.
#[test]
fn test_a_pull_that_failed_an_install_prunes_no_tree() {
    let (fx, sha) = pulled();
    fx.origin("other", "file:///srv/other.git");
    let other = Fixture::reference("file:///srv/other.git", "demo");
    fx.profile("dev", &[&demo_ref(), &other]);
    pull(&fx);
    // The project stops naming `other`, and a new bundle fails to install.
    let third = Fixture::reference("file:///srv/third.git", "demo");
    fx.origin("third", "file:///srv/third.git");
    fx.profile("dev", &[&demo_ref(), &third]);
    fx.files.fail_installs(true);
    let report = pull(&fx);
    assert_eq!(report.errors, 1);
    assert!(
        fx.files
            .installed_identities()
            .iter()
            .any(|(id, _)| *id == other),
        "{:?}",
        fx.files.installed_identities()
    );
    assert!(fx.files.installed_at(&demo_ref(), &sha).is_some());
}

// A local parent profile that is there leaves the closure complete.
#[test]
fn test_a_local_parent_that_is_there_leaves_the_closure_complete() {
    let (fx, _) = pulled();
    fx.content.bundle(
        "project",
        &[
            ENVELOPE,
            (
                "profiles/dev.yaml",
                &format!("bundles:\n  - {}\nparents:\n  - base\n", demo_ref()),
            ),
            ("profiles/base.yaml", "description: base\n"),
        ],
    );
    let report = pull(&fx);
    assert!(!report.incomplete, "{:?}", report.unreachable);
    assert!(report.unreachable.is_empty());
}
