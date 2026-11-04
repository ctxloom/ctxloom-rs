//! `remote discover` over the fakes.

use deps_ports::{Endpoint, ForgeRepo};

use crate::fakes::Fixture;
use crate::{AppError, DiscoverRequest, Notice};

fn repo(owner: &str, name: &str, stars: u64) -> ForgeRepo {
    ForgeRepo {
        owner: owner.to_string(),
        name: name.to_string(),
        description: format!("{owner}'s context"),
        stars,
        url: format!("https://github.com/{owner}/{name}"),
    }
}

fn found() -> Fixture {
    let fx = Fixture::new();
    fx.forge.serves(vec![
        repo("bob", "ctxloom-golang", 10),
        repo("alice", "ctxloom", 3),
    ]);
    fx
}

fn public() -> Endpoint {
    Endpoint {
        api_url: "https://api.github.com".to_string(),
        token_vars: vec!["GH_TOKEN".to_string(), "GITHUB_TOKEN".to_string()],
    }
}

// operations.DiscoverRemotes: the repositories, their count, and for each the
// command that registers it (named by the repository, not its owner).
#[test]
fn test_discover_lists_the_repositories_a_forge_finds_and_how_to_register_each() {
    let fx = found();
    let outcome = fx.deps.remote_discover(&DiscoverRequest::default());
    assert!(outcome.notices.is_empty());
    let found = outcome.result.expect("a search");
    assert_eq!(found.count, 2);
    assert!(found.errors.is_empty());
    let first = &found.repositories[0];
    assert_eq!(
        (first.owner.as_str(), first.name.as_str(), first.stars),
        ("bob", "ctxloom-golang", 10)
    );
    assert_eq!(first.description, "bob's context");
    assert_eq!(first.url, "https://github.com/bob/ctxloom-golang");
    assert_eq!(first.forge, "github");
    assert_eq!(
        first.add_command,
        "ctxloom remote create ctxloom-golang bob/ctxloom-golang"
    );
    assert_eq!(
        found.repositories[1].add_command,
        "ctxloom remote create ctxloom alice/ctxloom"
    );
}

#[test]
fn test_discover_asks_the_public_github_for_thirty_by_default() {
    let fx = found();
    fx.deps
        .remote_discover(&DiscoverRequest::default())
        .result
        .expect("a search");
    assert_eq!(fx.forge.searches(), vec![(public(), String::new(), 30)]);
}

#[test]
fn test_discover_passes_the_query_and_the_limit_on() {
    let fx = found();
    fx.deps
        .remote_discover(&DiscoverRequest {
            query: "golang tools".to_string(),
            limit: 5,
            source: "github".to_string(),
            ..DiscoverRequest::default()
        })
        .result
        .expect("a search");
    assert_eq!(
        fx.forge.searches(),
        vec![(public(), "golang tools".to_string(), 5)]
    );
}

// operations.DiscoverRemotes: r.Stars >= req.MinStars.
#[test]
fn test_discover_keeps_the_repositories_with_at_least_the_stars_asked() {
    let fx = found();
    let at_ten = fx
        .deps
        .remote_discover(&DiscoverRequest {
            min_stars: 10,
            ..DiscoverRequest::default()
        })
        .result
        .expect("a search");
    assert_eq!(at_ten.count, 1);
    assert_eq!(at_ten.repositories[0].name, "ctxloom-golang");
    let at_three = fx
        .deps
        .remote_discover(&DiscoverRequest {
            min_stars: 3,
            ..DiscoverRequest::default()
        })
        .result
        .expect("a search");
    assert_eq!(at_three.count, 2);
    let none = fx
        .deps
        .remote_discover(&DiscoverRequest {
            min_stars: 11,
            ..DiscoverRequest::default()
        })
        .result
        .expect("a search");
    assert_eq!((none.count, none.repositories.len()), (0, 0));
}

// operations.DiscoverRemotes: GitHub is the only searchable source.
#[test]
fn test_discover_searches_only_github_and_refuses_any_other_source() {
    let fx = found();
    for source in ["all", "github", ""] {
        fx.deps
            .remote_discover(&DiscoverRequest {
                source: source.to_string(),
                ..DiscoverRequest::default()
            })
            .result
            .unwrap_or_else(|e| panic!("{source}: {e:?}"));
    }
    let searched = fx.forge.searches().len();
    let refused = fx
        .deps
        .remote_discover(&DiscoverRequest {
            source: "gitlab".to_string(),
            ..DiscoverRequest::default()
        })
        .result
        .expect_err("refused");
    match &refused {
        AppError::DiscoverSource { source } => assert_eq!(source, "gitlab"),
        other => panic!("{other:?}"),
    }
    assert_eq!(refused.exit_code(), 2);
    assert_eq!(fx.forge.searches().len(), searched, "nothing was searched");
}

// forge.go: a configured github forge replaces the built-in one, so an
// Enterprise api_url and token_env are what discover reaches.
#[test]
fn test_discover_reaches_the_github_forge_the_registry_configures() {
    let fx = found();
    fx.files.set_registry(
        "schema_version: 1\nforges:\n  github:\n    type: github\n    api_url: https://ghe.corp.test/api/v3\n    token_env: CORP_TOKEN\n",
    );
    fx.deps
        .remote_discover(&DiscoverRequest::default())
        .result
        .expect("a search");
    let (endpoint, _, _) = fx.forge.searches().remove(0);
    assert_eq!(endpoint.api_url, "https://ghe.corp.test/api/v3");
    assert_eq!(endpoint.token_vars, vec!["CORP_TOKEN".to_string()]);
}

// remote_discover.go: a search that failed is no result, and is not an empty
// one; the warning says why.
#[test]
fn test_a_search_that_failed_is_an_error_with_the_reason_as_a_warning() {
    let fx = found();
    fx.forge
        .fail_search("search failed: HTTP 403: rate limit exceeded");
    let outcome = fx.deps.remote_discover(&DiscoverRequest::default());
    assert_eq!(
        outcome.notices,
        vec![Notice::text(
            "github: search failed: HTTP 403: rate limit exceeded"
        )]
    );
    let refused = outcome.result.expect_err("failed");
    assert!(matches!(refused, AppError::DiscoverFailed), "{refused:?}");
    assert_eq!(refused.exit_code(), 1);
}

#[test]
fn test_a_search_that_finds_nothing_is_an_empty_result_not_an_error() {
    let fx = Fixture::new();
    let outcome = fx.deps.remote_discover(&DiscoverRequest::default());
    assert!(outcome.notices.is_empty());
    let found = outcome.result.expect("an empty result");
    assert_eq!((found.count, found.repositories.len()), (0, 0));
}

// A build that wired no forge says so rather than finding nothing.
#[test]
fn test_a_build_with_no_forge_adapter_says_so() {
    let fx = Fixture::new();
    let bare = crate::Deps::new(fx.remotes.clone(), fx.files.clone(), fx.content.clone());
    let outcome = bare.remote_discover(&DiscoverRequest::default());
    assert!(matches!(outcome.result, Err(AppError::DiscoverFailed)));
    assert_eq!(
        outcome.notices,
        vec![Notice::text("github: this build has no forge adapter")]
    );
}
