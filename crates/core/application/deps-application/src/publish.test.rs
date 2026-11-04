//! `bundle push` and `bundle move` over the fakes.

use deps_ports::Endpoint;

use crate::fakes::Fixture;
use crate::{AppError, MoveRequest, Outcome, PushRequest};

const URL: &str = "file:///srv/team.git";

fn ok<T>(outcome: Outcome<T>) -> T {
    outcome.result.unwrap_or_else(|e| panic!("{e:?}"))
}

fn authored() -> Fixture {
    let fx = Fixture::new();
    fx.files.author(
        "authored",
        &[
            ("bundle.yaml", "description: acceptance fixture bundle\n"),
            ("fragments/example.md", "# Example Fragment\n"),
        ],
    );
    fx
}

fn push(bundle: &str, remote: &str) -> PushRequest {
    PushRequest {
        bundle: bundle.to_string(),
        remote: remote.to_string(),
        ..PushRequest::default()
    }
}

// bundle.feature: Pushing a bundle to a plain git remote lands its tree there
#[test]
fn test_a_push_lands_the_whole_tree_as_one_commit_beside_what_was_there() {
    let fx = authored();
    let before = fx.origin("team", URL);
    let pushed = ok(fx.deps.push(&push("authored", "team")));
    assert_eq!(pushed.status, "pushed");
    assert_eq!(pushed.remote, "team");
    assert_eq!(pushed.target_path, ".ctxloom/content/bundles/v2/authored");
    assert_eq!(pushed.title, "Update bundle: authored");
    assert!(!pushed.create_pr);
    assert_eq!(pushed.size_bytes, 58);
    assert_eq!(
        pushed.path,
        "/p/.ctxloom/content/bundles/v2/authored/bundle.yaml"
    );
    assert_ne!(pushed.commit_sha, before);
    assert_eq!(pushed.commit_sha, fx.remotes.head(URL));
    let held = |path: &str| {
        fx.remotes
            .file(URL, &format!(".ctxloom/content/bundles/v2/{path}"))
    };
    assert_eq!(
        held("authored/bundle.yaml").as_deref(),
        Some("description: acceptance fixture bundle\n")
    );
    assert_eq!(
        held("authored/fragments/example.md").as_deref(),
        Some("# Example Fragment\n")
    );
    assert!(
        held("demo/fragments/demo-frag.md").is_some(),
        "a publish adds to the history"
    );
    assert!(fx.files.has_authored("authored"), "a push keeps the source");
}

#[test]
fn test_a_push_with_a_message_and_to_the_default_remote() {
    let fx = authored();
    fx.origin("team", URL);
    ok(fx.deps.remote_default(Some("team")));
    let pushed = ok(fx.deps.push(&PushRequest {
        message: "ship it".to_string(),
        ..push("authored", "")
    }));
    assert_eq!(
        (pushed.title.as_str(), pushed.remote.as_str()),
        ("ship it", "team")
    );
}

#[test]
fn test_a_push_that_changes_nothing_reports_the_commit_already_at_the_tip() {
    let fx = authored();
    fx.origin("team", URL);
    let first = ok(fx.deps.push(&push("authored", "team")));
    let second = ok(fx.deps.push(&push("authored", "team")));
    assert_eq!(first.commit_sha, second.commit_sha);
}

#[test]
fn test_a_push_that_cannot_go_is_refused_naming_why() {
    let fx = authored();
    fx.origin("team", URL);
    let refuses = |request: PushRequest| fx.deps.push(&request).result.expect_err("refused");
    assert!(matches!(
        refuses(PushRequest {
            pr: true,
            ..push("authored", "team")
        }),
        AppError::PullRequestNeedsForge { .. }
    ));
    assert!(matches!(
        refuses(push("authored", "")),
        AppError::Required { .. }
    ));
    assert!(matches!(
        refuses(push("authored", "nope")),
        AppError::Cause(_)
    ));
    assert!(matches!(
        refuses(push("missing", "team")),
        AppError::BundleNotFound { .. }
    ));
    fx.remotes.set_unreachable(URL, true);
    match refuses(push("authored", "team")) {
        AppError::Publish { remote, detail } => {
            assert_eq!(remote, "team");
            assert!(detail.contains("could not read"));
        }
        other => panic!("{other:?}"),
    }
}

// bundle.feature: Moving a bundle into another project lands it in that
// project's content tree
#[test]
fn test_a_move_to_a_directory_lands_the_bundle_there_and_removes_the_source() {
    let fx = authored();
    fx.files.directory("other-project");
    let moved = ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "other-project".to_string(),
        ..MoveRequest::default()
    }));
    assert_eq!((moved.status, moved.dest_kind), ("moved", "path"));
    assert_eq!(
        moved.dest,
        "other-project/.ctxloom/content/bundles/v2/authored"
    );
    assert_eq!(
        moved.source,
        "/p/.ctxloom/content/bundles/v2/authored/bundle.yaml"
    );
    assert_eq!((moved.remote.as_str(), moved.commit_sha.as_str()), ("", ""));
    let placed = fx.files.placed(&moved.dest).expect("placed");
    assert_eq!(placed["fragments/example.md"], b"# Example Fragment\n");
    assert!(!fx.files.has_authored("authored"));
}

#[test]
fn test_a_move_to_a_remote_publishes_first_then_removes_the_source() {
    let fx = authored();
    fx.origin("team", URL);
    let moved = ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "team".to_string(),
        message: "relocate".to_string(),
        ..MoveRequest::default()
    }));
    assert_eq!(moved.dest_kind, "remote");
    assert_eq!(moved.remote, "team");
    assert_eq!(moved.dest, ".ctxloom/content/bundles/v2/authored");
    assert_eq!(moved.commit_sha, fx.remotes.head(URL));
    assert!(!fx.files.has_authored("authored"));
}

// bundle.feature: A destination that is neither a remote nor a directory is
// refused, and the source stays
#[test]
fn test_a_move_that_cannot_land_leaves_the_source_alone() {
    let fx = authored();
    let moves = |to: &str, force: bool| {
        fx.deps
            .move_bundle(&MoveRequest {
                name: "authored".to_string(),
                to: to.to_string(),
                force,
                ..MoveRequest::default()
            })
            .result
            .expect_err("refused")
    };
    match moves("nowhere", false) {
        AppError::BadDestination { to, known } => {
            assert_eq!(
                (to.as_str(), known.as_str()),
                ("nowhere", " (no remotes configured)")
            );
        }
        other => panic!("{other:?}"),
    }
    fx.origin("team", URL);
    fx.origin("other", "file:///srv/other.git");
    match moves("nowhere", false) {
        AppError::BadDestination { known, .. } => {
            assert_eq!(known, " (configured remotes: other, team)")
        }
        other => panic!("{other:?}"),
    }
    fx.files.directory("elsewhere");
    ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "elsewhere".to_string(),
        ..MoveRequest::default()
    }));
    // The bundle is gone from here; a second one of the same name is not.
    fx.files.author("authored", &[("bundle.yaml", "v: 2\n")]);
    assert!(matches!(
        moves("elsewhere", false),
        AppError::DestinationExists { .. }
    ));
    ok(fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "elsewhere".to_string(),
        force: true,
        ..MoveRequest::default()
    }));
    fx.files.author("authored", &[("bundle.yaml", "v: 3\n")]);
    fx.files.directory(".");
    assert!(matches!(
        moves(".", false),
        AppError::DestinationItself { .. }
    ));
    assert!(fx.files.has_authored("authored"));
    assert!(matches!(
        fx.deps
            .move_bundle(&MoveRequest {
                name: "missing".to_string(),
                to: "elsewhere".to_string(),
                ..MoveRequest::default()
            })
            .result
            .expect_err("refused"),
        AppError::BundleNotFound { .. }
    ));
    fx.remotes.set_unreachable(URL, true);
    assert!(matches!(moves("team", false), AppError::Publish { .. }));
    assert!(
        fx.files.has_authored("authored"),
        "a publish that failed took nothing"
    );
}

const HUB: &str = "https://github.com/acme/hub";
const TREE: &str = ".ctxloom/content/bundles/v2/authored";

fn on_github() -> Fixture {
    let fx = authored();
    fx.register_on("hub", HUB, "github");
    fx
}

fn pr(bundle: &str, remote: &str) -> PushRequest {
    PushRequest {
        pr: true,
        ..push(bundle, remote)
    }
}

// Plan 8.2 `bundle push --pr` via Forge: the tree is one commit on a new
// branch of the remote's repository, and a pull request from it into the
// default branch; nothing is pushed to the default branch.
#[test]
fn test_push_with_pr_opens_a_pull_request_through_the_remote_s_forge() {
    let fx = on_github();
    let pushed = ok(fx.deps.push(&pr("authored", "hub")));
    assert_eq!(pushed.status, "pr-created");
    assert!(pushed.create_pr);
    assert_eq!(pushed.pr_url, "https://github.com/acme/hub/pull/7");
    assert_eq!(pushed.commit_sha, "c0ffee".repeat(6) + "c0ff");
    assert_eq!(pushed.remote, "hub");
    assert_eq!(pushed.target_path, TREE);
    assert_eq!(pushed.title, "Update bundle: authored");
    assert_eq!(pushed.size_bytes, 58);
    let opened = fx.forge.opened();
    assert_eq!(opened.len(), 1);
    let (endpoint, request) = &opened[0];
    assert_eq!(
        endpoint,
        &Endpoint {
            api_url: "https://api.github.com".to_string(),
            token_vars: vec!["GH_TOKEN".to_string(), "GITHUB_TOKEN".to_string()],
        }
    );
    assert_eq!(
        (request.owner.as_str(), request.repo.as_str()),
        ("acme", "hub")
    );
    assert_eq!(request.branch, "ctxloom/bundle/authored");
    assert_eq!(request.title, "Update bundle: authored");
    assert_eq!(request.message, "Update bundle: authored");
    assert_eq!(
        request.body,
        format!("Publishing bundle `authored` from local repository.\n\nPath: `{TREE}`")
    );
    let paths: Vec<&str> = request.files.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec![
            format!("{TREE}/bundle.yaml"),
            format!("{TREE}/fragments/example.md")
        ]
    );
    assert_eq!(
        request.files[&format!("{TREE}/fragments/example.md")],
        b"# Example Fragment\n"
    );
}

// publish.go SplitTitleBody and buildPRBody: the message's first line is the
// title, the rest the body, above the footer.
#[test]
fn test_the_message_s_first_line_is_the_title_and_the_rest_the_body() {
    let fx = on_github();
    let pushed = ok(fx.deps.push(&PushRequest {
        message: "Add notes\n\nWhy it matters.\n".to_string(),
        ..pr("authored", "hub")
    }));
    assert_eq!(pushed.title, "Add notes");
    let (_, request) = fx.forge.opened().remove(0);
    assert_eq!(request.title, "Add notes");
    assert_eq!(request.message, "Add notes\n\nWhy it matters.");
    assert_eq!(
        request.body,
        format!(
            "Why it matters.\n\n---\n\nPublishing bundle `authored` from local repository.\n\nPath: `{TREE}`"
        )
    );
}

// publish.go fitPRTitle: a title over 72 characters is cut at a word and the
// whole of it kept in the body.
#[test]
fn test_a_title_over_seventy_two_characters_is_cut_and_kept_whole_in_the_body() {
    let fx = on_github();
    let long = "Add the notes bundle with its fragments and prompts so that every project of the team can pull it";
    ok(fx.deps.push(&PushRequest {
        message: long.to_string(),
        ..pr("authored", "hub")
    }));
    let (_, request) = fx.forge.opened().remove(0);
    assert_eq!(
        request.title,
        "Add the notes bundle with its fragments and prompts so that every\u{2026}"
    );
    assert!(request.title.chars().count() <= 73);
    assert!(
        request
            .body
            .starts_with(&format!("**Full title:** {long}\n\n---\n\n")),
        "{}",
        request.body
    );
    assert_eq!(request.message, long, "the commit keeps the whole subject");
}

#[test]
fn test_a_title_of_exactly_seventy_two_characters_is_not_cut() {
    let fx = on_github();
    let exact = "x".repeat(72);
    ok(fx.deps.push(&PushRequest {
        message: exact.clone(),
        ..pr("authored", "hub")
    }));
    let (_, request) = fx.forge.opened().remove(0);
    assert_eq!(request.title, exact);
    assert!(!request.body.contains("Full title"));
}

// forge.go: a configured forge's API and token_env are honoured, and the
// ambient GitHub token is not offered to another host.
#[test]
fn test_a_configured_forge_s_api_and_token_variable_are_the_ones_used() {
    let fx = authored();
    fx.files.set_registry(
        "schema_version: 1\nforges:\n  corp:\n    type: github\n    api_url: https://ghe.corp.test/api/v3\n    token_env: CORP_TOKEN\nremotes:\n  hub:\n    url: https://ghe.corp.test/acme/hub\n    forge: corp\n",
    );
    ok(fx.deps.push(&pr("authored", "hub")));
    let (endpoint, request) = fx.forge.opened().remove(0);
    assert_eq!(endpoint.api_url, "https://ghe.corp.test/api/v3");
    assert_eq!(endpoint.token_vars, vec!["CORP_TOKEN".to_string()]);
    assert_eq!(
        (request.owner.as_str(), request.repo.as_str()),
        ("acme", "hub")
    );
}

// publish.go pullRequestSupport: plain git has no API for it.
#[test]
fn test_push_with_pr_to_a_plain_git_remote_is_refused_and_opens_nothing() {
    let fx = authored();
    fx.origin("team", URL);
    let head = fx.remotes.head(URL);
    let refused = fx
        .deps
        .push(&pr("authored", "team"))
        .result
        .expect_err("refused");
    match &refused {
        AppError::PullRequestNeedsForge { remote, forge } => {
            assert_eq!((remote.as_str(), forge.as_str()), ("team", "git"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(refused.exit_code(), 2);
    assert!(fx.forge.opened().is_empty());
    assert_eq!(fx.remotes.head(URL), head, "nothing was pushed either");
}

#[test]
fn test_push_with_pr_to_an_address_with_no_owner_and_repository_is_refused() {
    let fx = authored();
    fx.remotes.serve(URL);
    fx.register_on("team", URL, "github");
    match fx
        .deps
        .push(&pr("authored", "team"))
        .result
        .expect_err("refused")
    {
        AppError::Publish { remote, detail } => {
            assert_eq!(remote, "team");
            assert!(detail.contains("owner") && detail.contains(URL), "{detail}");
        }
        other => panic!("{other:?}"),
    }
    assert!(fx.forge.opened().is_empty());
}

#[test]
fn test_a_forge_that_fails_is_a_publish_that_failed_naming_why() {
    let fx = on_github();
    fx.forge
        .fail_pull_requests("create blob for x: HTTP 401: Bad credentials");
    match fx
        .deps
        .push(&pr("authored", "hub"))
        .result
        .expect_err("failed")
    {
        AppError::Publish { remote, detail } => {
            assert_eq!(remote, "hub");
            assert_eq!(detail, "create blob for x: HTTP 401: Bad credentials");
        }
        other => panic!("{other:?}"),
    }
}

// publish.go refuseEmptyPublish: a 0-byte file would replace the remote's
// with nothing, by a pull request or by a push.
#[test]
fn test_a_file_with_nothing_in_it_is_refused_before_anything_is_published() {
    let fx = on_github();
    fx.files.author(
        "hollow",
        &[
            ("bundle.yaml", "description: d\n"),
            ("fragments/empty.md", ""),
        ],
    );
    fx.origin("team", URL);
    let head = fx.remotes.head(URL);
    for request in [pr("hollow", "hub"), push("hollow", "team")] {
        match fx.deps.push(&request).result.expect_err("refused") {
            AppError::EmptyFile { path } => assert_eq!(path, "fragments/empty.md"),
            other => panic!("{other:?}"),
        }
    }
    assert!(fx.forge.opened().is_empty());
    assert_eq!(fx.remotes.head(URL), head);
}

// `bundle move` to a remote publishes by git, as it did.
#[test]
fn test_a_move_to_a_github_remote_still_publishes_by_git_not_a_pull_request() {
    let fx = on_github();
    let moved = fx.deps.move_bundle(&MoveRequest {
        name: "authored".to_string(),
        to: "hub".to_string(),
        ..MoveRequest::default()
    });
    assert!(
        moved.result.is_err(),
        "no git repository serves the address"
    );
    assert!(fx.forge.opened().is_empty());
}
