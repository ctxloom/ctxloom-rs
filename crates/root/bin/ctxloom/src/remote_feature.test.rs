//! Go's `tests/acceptance/features/cli/remote.feature` (20 scenarios), ported.
//! Each test names its scenario. A scenario outline's Examples rows are the
//! `FORMATS` loop: no `--format` (off a terminal, json), json, text.

use super::super::library::{reports_as, Project};
use super::World;
use crate::tests::files;

const URL: &str = "file:///tmp/acceptance-remote.git";

/// Go's `the file "<path>" does not contain "<text>"`: a file that is not
/// there contains nothing.
fn lacks(world: &World, rel: &str, text: &str) {
    if let Some(body) = world.file(rel) {
        assert!(!body.contains(text), "{rel} contains {text:?}: {body}");
    }
}

fn has(world: &World, rel: &str, text: &str) {
    let body = world
        .file(rel)
        .unwrap_or_else(|| panic!("{rel} does not exist"));
    assert!(body.contains(text), "{rel} lacks {text:?}: {body}");
}

fn with_origin() -> World {
    let world = World::initialized();
    world
        .run(&["remote", "create", "origin", URL, "--forge", "git"])
        .succeeds();
    world
}

// remote.feature: Scenario: Registering a remote records it and it is listed back
#[test]
fn test_registering_a_remote_records_it_and_it_is_listed_back() {
    let world = World::initialized();
    world
        .run(&["remote", "create", "origin", URL, "--forge", "git"])
        .succeeds();
    has(&world, ".ctxloom/remotes.yaml", "origin");
    has(&world, ".ctxloom/remotes.yaml", "forge: git");
    world.run(&["remote", "list"]).succeeds().contains("origin");
}

// remote.feature: Scenario: Registering an address that is already a remote is refused
#[test]
fn test_registering_an_address_that_is_already_a_remote_is_refused() {
    let world = with_origin();
    world
        .run(&["remote", "create", "mirror", URL, "--forge", "git"])
        .fails()
        .contains("origin");
    lacks(&world, ".ctxloom/remotes.yaml", "mirror");
}

// remote.feature: Scenario: Registering a remote bound to an unknown forge is refused
#[test]
fn test_registering_a_remote_bound_to_an_unknown_forge_is_refused() {
    let world = World::initialized();
    world
        .run(&[
            "remote",
            "create",
            "origin",
            URL,
            "--forge",
            "no-such-forge",
        ])
        .fails()
        .contains("no-such-forge");
    lacks(&world, ".ctxloom/remotes.yaml", "no-such-forge");
}

// remote.feature: Scenario: Remotes are listed in name order
#[test]
fn test_remotes_are_listed_in_name_order() {
    let world = World::initialized();
    world
        .run(&[
            "remote",
            "create",
            "zeta",
            "file:///tmp/acceptance-zeta.git",
            "--forge",
            "git",
        ])
        .succeeds();
    world
        .run(&[
            "remote",
            "create",
            "alpha",
            "file:///tmp/acceptance-alpha.git",
            "--forge",
            "git",
        ])
        .succeeds();
    let ran = world.run(&["remote", "list", "--format", "text"]);
    ran.succeeds();
    let out = ran.output();
    let (alpha, zeta) = (
        out.find("alpha").expect("alpha"),
        out.find("zeta").expect("zeta"),
    );
    assert!(alpha < zeta, "alpha before zeta: {out}");
}

// remote.feature: Scenario: Bare remote lists the registry
#[test]
fn test_bare_remote_lists_the_registry() {
    let world = with_origin();
    world
        .run(&["remote"])
        .succeeds()
        .contains("origin")
        .lacks("Available Commands:");
}

// remote.feature: Scenario: A remote can be made the default
#[test]
fn test_a_remote_can_be_made_the_default() {
    let world = with_origin();
    world.run(&["remote", "default", "origin"]).succeeds();
    has(&world, ".ctxloom/remotes.yaml", "default: origin");
}

// remote.feature: Scenario Outline: Bare remote remove reports and destroys nothing
#[test]
fn test_bare_remote_remove_reports_and_destroys_nothing() {
    let rows: [(&[&str], &str); 3] = [
        (&[], "false"),
        (&["--format", "json"], "false"),
        (&["--format", "text"], "Nothing was removed"),
    ];
    for (flags, reports) in rows {
        let world = with_origin();
        let mut args = vec!["remote", "remove", "origin"];
        args.extend_from_slice(flags);
        let ran = world.run(&args);
        ran.succeeds();
        if flags.contains(&"text") {
            ran.contains(reports)
                .contains("ctxloom remote remove origin --yes");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "applied"), reports);
            assert_eq!(
                reports_as(&doc, "apply"),
                "ctxloom remote remove origin --yes"
            );
        }
        world.run(&["remote", "list"]).contains("origin");
    }
}

// remote.feature: Scenario: Removing a remote takes it out of the registry
#[test]
fn test_removing_a_remote_takes_it_out_of_the_registry() {
    let world = with_origin();
    world
        .run(&["remote", "remove", "origin", "--yes"])
        .succeeds();
    world.run(&["remote", "list"]).lacks("origin");
}

// remote.feature: Scenario: Correcting a remote's URL records the new address and drops the old
#[test]
fn test_correcting_a_remote_s_url_records_the_new_address_and_drops_the_old() {
    let world = with_origin();
    world
        .run(&[
            "remote",
            "edit",
            "origin",
            "--url",
            "file:///tmp/acceptance-moved.git",
        ])
        .succeeds();
    has(&world, ".ctxloom/remotes.yaml", "acceptance-moved.git");
    lacks(&world, ".ctxloom/remotes.yaml", "acceptance-remote.git");
}

// remote.feature: Scenario: An edit restating a remote's own name and address is accepted
#[test]
fn test_an_edit_restating_a_remote_s_own_name_and_address_is_accepted() {
    let world = with_origin();
    world
        .run(&["remote", "edit", "origin", "--name", "origin", "--url", URL])
        .succeeds();
    has(&world, ".ctxloom/remotes.yaml", "origin");
}

// remote.feature: Scenario: Moving a remote onto another remote's address is refused
#[test]
fn test_moving_a_remote_onto_another_remote_s_address_is_refused() {
    let world = with_origin();
    world
        .run(&[
            "remote",
            "create",
            "mirror",
            "file:///tmp/acceptance-mirror.git",
            "--forge",
            "git",
        ])
        .succeeds();
    world
        .run(&["remote", "edit", "mirror", "--url", URL])
        .fails()
        .contains("origin");
    has(&world, ".ctxloom/remotes.yaml", "acceptance-mirror.git");
}

// remote.feature: Scenario: Rebinding a remote to an unknown forge is refused
#[test]
fn test_rebinding_a_remote_to_an_unknown_forge_is_refused() {
    let world = with_origin();
    world
        .run(&["remote", "edit", "origin", "--forge", "no-such-forge"])
        .fails();
    has(&world, ".ctxloom/remotes.yaml", "forge: git");
    lacks(&world, ".ctxloom/remotes.yaml", "no-such-forge");
}

// remote.feature: Scenario: Rebinding a remote's forge records the new binding
#[test]
fn test_rebinding_a_remote_s_forge_records_the_new_binding() {
    let world = with_origin();
    world
        .run(&["remote", "edit", "origin", "--forge", "github"])
        .succeeds();
    has(&world, ".ctxloom/remotes.yaml", "forge: github");
}

// remote.feature: Scenario: Renaming the default remote carries the default with it
#[test]
fn test_renaming_the_default_remote_carries_the_default_with_it() {
    let world = with_origin();
    world.run(&["remote", "default", "origin"]).succeeds();
    world
        .run(&["remote", "edit", "origin", "--name", "upstream"])
        .succeeds();
    has(&world, ".ctxloom/remotes.yaml", "default: upstream");
    lacks(&world, ".ctxloom/remotes.yaml", "default: origin");
    world
        .run(&["remote", "list"])
        .contains("upstream")
        .lacks("origin");
}

// remote.feature: Scenario: An edit that asks for nothing is refused
#[test]
fn test_an_edit_that_asks_for_nothing_is_refused() {
    let world = with_origin();
    world.run(&["remote", "edit", "origin"]).fails();
    has(&world, ".ctxloom/remotes.yaml", "origin");
}

// remote.feature: Scenario: Browsing a remote lists the bundles it publishes
#[test]
fn test_browsing_a_remote_lists_the_bundles_it_publishes() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world
        .run(&["remote", "show", "origin", "--format", "text"])
        .succeeds()
        .contains("//bundles/demo");
}

// remote.feature: Scenario: A hand-configured forge can be bound, and survives the rewrite
#[test]
fn test_a_hand_configured_forge_can_be_bound_and_survives_the_rewrite() {
    let world = World::initialized();
    world.project.write(
        ".ctxloom/remotes.yaml",
        "schema_version: 1\nforges:\n  corp:\n    type: git\nremotes:\n  origin:\n    url: file:///tmp/acceptance-remote.git\nnote: written by hand\n",
    );
    world
        .run(&[
            "remote",
            "create",
            "mirror",
            "file:///tmp/acceptance-mirror.git",
            "--forge",
            "corp",
        ])
        .succeeds();
    let body = world.file(".ctxloom/remotes.yaml").expect("the registry");
    assert!(
        body.contains("\nforges:\n  corp:") || body.starts_with("forges:\n  corp:"),
        "{body}"
    );
    assert!(body.contains("note: written by hand"), "{body}");
    world
        .run(&["remote", "list"])
        .contains("origin")
        .contains("mirror");
}

// remote.feature: Scenario: A registry that declares no schema generation is refused
#[test]
fn test_a_registry_that_declares_no_schema_generation_is_refused() {
    let world = World::initialized();
    world.project.write(
        ".ctxloom/remotes.yaml",
        "remotes:\n  origin:\n    url: file:///tmp/acceptance-remote.git\n",
    );
    world
        .run(&["remote", "list"])
        .fails()
        .contains("schema_version 0");
}

// remote.feature: Scenario: A remote registered at the root is listed from a subdirectory
#[test]
fn test_a_remote_registered_at_the_root_is_listed_from_a_subdirectory() {
    let world = with_origin();
    let docs = world.project.dir().join("docs");
    files::mkdirs(&docs);
    world
        .project
        .run_in(&docs, &["remote", "list"])
        .succeeds()
        .contains("origin");
}

// remote.feature: Scenario: Registering a first remote outside any project creates a usable home registry
#[test]
fn test_registering_a_first_remote_outside_any_project_creates_a_usable_home_registry() {
    let project = Project::bare();
    let home = project.dir().parent().expect("the root").join("home");
    assert!(
        !files::exists(home.join(".ctxloom")),
        "the home has no .ctxloom yet"
    );
    project
        .run_in(
            project.dir(),
            &["remote", "create", "origin", URL, "--forge", "git"],
        )
        .succeeds();
    // Where the host has permission bits at all.
    if let Some(mode) = files::mode_of(home.join(".ctxloom")) {
        assert_eq!(
            mode & 0o700,
            0o700,
            "readable, writable and searchable by its owner"
        );
    }
    let registry = files::read(home.join(".ctxloom/remotes.yaml")).expect("remotes.yaml");
    assert!(registry.contains("origin"), "{registry}");
}
