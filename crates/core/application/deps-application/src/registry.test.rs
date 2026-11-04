//! `remote list|create|edit|remove|default|show|discover` over the fakes.

use angzarr_diagnostic::Diagnostic;
use deps::registry::Edit;

use crate::fakes::Fixture;
use crate::views::RemoveResult;
use crate::{AppError, CreateRequest, Outcome};

const URL: &str = "file:///srv/origin.git";

fn ok<T>(outcome: Outcome<T>) -> T {
    outcome.result.unwrap_or_else(|e| panic!("{e:?}"))
}

fn err<T: std::fmt::Debug>(outcome: Outcome<T>) -> AppError {
    outcome.result.expect_err("a refusal")
}

fn create(name: &str, url: &str) -> CreateRequest {
    CreateRequest {
        name: name.to_string(),
        url: url.to_string(),
        forge: "git".to_string(),
    }
}

// remote.feature: Registering a remote records it and it is listed back
#[test]
fn test_creating_a_remote_registers_it_and_clones_it() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    let created = ok(fx.deps.remote_create(&create("origin", URL)));
    assert_eq!(
        (created.status, created.name.as_str(), created.url.as_str()),
        ("added", "origin", URL)
    );
    assert_eq!(created.warning, "");
    assert_eq!(fx.remotes.calls(), vec![format!("ensure {URL}")]);
    assert_eq!(
        fx.files.registry_text().as_deref(),
        Some("remotes:\n  origin:\n    url: file:///srv/origin.git\n    forge: git\nschema_version: 1\n")
    );
    let listing = ok(fx.deps.remote_list());
    assert_eq!(listing.count, 1);
    assert_eq!(listing.remotes[0].name, "origin");
}

// remote.feature: a clone that fails is a warning, not a refusal
#[test]
fn test_a_clone_that_fails_leaves_the_remote_registered_with_a_warning() {
    let fx = Fixture::new();
    let outcome = fx
        .deps
        .remote_create(&create("gone", "file:///nonexistent/x.git"));
    let notices = outcome.notices.clone();
    let created = ok(outcome);
    assert!(
        created.warning.starts_with("gone: clone failed: "),
        "{}",
        created.warning
    );
    assert_eq!(notices.len(), 1);
    assert!(fx.files.registry_text().expect("written").contains("gone:"));
}

#[test]
fn test_a_name_and_an_address_are_required_in_that_order() {
    let fx = Fixture::new();
    let required = |request: CreateRequest| match err(fx.deps.remote_create(&request)) {
        AppError::Required { what } => what,
        other => panic!("{other:?}"),
    };
    assert_eq!(required(create("", "")), "name");
    assert_eq!(required(create("origin", "")), "url");
    assert_eq!(fx.files.registry_writes(), 0);
}

// remote.feature: Registering an address that is already a remote is refused /
// bound to an unknown forge is refused
#[test]
fn test_a_remote_that_cannot_be_registered_changes_nothing() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    ok(fx.deps.remote_create(&create("origin", URL)));
    let writes = fx.files.registry_writes();
    for (request, code) in [
        (create("mirror", URL), "deps-same-repository"),
        (create("origin", "file:///other.git"), "deps-remote-exists"),
        (
            CreateRequest {
                forge: "no-such-forge".to_string(),
                ..create("second", "file:///second.git")
            },
            "deps-unknown-forge",
        ),
        (create("bare", "bundles"), "deps-no-repository"),
    ] {
        assert_eq!(err(fx.deps.remote_create(&request)).code(), code);
    }
    assert_eq!(fx.files.registry_writes(), writes);
}

// cli/remote.go: a local path is resolved to the repository there
#[test]
fn test_a_path_is_registered_as_the_file_url_of_the_repository_there() {
    let fx = Fixture::new();
    let created = ok(fx.deps.remote_create(&create("local", "/repos/mine.git")));
    assert_eq!(created.url, "file:///repos/mine.git");
    let refused = err(fx
        .deps
        .remote_create(&create("nowhere", "/elsewhere/x.git")));
    assert_eq!(refused.code(), "deps-no-repository");
    assert!(fx
        .files
        .registry_text()
        .expect("written")
        .contains("local:"));
    assert!(!fx
        .files
        .registry_text()
        .expect("written")
        .contains("nowhere"));
}

#[test]
fn test_remotes_are_listed_by_name_with_the_default() {
    let fx = Fixture::new();
    assert_eq!(ok(fx.deps.remote_list()).count, 0);
    fx.register("zeta", "file:///z.git");
    fx.register("alpha", "file:///a.git");
    ok(fx.deps.remote_default(Some("zeta")));
    let listing = ok(fx.deps.remote_list());
    let names: Vec<&str> = listing.remotes.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "zeta"]);
    assert_eq!(listing.default, "zeta");
    assert_eq!(listing.count, 2);
}

// remote.feature: Correcting a remote's URL / An edit that asks for nothing
#[test]
fn test_an_edit_reports_what_moved_and_writes_only_a_change() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    let writes = fx.files.registry_writes();
    let edit = |edit: Edit| fx.deps.remote_edit("origin", &edit);
    let same = ok(edit(Edit {
        name: Some("origin".to_string()),
        url: Some(URL.to_string()),
        forge: None,
    }));
    assert!(same.changed.is_empty());
    assert_eq!(
        fx.files.registry_writes(),
        writes,
        "restating is not a change"
    );
    let moved = ok(edit(Edit {
        url: Some("file:///moved.git".to_string()),
        ..Edit::default()
    }));
    assert_eq!(moved.changed, vec!["url"]);
    assert_eq!(moved.before.url, URL);
    assert_eq!(moved.after.url, "file:///moved.git");
    assert!(!moved.default_pointer_updated);
    assert_eq!(fx.files.registry_writes(), writes + 1);
    assert_eq!(err(edit(Edit::default())).code(), "deps-nothing-to-edit");
}

#[test]
fn test_renaming_the_default_remote_says_the_default_followed() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    ok(fx.deps.remote_default(Some("origin")));
    let renamed = ok(fx.deps.remote_edit(
        "origin",
        &Edit {
            name: Some("upstream".to_string()),
            ..Edit::default()
        },
    ));
    assert!(renamed.default_pointer_updated);
    assert_eq!(renamed.name, "upstream");
    assert_eq!(ok(fx.deps.remote_list()).default, "upstream");
}

#[test]
fn test_an_edit_resolves_a_path_too() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    let edited = ok(fx.deps.remote_edit(
        "origin",
        &Edit {
            url: Some("/repos/new.git".to_string()),
            ..Edit::default()
        },
    ));
    assert_eq!(edited.after.url, "file:///repos/new.git");
}

// remote.feature: Bare remote remove reports and destroys nothing
#[test]
fn test_a_removal_previews_until_it_is_applied() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    let writes = fx.files.registry_writes();
    match ok(fx.deps.remote_remove("origin", false)) {
        RemoveResult::Preview {
            status,
            applied,
            target,
            apply,
        } => {
            assert_eq!((status, applied), ("preview", false));
            assert_eq!(target, "remote \"origin\"");
            assert_eq!(apply, "ctxloom remote remove origin --yes");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fx.files.registry_writes(), writes);
    assert_eq!(ok(fx.deps.remote_list()).count, 1);
    assert!(matches!(
        ok(fx.deps.remote_remove("origin", true)),
        RemoveResult::Removed {
            status: "removed",
            ..
        }
    ));
    assert_eq!(ok(fx.deps.remote_list()).count, 0);
}

#[test]
fn test_removing_a_remote_that_is_not_there_is_refused_either_way() {
    let fx = Fixture::new();
    assert!(matches!(
        err(fx.deps.remote_remove("nope", false)),
        AppError::NoRemoteNamed { .. }
    ));
    assert_eq!(
        err(fx.deps.remote_remove("nope", true)).code(),
        "deps-remote-not-found"
    );
}

#[test]
fn test_the_default_is_set_to_a_remote_that_is_there_and_cleared() {
    let fx = Fixture::new();
    fx.register("origin", URL);
    let set = ok(fx.deps.remote_default(Some("origin")));
    assert_eq!((set.status, set.name.as_str()), ("set", "origin"));
    assert_eq!(
        err(fx.deps.remote_default(Some("nope"))).code(),
        "deps-remote-not-found"
    );
    let cleared = ok(fx.deps.remote_default(None));
    assert_eq!((cleared.status, cleared.name.as_str()), ("cleared", ""));
    assert_eq!(ok(fx.deps.remote_list()).default, "");
}

// remote.feature: Browsing a remote lists the bundles it publishes
#[test]
fn test_showing_a_remote_lists_the_bundles_it_publishes_with_the_reference_that_installs_each() {
    let fx = Fixture::new();
    fx.origin("origin", URL);
    let shown = ok(fx.deps.remote_show("origin"));
    assert_eq!(
        (shown.remote.as_str(), shown.url.as_str(), shown.count),
        ("origin", URL, 1)
    );
    assert_eq!(shown.items[0].name, "demo");
    assert_eq!(shown.items[0].kind, "bundle");
    assert_eq!(shown.items[0].path, "demo");
    assert!(shown.items[0].is_dir);
    assert_eq!(shown.items[0].pull_ref, Fixture::reference(URL, "demo"));
    assert!(shown.warnings.is_empty());
}

#[test]
fn test_a_remote_that_cannot_be_browsed_is_not_an_empty_one() {
    let fx = Fixture::new();
    let missing = err(fx.deps.remote_show("nope"));
    match missing {
        AppError::Browse { remote, detail } => {
            assert_eq!(remote, "nope");
            assert_eq!(detail, "remote not found: nope");
        }
        other => panic!("{other:?}"),
    }
    fx.register("origin", URL);
    match err(fx.deps.remote_show("origin")) {
        AppError::Browse { remote, detail } => {
            assert_eq!(remote, "origin");
            assert!(
                detail.contains("does not appear to be a git repository"),
                "{detail}"
            );
        }
        other => panic!("{other:?}"),
    }
}

// remote.go: discover searches GitHub through its API
#[test]
fn test_discover_is_refused_without_a_forge_api() {
    let fx = Fixture::new();
    let refused = err(fx.deps.remote_discover());
    assert!(matches!(refused, AppError::DiscoverNeedsForge));
    assert_eq!(refused.exit_code(), 2);
}
