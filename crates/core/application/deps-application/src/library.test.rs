//! What the library reads of the remotes, over the fakes.

use deps_ports::{RemoteLibrary, Remotes};

use crate::fakes::{demo, Fixture, BUNDLES};
use crate::Outcome;

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

#[test]
fn test_installed_bundles_are_named_by_their_remote_and_carry_their_tree() {
    let fx = Fixture::new();
    fx.remotes.serve(URL);
    let mut files: Vec<(String, String, bool)> = demo("1.0.0", "x")
        .iter()
        .map(|(p, t)| (p.to_string(), t.to_string(), false))
        .collect();
    files.push((
        format!("{BUNDLES}/demo/skills/runner/scripts/run.sh"),
        "#!/bin/sh\n".to_string(),
        true,
    ));
    let refs: Vec<(&str, &str, bool)> = files
        .iter()
        .map(|(p, t, e)| (p.as_str(), t.as_str(), *e))
        .collect();
    let sha = fx.remotes.commit_modes(URL, &refs);
    fx.register("origin", URL);
    fx.profile("dev", &[&demo_ref()]);
    ok(fx.deps.pull(false, true));
    let installed = fx.deps.installed().expect("readable");
    assert_eq!(installed.len(), 1);
    let bundle = &installed[0];
    assert_eq!(bundle.reference, demo_ref());
    assert_eq!(
        (
            bundle.name.as_str(),
            bundle.url.as_str(),
            bundle.sha.as_str()
        ),
        ("origin/demo", URL, sha.as_str())
    );
    assert_eq!(bundle.path, format!("/p/.ctxloom/cache/bundles/{sha}"));
    assert!(bundle.files.contains_key("bundle.yaml"));
    assert_eq!(
        bundle
            .executable
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["skills/runner/scripts/run.sh"]
    );
}

#[test]
fn test_a_bundle_whose_remote_was_unregistered_is_named_by_its_reference() {
    let (fx, _) = pulled();
    ok(fx.deps.remote_remove("origin", true));
    assert_eq!(fx.deps.installed().expect("readable")[0].name, demo_ref());
}

#[test]
fn test_a_lock_entry_whose_tree_is_gone_is_not_installed() {
    let (fx, sha) = pulled();
    fx.files.uninstall(&demo_ref(), &sha);
    assert!(fx.deps.installed().expect("readable").is_empty());
}

#[test]
fn test_an_unreadable_lock_is_a_failure_the_library_can_say() {
    let (fx, _) = pulled();
    fx.files.set_lock("not: [yaml");
    assert!(fx.deps.installed().is_err());
}

#[test]
fn test_a_short_reference_is_spelled_out_unless_a_local_bundle_wins() {
    let (fx, _) = pulled();
    let none = |_: &str| false;
    assert_eq!(
        fx.deps.canonical("origin/demo#profiles/dev", &none),
        format!("{}#profiles/dev", demo_ref())
    );
    assert_eq!(fx.deps.canonical("demo", &none), "demo");
    assert_eq!(
        fx.deps
            .canonical("origin/demo", &|base: &str| base == "origin/demo"),
        "origin/demo"
    );
    fx.files.set_registry("not: [yaml");
    assert_eq!(
        fx.deps.canonical("origin/demo", &none),
        "origin/demo",
        "no registry, no meaning"
    );
}

#[test]
fn test_a_sibling_reference_is_within_its_repository_at_its_commit() {
    let (fx, sha) = pulled();
    assert_eq!(
        fx.deps.within("demo", URL, &sha),
        format!("{}@{sha}", demo_ref())
    );
}

// operations.SearchRemotes
#[test]
fn test_a_search_finds_bundles_by_name_or_description_in_remote_then_name_order() {
    let fx = Fixture::new();
    fx.origin("zeta", "file:///srv/z.git");
    fx.origin("alpha", "file:///srv/a.git");
    fx.remotes.commit(
        "file:///srv/a.git",
        &[
            (&format!("{BUNDLES}/other/bundle.yaml"), "schema_version: 4\nversion: 1.0.0\ndescription: all about DEMOs\nauthor: Ann\ntags: [t1, t2]\n"),
            (&format!("{BUNDLES}/unrelated/bundle.yaml"), "description: nothing\n"),
        ],
    );
    // A search reads the clone as the last fetch left it.
    fx.remotes
        .refresh(&crate::cache::repository("file:///srv/a.git").expect("a repository"))
        .expect("fetched");
    let found = fx.deps.search("demo").expect("searches");
    let hits: Vec<(&str, &str)> = found
        .hits
        .iter()
        .map(|h| (h.remote.as_str(), h.name.as_str()))
        .collect();
    assert_eq!(
        hits,
        vec![("alpha", "demo"), ("alpha", "other"), ("zeta", "demo")]
    );
    assert!(found.warnings.is_empty());
    let other = &found.hits[1];
    assert_eq!(
        (other.description.as_str(), other.author.as_str()),
        ("all about DEMOs", "Ann")
    );
    assert_eq!(other.tags, vec!["t1", "t2"]);
    assert_eq!(
        other.pull_ref,
        Fixture::reference("file:///srv/a.git", "other")
    );
    assert_eq!(found.hits[0].description, "Demo bundle");
}

#[test]
fn test_a_search_with_no_remote_says_so_and_a_remote_that_fails_is_said_by_name() {
    let fx = Fixture::new();
    let none = fx.deps.search("demo").expect("searches");
    assert!(none.hits.is_empty());
    assert_eq!(none.warnings, vec!["no remotes configured"]);
    fx.origin("good", "file:///srv/good.git");
    fx.register("bad", "file:///srv/bad.git");
    let found = fx.deps.search("demo").expect("searches");
    assert_eq!(found.hits.len(), 1);
    assert_eq!(found.warnings.len(), 1);
    assert!(
        found.warnings[0].starts_with("bad (bundle): "),
        "{:?}",
        found.warnings
    );
}

#[test]
fn test_a_search_over_an_unreadable_registry_fails() {
    let fx = Fixture::new();
    fx.files.set_registry("not: [yaml");
    assert!(fx.deps.search("demo").is_err());
}
