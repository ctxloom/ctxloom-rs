//! `deps upgrade` over the fakes: the one command that moves a pin, and shows
//! the move first.

use angzarr_diagnostic::Diagnostic;

use crate::fakes::{demo, Fixture, BUNDLES};
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

fn mcp(version: &str, command: &str, secret: &str) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = demo(version, "Demo fragment content.")
        .iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect();
    files.push((
        format!("{BUNDLES}/demo/mcp/srv.yaml"),
        format!("command: {command}\nenv:\n  API_KEY: secret-{secret}\n"),
    ));
    files
}

fn commit(fx: &Fixture, files: &[(String, String)]) -> String {
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    fx.remotes.commit(URL, &refs)
}

// deps.feature: An upgrade without --yes shows the move and writes nothing
#[test]
fn test_a_preview_shows_the_move_and_writes_nothing() {
    let (fx, sha) = pulled();
    let later = commit(&fx, &mcp("1.1.0", "fixture-mcp-one", "ONE"));
    let (lock, writes) = (fx.files.lock_text(), fx.files.lock_writes());
    let report = ok(fx.deps.upgrade(false));
    assert!(!report.applied && !report.incomplete && !report.nothing_declared);
    assert_eq!(report.changes.len(), 1);
    let change = &report.changes[0];
    assert_eq!(
        (change.from_sha.as_str(), change.to_sha.as_str()),
        (sha.as_str(), later.as_str())
    );
    assert!(change
        .items
        .iter()
        .any(|i| i.kind == "mcp" && i.name == "srv"));
    assert_eq!(fx.files.lock_text(), lock);
    assert_eq!(fx.files.lock_writes(), writes);
    assert!(
        fx.files.installed_at(&demo_ref(), &sha).is_some(),
        "the installed tree stays"
    );
}

// deps.feature: An upgrade with --yes applies the move
#[test]
fn test_an_applied_upgrade_moves_the_pin_and_its_tree() {
    let (fx, sha) = pulled();
    let later = commit(&fx, &mcp("1.1.0", "fixture-mcp-one", "ONE"));
    let report = ok(fx.deps.upgrade(true));
    assert!(report.applied);
    assert_eq!(report.changes.len(), 1);
    assert!(fx.files.lock_text().expect("a lock").contains(&later));
    assert!(fx.files.installed_at(&demo_ref(), &later).is_some());
    assert!(
        fx.files.installed_at(&demo_ref(), &sha).is_none(),
        "the old commit's tree went"
    );
    let again = ok(fx.deps.upgrade(false));
    assert!(again.changes.is_empty() && !again.incomplete && !again.nothing_declared);
}

// deps.feature: An upgrade shows what an MCP server and a script run before and
// after, and never a secret
#[test]
fn test_a_second_move_shows_before_and_after_with_fingerprinted_values() {
    let (fx, _) = pulled();
    commit(&fx, &mcp("1.1.0", "fixture-mcp-one", "ONE"));
    ok(fx.deps.upgrade(true));
    commit(&fx, &mcp("1.2.0", "fixture-mcp-two", "TWO"));
    let report = ok(fx.deps.upgrade(false));
    let item = report.changes[0]
        .items
        .iter()
        .find(|i| i.name == "srv")
        .expect("the server");
    let exec = item.exec.as_ref().expect("what it runs");
    assert_eq!(
        exec.before.as_ref().expect("before").command,
        "fixture-mcp-one"
    );
    assert_eq!(
        exec.after.as_ref().expect("after").command,
        "fixture-mcp-two"
    );
    let json = serde_json::to_string(&report).expect("json");
    assert!(
        !json.contains("secret-ONE") && !json.contains("secret-TWO"),
        "{json}"
    );
}

// deps.feature: A held dependency is not upgraded
#[test]
fn test_a_held_dependency_is_frozen() {
    let (fx, sha) = pulled();
    ok(fx.deps.hold("origin/demo", true));
    commit(&fx, &mcp("1.1.0", "fixture-mcp-one", "ONE"));
    let report = ok(fx.deps.upgrade(true));
    assert!(report.changes.is_empty());
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
}

// deps.feature: An upgrade names each dependency it drops from the lock
#[test]
fn test_a_dependency_no_profile_names_any_more_is_named_and_dropped() {
    let (fx, _) = pulled();
    fx.origin("other", "file:///srv/other.git");
    let other = Fixture::reference("file:///srv/other.git", "demo");
    fx.profile("dev", &[&demo_ref(), &other]);
    ok(fx.deps.pull(false, true));
    fx.profile("dev", &[&demo_ref()]);
    let preview = ok(fx.deps.upgrade(false));
    assert_eq!(preview.removed, vec![other.clone()]);
    assert!(
        fx.files.lock_text().expect("a lock").contains(&other),
        "a preview drops nothing"
    );
    let applied = ok(fx.deps.upgrade(true));
    assert_eq!(applied.removed, vec![other.clone()]);
    assert!(!fx.files.lock_text().expect("a lock").contains(&other));
    assert!(
        fx.files
            .installed_identities()
            .iter()
            .all(|(i, _)| *i != other),
        "its tree went with it"
    );
}

// deps.feature: An upgrade that could not reach the remote does not call
// everything up to date
#[test]
fn test_an_unreachable_remote_makes_the_round_incomplete() {
    let (fx, _) = pulled();
    fx.remotes.set_unreachable(URL, true);
    let outcome = fx.deps.upgrade(true);
    assert!(outcome.notices.iter().any(
        |n| matches!(n, Notice::Text { text, .. } if text.starts_with(&format!("fetch {URL}")))
    ));
    let report = ok(outcome);
    assert!(report.incomplete && report.changes.is_empty());
}

#[test]
fn test_a_part_of_the_closure_that_cannot_be_read_keeps_its_entries_and_is_said() {
    let (fx, sha) = pulled();
    fx.register("gone", "file:///nonexistent/gone");
    let kit = Fixture::reference("file:///nonexistent/gone", "kit");
    fx.content.bundle(
        "project",
        &[
            crate::fakes::ENVELOPE,
            (
                "profiles/orphan.yaml",
                &format!("parents:\n  - {kit}#profiles/p\n"),
            ),
        ],
    );
    let outcome = fx.deps.upgrade(true);
    assert!(outcome.notices.iter().any(|n| matches!(n, Notice::Text { text, .. } if text.contains("closure is incomplete (1 parent profile(s) unreachable)"))), "{:?}", outcome.notices);
    let report = ok(outcome);
    assert!(report.incomplete && report.removed.is_empty());
    assert!(fx.files.lock_text().expect("a lock").contains(&sha));
}

// upgrade.go checkUpgradedLock
#[test]
fn test_nothing_declared_and_nothing_locked_is_said_and_writes_no_lock() {
    let fx = Fixture::new();
    let report = ok(fx.deps.upgrade(true));
    assert!(report.nothing_declared && report.changes.is_empty());
    assert!(fx.files.lock_text().is_none());
}

// lockfile.go Save: an empty closure never erases a populated lock
#[test]
fn test_a_closure_that_resolves_to_nothing_never_erases_the_lock() {
    let (fx, _) = pulled();
    let lock = fx.files.lock_text();
    fx.profile("dev", &["local-only"]);
    let refused = fx.deps.upgrade(true).result.expect_err("refused");
    assert_eq!(refused.code(), "deps-lock-would-erase");
    assert_eq!(fx.files.lock_text(), lock);
    // A preview refuses exactly what applying it would.
    assert_eq!(
        fx.deps.upgrade(false).result.expect_err("refused").code(),
        "deps-lock-would-erase"
    );
}

#[test]
fn test_an_unregistered_repository_is_refused_and_nothing_is_written() {
    let (fx, _) = pulled();
    let lock = fx.files.lock_text();
    fx.profile(
        "dev",
        &[
            &demo_ref(),
            &Fixture::reference("file:///srv/stranger.git", "x"),
        ],
    );
    let refused = fx.deps.upgrade(true).result.expect_err("refused");
    assert!(matches!(refused, AppError::NotRegistered { .. }));
    assert_eq!(fx.files.lock_text(), lock);
}

#[test]
fn test_a_corrupt_lock_is_refused_and_stands() {
    let (fx, _) = pulled();
    fx.files.set_lock("not: [yaml");
    assert_eq!(
        fx.deps.upgrade(true).result.expect_err("refused").code(),
        "deps-unreadable"
    );
    assert_eq!(fx.files.lock_text().as_deref(), Some("not: [yaml"));
}

#[test]
fn test_two_commits_for_one_item_are_a_conflict() {
    let (fx, sha) = pulled();
    let later = fx.remotes.commit(URL, &demo("1.1.0", "two"));
    fx.content.bundle(
        "project",
        &[
            crate::fakes::ENVELOPE,
            (
                "profiles/one.yaml",
                &format!("bundles:\n  - {}@{sha}\n", demo_ref()),
            ),
            (
                "profiles/two.yaml",
                &format!("bundles:\n  - {}@{later}\n", demo_ref()),
            ),
        ],
    );
    assert!(matches!(
        fx.deps.upgrade(true).result.expect_err("refused"),
        AppError::Conflict { .. }
    ));
}

// pin_change.go: a tree that cannot be read leaves the change listed by its
// header, the reason said
#[test]
fn test_a_move_to_something_that_is_not_a_bundle_is_listed_by_its_header_with_the_reason() {
    let (fx, sha) = pulled();
    fx.remotes
        .delete(URL, &[&format!("{BUNDLES}/demo/bundle.yaml")]);
    fx.remotes
        .commit(URL, &[(&format!("{BUNDLES}/demo/note.txt"), "x")]);
    let outcome = fx.deps.upgrade(false);
    assert!(
        outcome
            .notices
            .iter()
            .any(|n| matches!(n, Notice::Unlisted { .. })),
        "{:?}",
        outcome.notices
    );
    let report = ok(outcome);
    assert_eq!(report.changes.len(), 1);
    assert_eq!(report.changes[0].from_sha, sha);
    // The new side reads as no bundle at all: everything the old one had is
    // shown as gone, and the files say what actually differs.
    assert!(report.changes[0]
        .items
        .iter()
        .all(|i| i.change == pin_changes::Change::Removed));
    assert!(!report.changes[0].files.is_empty());
}

// upgrade.go movePinnedWorktree: best effort, said
#[test]
fn test_a_tree_that_cannot_be_moved_is_said_and_the_lock_stands() {
    let (fx, _) = pulled();
    let later = commit(&fx, &mcp("1.1.0", "fixture-mcp-one", "ONE"));
    fx.files.fail_installs(true);
    let outcome = fx.deps.upgrade(true);
    assert!(outcome.notices.iter().any(|n| matches!(n, Notice::Text { text, remedy } if text.contains("could not be moved to that commit (disk full)") && remedy.contains("ctxloom deps pull"))), "{:?}", outcome.notices);
    ok(outcome);
    assert!(fx.files.lock_text().expect("a lock").contains(&later));
}

// A tag pin never moves; a branch does.
#[test]
fn test_a_tag_pin_stays_where_the_tag_is() {
    let fx = Fixture::new();
    let sha = fx.origin("origin", URL);
    fx.remotes.tag(URL, "v1", &sha);
    fx.profile("dev", &[&format!("{}@tag:v1", demo_ref())]);
    ok(fx.deps.pull(false, true));
    assert!(fx.files.lock_text().expect("a lock").contains("kind: tag"));
    commit(&fx, &mcp("1.1.0", "x", "Y"));
    assert!(ok(fx.deps.upgrade(false)).changes.is_empty());
}

#[test]
fn test_a_complete_closure_says_nothing_of_preserving_entries() {
    let (fx, _) = pulled();
    fx.remotes.commit(URL, &demo("1.1.0", "two"));
    let outcome = fx.deps.upgrade(false);
    assert!(
        !outcome
            .notices
            .iter()
            .any(|n| matches!(n, Notice::Text { text, .. } if text.contains("incomplete"))),
        "{:?}",
        outcome.notices
    );
}
