//! The remote halves of Go's `search.feature` (the two `@remote` outlines) and
//! `bundle.feature` (push, move, and holding a local bundle), ported.

use forge_github::stub::Stub;

use super::super::library::{entries, reports_as};
use super::{tree_bundle, World};

const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

/// `ctxloom search <args...> <flags>`.
fn search(world: &World, args: &[&str], flags: &[&str]) -> super::Ran {
    let mut all = vec!["search"];
    all.extend_from_slice(args);
    all.extend_from_slice(flags);
    let ran = world.run(&all);
    ran.succeeds();
    ran
}

fn notes_with_local_fragment(world: &World) {
    world.project.bundle("notes");
    world.project.fragment("notes", "demo-local-note");
}

// search.feature: Scenario Outline: One query answers from both sources, and
// each flag narrows it to one
#[test]
fn test_one_query_answers_from_both_sources_and_each_flag_narrows_it_to_one() {
    for flags in FORMATS {
        let mut world = World::initialized();
        world.git_remote("origin");
        notes_with_local_fragment(&world);

        let both = search(&world, &["demo"], flags);
        let local = search(&world, &["--local", "demo"], flags);
        let remote = search(&world, &["--remote", "demo"], flags);
        if is_text(flags) {
            both.contains("Results (2):").contains("- demo-local-note");
            local.contains("Results (1):").contains("- demo-local-note");
            remote.contains("Results (1):").contains("demo");
        } else {
            let doc = both.json();
            assert_eq!(reports_as(&doc, "count"), "2");
            assert_eq!(
                reports_as(&doc, "local[name=demo-local-note].type"),
                "fragment"
            );
            let doc = local.json();
            assert_eq!(reports_as(&doc, "count"), "1");
            assert_eq!(
                reports_as(&doc, "local[name=demo-local-note].type"),
                "fragment"
            );
            let doc = remote.json();
            assert_eq!(reports_as(&doc, "count"), "1");
            assert_eq!(reports_as(&doc, "remote[name=demo].type"), "bundle");
        }
    }
}

// search.feature: Scenario Outline: Narrowing to bundles searches upstream alone
#[test]
fn test_narrowing_to_bundles_searches_upstream_alone() {
    for flags in FORMATS {
        let mut world = World::initialized();
        world.git_remote("origin");
        notes_with_local_fragment(&world);
        let all = world.run(&["search", "demo"]);
        all.succeeds();
        assert_eq!(reports_as(&all.json(), "count"), "2");

        let narrowed = search(&world, &["--type", "bundle", "demo"], flags);
        if is_text(flags) {
            narrowed.contains("Results (1):").contains("demo");
        } else {
            let doc = narrowed.json();
            assert_eq!(reports_as(&doc, "count"), "1");
            assert_eq!(reports_as(&doc, "remote[name=demo].type"), "bundle");
        }
    }
}

// New design (plan 8.2 `search --remote`, S3): a remote hit names the remote it
// is published by and the reference that installs it (Go's SearchRemoteEntry).
#[test]
fn test_a_remote_search_hit_names_its_remote_and_the_reference_that_installs_it() {
    let mut world = World::initialized();
    world.git_remote("origin");
    let doc = search(&world, &["--remote", "demo"], &[]).json();
    assert_eq!(reports_as(&doc, "remote[name=demo].remote"), "origin");
    assert_eq!(
        reports_as(&doc, "remote[name=demo].pull_ref"),
        world.reference("origin", "demo")
    );
    assert_eq!(entries(&doc, "local"), 0);
}

// bundle.feature: Scenario: Pushing a bundle to a plain git remote lands its tree there
#[test]
fn test_pushing_a_bundle_to_a_plain_git_remote_lands_its_tree_there() {
    let mut world = World::initialized();
    world.project.bundle("authored");
    world.git_remote("team");
    let identity = [
        ("GIT_AUTHOR_NAME", "Alice"),
        ("GIT_AUTHOR_EMAIL", "alice@example.test"),
        ("GIT_COMMITTER_NAME", "Alice"),
        ("GIT_COMMITTER_EMAIL", "alice@example.test"),
    ];
    world
        .run_with(&identity, &["bundle", "push", "authored", "team"])
        .succeeds();
    let held = |bundle: &str, file: &str| {
        world.remote_file("team", &format!("{}/{file}", tree_bundle(bundle)))
    };
    assert!(held("authored", "bundle.yaml").contains("acceptance fixture bundle"));
    assert!(held("authored", "fragments/example.md").contains("# Example Fragment"));
    // The bundle the remote already served is still there: a publish adds to
    // the repository's history rather than replacing it.
    assert!(held("demo", "fragments/demo-frag.md").contains("Demo fragment content."));
}

// bundle.feature: Scenario: Moving a bundle into another project lands it in
// that project's content tree
#[test]
fn test_moving_a_bundle_into_another_project_lands_it_in_that_project_s_content_tree() {
    let world = World::initialized();
    world.project.bundle("demo");
    world.project.write("other-project/.ctxloom/.keep", "");
    let ran = world.run(&["bundle", "move", "demo", "--to", "other-project"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "status"), "moved");
    assert_eq!(
        reports_as(&doc, "dest"),
        "other-project/.ctxloom/content/bundles/v2/demo"
    );
    let landed = |rel: &str| {
        world.file(&format!(
            "other-project/.ctxloom/content/bundles/v2/demo/{rel}"
        ))
    };
    assert!(landed("bundle.yaml")
        .expect("bundle.yaml")
        .contains("acceptance fixture bundle"));
    assert!(landed("fragments/example.md")
        .expect("a fragment")
        .contains("# Example Fragment"));
    assert!(landed("prompts/example.md")
        .expect("a prompt")
        .contains("Example prompt content. Describe what this prompt does."));
    assert!(world
        .file(".ctxloom/content/bundles/v2/demo/bundle.yaml")
        .is_none());
    world.run(&["bundle", "list"]).succeeds().lacks("demo");
}

// bundle.feature: Scenario: A destination that is neither a remote nor a
// directory is refused, and the source stays
#[test]
fn test_a_destination_that_is_neither_a_remote_nor_a_directory_is_refused_and_the_source_stays() {
    let world = World::initialized();
    world.project.bundle("demo");
    world
        .run(&["bundle", "move", "demo", "--to", "nowhere"])
        .fails()
        .contains("neither a configured remote nor an existing directory");
    let source = world
        .file(".ctxloom/content/bundles/v2/demo/fragments/example.md")
        .expect("the source survives");
    assert!(source.contains("# Example Fragment"));
}

// New design (plan 8.2 `bundle move`, S3; Go's MoveBundle ordering): a move to
// a remote publishes first and removes the source only once the remote holds
// it.
#[test]
fn test_moving_a_bundle_to_a_remote_publishes_it_then_removes_the_source() {
    let mut world = World::initialized();
    world.project.bundle("authored");
    world.git_remote("team");
    let identity = [
        ("GIT_AUTHOR_NAME", "Alice"),
        ("GIT_AUTHOR_EMAIL", "alice@example.test"),
        ("GIT_COMMITTER_NAME", "Alice"),
        ("GIT_COMMITTER_EMAIL", "alice@example.test"),
    ];
    let ran = world.run_with(&identity, &["bundle", "move", "authored", "--to", "team"]);
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "dest_kind"), "remote");
    assert!(world
        .remote_file("team", &format!("{}/bundle.yaml", tree_bundle("authored")))
        .contains("acceptance fixture bundle"));
    assert!(world
        .file(".ctxloom/content/bundles/v2/authored/bundle.yaml")
        .is_none());
}

// bundle.feature: Scenario: Holding a local bundle reports it is not
// lockfile-tracked
#[test]
fn test_holding_a_local_bundle_reports_it_is_not_lockfile_tracked() {
    let world = World::initialized();
    world.project.bundle("demo");
    world
        .run(&["deps", "hold", "demo"])
        .succeeds()
        .contains("nothing to hold");
    world.run(&["deps", "unhold", "demo"]).succeeds();
}

// deps.feature (hold): a name that is neither a local bundle nor a locked
// entry is refused, not reported as held (Go: reportNothingToHold).
#[test]
fn test_holding_a_name_that_is_nothing_is_refused() {
    let world = World::initialized();
    world
        .run(&["deps", "hold", "typo"])
        .fails()
        .contains("neither a local bundle nor an entry in the active lockfile");
}

// ---- New-design tests, withheld red: the plan asks for these, the slice does
// not implement them (they are `#[ignore]`d so the gates stay green; each says
// what is missing). They are listed in the slice report.

// Owner ruling, 2026-11-04 (plan 8.2 `deps`): a semver range in a profile
// reference resolves to the newest matching tag, by the semver spec.
#[test]
fn test_a_range_constraint_pulls_the_newest_matching_tag() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.tag_remote("origin", "v1.0.0");
    world.changes_fragment(
        "origin",
        "demo-frag",
        "Demo fragment content, one point one.",
    );
    world.tag_remote("origin", "v1.1.0");
    world.advances("origin");
    world.tag_remote("origin", "v2.0.0");
    let ranged = format!("{}@^1.0", world.reference("origin", "demo"));
    world.write_profile("dev", &[ranged]);
    world.run(&["deps", "pull"]).succeeds();
    let lock = world.lock();
    assert!(lock.contains("v1.1.0"), "{lock}");
    assert!(!lock.contains("v2.0.0"), "{lock}");
    world
        .run(&["deps", "list", "--format", "text"])
        .succeeds()
        .contains("^1.0");
}

// Owner ruling, 2026-11-04: the `deps upgrade` disclosure classifies a pin
// change as breaking, compatible or unknown; it never refuses for a version
// that is not semver.
#[test]
fn test_an_upgrade_across_a_major_is_disclosed_as_breaking() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.tag_remote("origin", "v1.0.0");
    let ranged = format!("{}@>=1.0", world.reference("origin", "demo"));
    world.write_profile("dev", &[ranged]);
    world.run(&["deps", "pull"]).succeeds();
    world.advances("origin");
    world.tag_remote("origin", "v2.0.0");
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .succeeds()
        .contains("v1.0.0 -> v2.0.0")
        .contains("[breaking]");
}

#[test]
fn test_an_upgrade_of_a_branch_with_no_semver_anywhere_is_disclosed_as_unknown() {
    let mut world = World::pulled_dev();
    let files = super::demo_tree("nightly", "Demo bundle", "demo-frag", "Changed.", true);
    world.advance_with("origin", &files);
    world
        .run(&["deps", "upgrade", "--format", "text"])
        .succeeds()
        .contains("[unknown]");
}

// ---- The forge: a GitHub on a local port (forge-github's stub), so no test
// reaches the network. The registry binds the built-in `github` forge and a
// `stub` forge to it, with the token in STUB_TOKEN.

const TOKEN: &str = "stub-token-0123456789";

fn forge_registry(api: &str, remotes: &str) -> String {
    format!(
        "schema_version: 1\nforges:\n  github:\n    type: github\n    api_url: {api}\n    token_env: STUB_TOKEN\n  stub:\n    type: github\n    api_url: {api}\n    token_env: STUB_TOKEN\n{remotes}"
    )
}

// remote.feature `discover`, plan 8.2: `remote discover` uses the GitHub API.
#[test]
fn test_discover_lists_repositories_from_a_forge() {
    let stub = Stub::github(None, "");
    let world = World::initialized();
    world
        .project
        .write(".ctxloom/remotes.yaml", &forge_registry(&stub.url(), ""));
    let ran = world.run(&["remote", "discover", "ctxloom", "--source", "github"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "count"), "2");
    assert_eq!(
        reports_as(&doc, "repositories[name=ctxloom-golang].add_command"),
        "ctxloom remote create ctxloom-golang bob/ctxloom-golang"
    );
    assert_eq!(
        reports_as(&doc, "repositories[name=ctxloom].url"),
        "https://github.com/alice/ctxloom"
    );
    let ask = stub.requests().remove(0);
    assert_eq!(ask.line(), "GET /search/repositories");
    assert_eq!(ask.param("q").as_deref(), Some("ctxloom ctxloom in:name"));
    let text = world.run(&["remote", "discover", "--format", "text"]);
    text.succeeds()
        .contains("bob/ctxloom-golang")
        .contains("alice/ctxloom")
        .lacks("dotfiles");
}

#[test]
fn test_discover_refuses_a_source_that_cannot_be_searched() {
    let world = World::initialized();
    world
        .run(&[
            "remote", "discover", "--source", "gitlab", "--format", "text",
        ])
        .fails()
        .contains("only \"github\" is searchable");
}

// Owner rule (2026-11-04): the token is read from the environment (the forge's
// token_env), is sent to the forge, and is never said back.
#[test]
fn test_the_forge_s_token_is_read_from_the_environment_and_never_said_back() {
    let stub = Stub::github(None, "");
    let world = World::initialized();
    world
        .project
        .write(".ctxloom/remotes.yaml", &forge_registry(&stub.url(), ""));
    let ran = world.run_with(&[("STUB_TOKEN", TOKEN)], &["remote", "discover"]);
    ran.succeeds();
    assert_eq!(
        stub.requests()[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert!(!ran.stdout.contains(TOKEN) && !ran.stderr.contains(TOKEN));
    let bare = world.run(&["remote", "discover"]);
    bare.succeeds();
    assert_eq!(stub.requests()[1].header("authorization"), None);
}

#[test]
fn test_a_search_the_forge_refused_fails_with_the_reason_and_not_the_token() {
    let stub = Stub::replying(403, r#"{"message":"API rate limit exceeded"}"#);
    let world = World::initialized();
    world
        .project
        .write(".ctxloom/remotes.yaml", &forge_registry(&stub.url(), ""));
    let ran = world.run_with(&[("STUB_TOKEN", TOKEN)], &["remote", "discover"]);
    ran.fails().contains("API rate limit exceeded");
    assert!(!ran.stdout.contains(TOKEN) && !ran.stderr.contains(TOKEN));
}

// bundle.feature `push --pr`, plan 8.2: `bundle push --pr` via Forge.
#[test]
fn test_push_with_pr_opens_a_pull_request() {
    let stub = Stub::github(Some(TOKEN), "");
    let world = World::initialized();
    world.project.write(
        ".ctxloom/remotes.yaml",
        &forge_registry(
            &stub.url(),
            "remotes:\n  hub:\n    url: https://github.com/acme/hub\n    forge: stub\ndefault: hub\n",
        ),
    );
    world.project.bundle("notes");
    let ran = world.run_with(
        &[("STUB_TOKEN", TOKEN)],
        &["bundle", "push", "notes", "hub", "--pr", "--format", "text"],
    );
    ran.succeeds()
        .contains("Created pull request: https://github.com/acme/hub/pull/7");
    assert!(!ran.stdout.contains(TOKEN) && !ran.stderr.contains(TOKEN));
    let lines = stub.lines();
    assert_eq!(
        lines.first().map(String::as_str),
        Some("GET /repos/acme/hub")
    );
    assert_eq!(
        lines.last().map(String::as_str),
        Some("POST /repos/acme/hub/pulls")
    );
    let pull = stub.requests().pop().expect("the pull").json();
    assert_eq!(pull["title"], "Update bundle: notes");
    assert_eq!(pull["base"], "main");
    assert!(
        pull["head"]
            .as_str()
            .is_some_and(|head| head.starts_with("ctxloom/bundle/notes-")),
        "{pull}"
    );
    let blobs = stub
        .requests()
        .iter()
        .filter(|r| r.path.ends_with("/git/blobs"))
        .count();
    assert_eq!(blobs, 3, "bundle.yaml, a fragment and a prompt");
    let json = world
        .run_with(
            &[("STUB_TOKEN", TOKEN)],
            &["bundle", "push", "notes", "hub", "--pr"],
        )
        .json();
    assert_eq!(reports_as(&json, "status"), "pr-created");
    assert_eq!(
        reports_as(&json, "pr_url"),
        "https://github.com/acme/hub/pull/7"
    );
}

// publish.go pullRequestSupport: plain git has no API to open one through.
#[test]
fn test_push_with_pr_to_a_plain_git_remote_is_refused_and_says_how() {
    let mut world = World::initialized();
    world.git_remote("origin");
    world.project.bundle("notes");
    world
        .run(&[
            "bundle", "push", "notes", "origin", "--pr", "--format", "text",
        ])
        .fails()
        .contains("no API to open a pull request through")
        .contains("ctxloom remote edit origin --forge github");
}

#[test]
fn test_push_with_pr_without_a_token_says_which_variable_to_set_and_asks_nothing() {
    let stub = Stub::github(Some(TOKEN), "");
    let world = World::initialized();
    world.project.write(
        ".ctxloom/remotes.yaml",
        &forge_registry(
            &stub.url(),
            "remotes:\n  hub:\n    url: https://github.com/acme/hub\n    forge: stub\n",
        ),
    );
    world.project.bundle("notes");
    world
        .run(&["bundle", "push", "notes", "hub", "--pr"])
        .fails()
        .contains("set STUB_TOKEN to open a pull request");
    assert!(stub.requests().is_empty());
}
