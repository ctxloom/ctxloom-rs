//! Go's `tests/acceptance/features/cli/deps.feature` (36 scenarios), ported:
//! 32 here, the 4 of "Pull writes the project's engine files only with --yes"
//! wait for S9's materialize (see `deps.test.rs`). Each test names its
//! scenario. A scenario outline's Examples rows are the `FORMATS` loop.
//!
//! plan supersedes Go: where a scenario reads `materialize dev --target out
//! --backend claude-code` and its `out/CLAUDE.md`, the context is read from
//! `context assemble -p dev` (materialize is S9, plan 8.2).

use super::super::library::{entries, reports_as};
use super::World;

const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

/// What `context assemble -p dev` carries, as text.
fn assembled(world: &World) -> String {
    let ran = world.run(&["context", "assemble", "-p", "dev", "--format", "text"]);
    ran.succeeds();
    ran.stdout.clone()
}

/// Go's `the file "<path>" contains "<text>" exactly <n> times`.
fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

// deps.feature: Scenario Outline: A project with nothing installed reports an
// empty closure
#[test]
fn test_a_project_with_nothing_installed_reports_an_empty_closure() {
    for flags in FORMATS {
        let world = World::initialized();
        let mut args = flags.to_vec();
        args.extend(["deps", "list"]);
        let ran = world.run(&args);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("No dependencies installed");
        } else {
            assert_eq!(entries(&ran.json(), "deps"), 0);
        }
    }
}

// deps.feature: Scenario: A project with nothing installed says what to run next
#[test]
fn test_a_project_with_nothing_installed_says_what_to_run_next() {
    let world = World::initialized();
    world
        .run(&["--format", "text", "deps", "list"])
        .succeeds()
        .contains("ctxloom deps pull");
}

// deps.feature: Scenario: The listing names each dependency, its commit and its origin
#[test]
fn test_the_listing_names_each_dependency_its_commit_and_its_origin() {
    let world = World::pulled_dev();
    let ran = world.run(&["deps", "list"]);
    ran.succeeds().contains("demo").contains("origin");
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "count"), "1");
    assert_eq!(reports_as(&doc, "deps.0.name"), "demo");
    assert_eq!(reports_as(&doc, "deps.0.origin"), "origin");
    assert_eq!(reports_as(&doc, "deps.0.held"), "false");
    assert_eq!(reports_as(&doc, "deps.0.sha").len(), 40);
}

// deps.feature: Scenario Outline: Bare deps lists the closure
#[test]
fn test_bare_deps_lists_the_closure() {
    for flags in FORMATS {
        let world = World::initialized();
        let mut args = flags.to_vec();
        args.push("deps");
        let ran = world.run(&args);
        ran.succeeds().lacks("Available Commands:");
        if is_text(flags) {
            ran.contains("No dependencies installed");
        } else {
            assert_eq!(entries(&ran.json(), "deps"), 0);
        }
    }
}

// deps.feature: Scenario: A held dependency is marked held in the listing
#[test]
fn test_a_held_dependency_is_marked_held_in_the_listing() {
    let world = World::pulled_dev();
    world.run(&["deps", "hold", "origin/demo"]).succeeds();
    world.run(&["deps", "list"]).succeeds().contains("held");
}

// deps.feature: Scenario: Referencing a remote bundle and pulling it locks the
// dependency closure
#[test]
fn test_referencing_a_remote_bundle_and_pulling_it_locks_the_dependency_closure() {
    let world = World::unpulled_dev();
    world.run(&["deps", "pull"]).succeeds();
    assert!(world.lock().contains("//bundles/demo"), "{}", world.lock());
}

// deps.feature: Scenario: Pulling straight from an unregistered address is refused
#[test]
fn test_pulling_straight_from_an_unregistered_address_is_refused() {
    let mut world = World::initialized();
    world.profile_from_unregistered("dev");
    world
        .run(&["deps", "pull"])
        .fails()
        .contains("remote not registered")
        .contains("ctxloom remote create <name> file://");
    assert!(
        world.file(".ctxloom/lock.yaml").is_none(),
        "no lock is written"
    );
    let listed = world.run(&["remote", "list", "--format", "json"]);
    let doc = listed.json();
    let names = doc["remotes"].as_array().expect("remotes");
    assert!(names.iter().all(|r| r["name"] != "remote"), "{doc}");
}

// deps.feature: Scenario: A remote profile reaching into another repository is refused
#[test]
fn test_a_remote_profile_reaching_into_another_repository_is_refused() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.git_remote_reaching("vendor", "kit", "reach", "demo", "origin");
    world.profile_including("dev", &[("vendor", "kit")]);
    world.run(&["deps", "pull"]);
    world
        .run(&["profile", "show", "vendor/kit#profiles/reach"])
        .fails()
        .contains("a remote profile may refer only to bundles in its own repository")
        .contains("//bundles/demo");
}

// deps.feature: Scenario: A local profile draws on several registered remotes
#[test]
fn test_a_local_profile_draws_on_several_registered_remotes() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.git_remote("vendor");
    world.profile_including("dev", &[("origin", "demo"), ("vendor", "demo")]);
    world.run(&["deps", "pull"]).succeeds();
    assert_eq!(
        count(&world.lock(), "//bundles/demo"),
        2,
        "{}",
        world.lock()
    );
}

// deps.feature: Scenario: A second pull is incremental — an already-locked
// dependency is not re-fetched
#[test]
fn test_a_second_pull_is_incremental() {
    let world = World::pulled_dev();
    world
        .run(&["deps", "pull", "--format", "text"])
        .succeeds()
        .contains("Skipped (kept at their locked commit)");
}

// deps.feature: Scenario: A skipped pull leaves old content in place, and says
// so rather than implying it is current
#[test]
fn test_a_skipped_pull_leaves_old_content_in_place_and_says_so() {
    let mut world = World::pulled_dev();
    world.changes_fragment("origin", "demo-frag", "MARKER-SKIPPED-PULL-never-seen");
    world
        .run(&["deps", "pull", "--format", "text"])
        .succeeds()
        .contains("Skipped (kept at their locked commit)")
        .contains("ctxloom deps check")
        .lacks("ctxloom deps upgrade")
        .lacks("already installed");
    let context = assembled(&world);
    assert!(context.contains("Demo fragment content."), "{context}");
    assert!(
        !context.contains("MARKER-SKIPPED-PULL-never-seen"),
        "{context}"
    );
}

// deps.feature: Scenario: A bundle deleted upstream is removed locally, by name
#[test]
fn test_a_bundle_deleted_upstream_is_removed_locally_by_name() {
    let mut world = World::pulled_dev();
    world.stops_publishing("origin", "demo");
    world
        .run(&["deps", "pull", "--format", "text"])
        .succeeds()
        .contains("no longer published")
        .contains("demo");
    assert!(!world.lock().contains("//bundles/demo"), "{}", world.lock());
}

// deps.feature: Scenario: An unreachable remote removes nothing and says it
// could not check
#[test]
fn test_an_unreachable_remote_removes_nothing_and_says_it_could_not_check() {
    let mut world = World::pulled_dev();
    world.becomes_unreachable("origin");
    world
        .run(&["deps", "pull", "--format", "text"])
        .contains("could not be reached")
        .lacks("no longer published");
    assert!(world.lock().contains("//bundles/demo"), "{}", world.lock());
    let context = assembled(&world);
    assert!(context.contains("Demo fragment content."), "{context}");
}

// deps.feature: Scenario: A remote that cannot be reached leaves the listing intact
#[test]
fn test_a_remote_that_cannot_be_reached_leaves_the_listing_intact() {
    let mut world = World::pulled_dev();
    world.becomes_unreachable("origin");
    world.run(&["deps", "pull"]);
    world.run(&["deps", "list"]).succeeds().contains("demo");
}

// deps.feature: Scenario: A pull whose lock could not reach part of the closure
// names what it kept
#[test]
fn test_a_pull_whose_lock_could_not_reach_part_of_the_closure_names_what_it_kept() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.run(&["remote", "default", "origin"]).succeeds();
    world
        .run(&[
            "remote",
            "create",
            "gone",
            "file:///nonexistent/nonexistent-ctxloom-remote",
            "--forge",
            "git",
        ])
        .succeeds();
    world.profile_including("dev", &[("origin", "demo")]);
    world.project.write(
        ".ctxloom/content/bundles/v2/project/profiles/orphan.yaml",
        "parents:\n  - file:///nonexistent/nonexistent-ctxloom-remote@bundles/kit#profiles/parent\n",
    );
    world
        .run(&["deps", "pull", "--format", "text"])
        .contains("Lock incomplete")
        .contains("nonexistent-ctxloom-remote")
        .contains("previous lock entries were kept");
}

// deps.feature: Scenario: A check that could not reach the remote does not call
// it up to date
#[test]
fn test_a_check_that_could_not_reach_the_remote_does_not_call_it_up_to_date() {
    let mut world = World::pulled_dev();
    world.becomes_unreachable("origin");
    world
        .run(&["deps", "check", "--format", "text"])
        .lacks("up to date")
        .contains("could not be checked");
}

// deps.feature: Scenario: An upgrade that could not reach the remote does not
// call everything up to date
#[test]
fn test_an_upgrade_that_could_not_reach_the_remote_does_not_call_everything_up_to_date() {
    let mut world = World::pulled_dev();
    world.becomes_unreachable("origin");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .lacks("Everything is up to date")
        .contains("unreachable");
}

// deps.feature: Scenario: Check reports an available advance and changes nothing
#[test]
fn test_check_reports_an_available_advance_and_changes_nothing() {
    let mut world = World::pulled_dev();
    world.advances("origin");
    let before = world.lock();
    world
        .run(&["deps", "check", "--format", "text"])
        .succeeds()
        .contains("ctxloom deps upgrade");
    world.run(&["deps", "list"]).contains("demo");
    assert_eq!(world.lock(), before, "check changes nothing");
}

// deps.feature: Scenario: Check on an empty closure has nothing to check
#[test]
fn test_check_on_an_empty_closure_has_nothing_to_check() {
    let world = World::initialized();
    world
        .run(&["deps", "check", "--format", "text"])
        .succeeds()
        .contains("nothing to check");
}

// deps.feature: Scenario: An upgrade advances the pin directly
#[test]
fn test_an_upgrade_advances_the_pin_directly() {
    let mut world = World::pulled_dev();
    world.advances("origin");
    world
        .run(&["deps", "upgrade", "--yes", "--format", "text"])
        .succeeds()
        .contains("Applied 1 pin(s).");
}

// deps.feature: Scenario: An upgrade names each dependency it drops from the lock
#[test]
fn test_an_upgrade_names_each_dependency_it_drops_from_the_lock() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.git_remote("other");
    world.run(&["remote", "default", "origin"]).succeeds();
    world.profile_including("dev", &[("origin", "demo"), ("other", "demo")]);
    world.run(&["deps", "pull"]).succeeds();
    // profile modify dev --remove-bundle other/demo
    world.profile_including("dev", &[("origin", "demo")]);
    world
        .run(&["deps", "upgrade", "--yes", "--format", "text"])
        .succeeds()
        .contains("from the lockfile: nothing this project composes depends on it any more.");
}

// deps.feature: Scenario: A pull right after an upgrade does not tell the user
// to upgrade again
#[test]
fn test_a_pull_right_after_an_upgrade_does_not_tell_the_user_to_upgrade_again() {
    let mut world = World::pulled_dev();
    world.advances("origin");
    world.run(&["deps", "upgrade", "--yes"]).succeeds();
    world
        .run(&["deps", "pull"])
        .succeeds()
        .lacks("ctxloom deps upgrade");
}

// deps.feature: Scenario: An upgrade without --yes shows the move and writes nothing
#[test]
fn test_an_upgrade_without_yes_shows_the_move_and_writes_nothing() {
    let mut world = World::pulled_dev();
    world.ships("origin", "srv", "fixture-mcp-one", "SCRIPT-ONE");
    let before = world.lock();
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .succeeds()
        .contains("+ mcp srv")
        .contains("command: fixture-mcp-one")
        .contains("+ skill runner")
        .contains("+echo SCRIPT-ONE")
        .contains("1 pin(s) would move. Re-run with --yes to apply.");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .contains("1 pin(s) would move.");
    assert_eq!(world.lock(), before, "a preview writes nothing");
}

// deps.feature: Scenario: An upgrade with --yes applies the move and shows what it applied
#[test]
fn test_an_upgrade_with_yes_applies_the_move_and_shows_what_it_applied() {
    let mut world = World::pulled_dev();
    world.ships("origin", "srv", "fixture-mcp-one", "SCRIPT-ONE");
    world
        .run(&["deps", "upgrade", "--yes", "--format", "text"])
        .succeeds()
        .contains("+ mcp srv")
        .contains("Applied 1 pin(s).");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .contains("Everything is up to date.");
}

// deps.feature: Scenario: An upgrade shows what an MCP server and a script run
// before and after
#[test]
fn test_an_upgrade_shows_what_an_mcp_server_and_a_script_run_before_and_after() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.ships("origin", "srv", "fixture-mcp-one", "SCRIPT-ONE");
    world.run(&["remote", "default", "origin"]).succeeds();
    world.profile_including("dev", &[("origin", "demo")]);
    world.run(&["deps", "pull"]).succeeds();
    world.ships("origin", "srv", "fixture-mcp-two", "SCRIPT-TWO");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .succeeds()
        .contains("~ mcp srv")
        .contains("command: fixture-mcp-one -> fixture-mcp-two")
        .contains("-echo SCRIPT-ONE")
        .contains("+echo SCRIPT-TWO")
        .contains("~ API_KEY: <")
        .lacks("secret-SCRIPT-ONE")
        .lacks("secret-SCRIPT-TWO");
}

// deps.feature: Scenario: A first pin shows everything the bundle brings in,
// executables included
#[test]
fn test_a_first_pin_shows_everything_the_bundle_brings_in_executables_included() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.ships("origin", "srv", "fixture-mcp-one", "SCRIPT-ONE");
    world.run(&["remote", "default", "origin"]).succeeds();
    world.profile_including("dev", &[("origin", "demo")]);
    world
        .run(&["deps", "pull", "--format", "text"])
        .succeeds()
        .contains("New pins, with everything each one brings in:")
        .contains("first pin")
        .contains("+ mcp srv")
        .contains("command: fixture-mcp-one")
        .contains("+ fragment demo-frag")
        .contains("+echo SCRIPT-ONE")
        .contains("API_KEY: <")
        .lacks("secret-SCRIPT-ONE");
}

// deps.feature: Scenario: A pull never moves an existing pin, even when its
// constraint changed
#[test]
fn test_a_pull_never_moves_an_existing_pin_even_when_its_constraint_changed() {
    let mut world = World::pulled_dev();
    world.advances("origin");
    // profile modify dev --remove-bundle origin/demo --add-bundle origin/demo@main
    let pinned = format!("{}@main", world.reference("origin", "demo"));
    world.write_profile("dev", &[pinned]);
    world
        .run(&["deps", "pull", "--force", "--format", "text"])
        .succeeds()
        .contains("the manifest now asks for main; the pin stays at")
        .contains("ctxloom deps upgrade --yes")
        .lacks("New pins");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .contains("1 pin(s) would move.");
}

// deps.feature: Scenario: A held dependency is not upgraded
#[test]
fn test_a_held_dependency_is_not_upgraded() {
    let mut world = World::pulled_dev();
    world.run(&["deps", "hold", "origin/demo"]).succeeds();
    world.advances("origin");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .succeeds()
        .contains("up to date");
}

// deps.feature: Scenario: A held dependency's content survives even a forced pull
#[test]
fn test_a_held_dependency_s_content_survives_even_a_forced_pull() {
    let mut world = World::pulled_dev();
    world.run(&["deps", "hold", "origin/demo"]).succeeds();
    world.advances("origin");
    world.run(&["deps", "pull", "--force"]).succeeds();
    assert!(world.lock().contains("held: true"), "{}", world.lock());
    let context = assembled(&world);
    assert!(context.contains("Demo fragment content."), "{context}");
    assert!(!context.contains("version two"), "{context}");
}

// deps.feature: Scenario: Unholding lets the pin move again
#[test]
fn test_unholding_lets_the_pin_move_again() {
    let mut world = World::pulled_dev();
    world.run(&["deps", "hold", "origin/demo"]).succeeds();
    world.advances("origin");
    world.run(&["deps", "unhold", "origin/demo"]).succeeds();
    world
        .run(&["deps", "upgrade", "--yes", "--format", "text"])
        .succeeds()
        .contains("Applied 1 pin(s).");
}

// deps.feature: Scenario: An upstream content change reaches the assembled
// context once the pin advances
#[test]
fn test_an_upstream_content_change_reaches_the_assembled_context_once_the_pin_advances() {
    let mut world = World::pulled_dev();
    let before = assembled(&world);
    assert!(before.contains("Demo fragment content."), "{before}");
    world.changes_fragment("origin", "demo-frag", "MARKER-BRAVO-second-edition");
    world.run(&["deps", "upgrade", "--yes"]).succeeds();
    let after = assembled(&world);
    assert!(after.contains("MARKER-BRAVO-second-edition"), "{after}");
    assert!(!after.contains("Demo fragment content."), "{after}");
}

// deps.feature: Scenario: A stale local checkout never leaks into what's served
#[test]
fn test_a_stale_local_checkout_never_leaks_into_what_s_served() {
    let mut world = World::pulled_dev();
    world.changes_fragment("origin", "demo-frag", "MARKER-STALE-CHECKOUT-current");
    world.run(&["deps", "upgrade", "--yes"]).succeeds();
    world.clone_forced_back_to_first_commit();
    let context = assembled(&world);
    assert!(
        context.contains("MARKER-STALE-CHECKOUT-current"),
        "{context}"
    );
    assert!(!context.contains("Demo fragment content."), "{context}");
}

// lockfile_test.go / types.go (LockfileVersion): "a pull at unchanged pins
// rewrites it byte for byte"; the lock records no time. Plan 8.6: lock.yaml
// schema 3, byte-identical rewrite at unchanged pins.
#[test]
fn test_a_pull_and_a_hold_round_trip_leave_the_lockfile_byte_identical() {
    let world = World::pulled_dev();
    let first = world.lock();
    assert!(
        first.starts_with("schema_version: 3\nbundles:\n"),
        "{first}"
    );
    world.run(&["deps", "pull"]).succeeds();
    assert_eq!(world.lock(), first, "a second pull rewrites nothing");
    world.run(&["deps", "hold", "origin/demo"]).succeeds();
    assert_ne!(world.lock(), first, "a hold is recorded");
    world.run(&["deps", "unhold", "origin/demo"]).succeeds();
    assert_eq!(world.lock(), first, "an unhold restores the bytes");
}

// lockfile_guard_test.go: a lockfile that cannot be read is refused and never
// overwritten (ErrLockfileUnreadable); the pins and holds in it are not ours
// to replace.
#[test]
fn test_a_corrupt_lockfile_is_refused_and_never_overwritten() {
    let world = World::pulled_dev();
    world.project.write(
        ".ctxloom/lock.yaml",
        "schema_version: 3\nbundles: [unterminated\n",
    );
    for args in [
        &["deps", "pull"][..],
        &["deps", "upgrade", "--yes"],
        &["deps", "hold", "origin/demo"],
    ] {
        world.run(args).fails();
        assert_eq!(
            world.lock(),
            "schema_version: 3\nbundles: [unterminated\n",
            "{args:?} must leave the corrupt lock as it is"
        );
    }
}
