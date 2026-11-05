//! The tests that reach the real GitHub. Never part of a gate: they are
//! `#[ignore]`d, and even asked for (`cargo nextest run --run-ignored all`)
//! they do nothing unless `CTXLOOM_LIVE_GITHUB=1` is set. They only read.

use deps_ports::{Endpoint, Forge};

use super::GitHub;

fn live() -> bool {
    process().utf8("CTXLOOM_LIVE_GITHUB") == Some("1")
}

fn process() -> process_env::EnvSnapshot {
    process_env::EnvSnapshot::from_process()
}

fn github() -> GitHub {
    let env = process();
    GitHub::with_env(move |name| env.utf8(name).map(String::from))
}

fn public() -> Endpoint {
    Endpoint {
        api_url: "https://api.github.com".to_string(),
        token_vars: vec!["GH_TOKEN".to_string(), "GITHUB_TOKEN".to_string()],
    }
}

#[test]
#[ignore = "network: the real GitHub, with CTXLOOM_LIVE_GITHUB=1"]
fn test_live_a_search_of_the_public_github_answers_ctxloom_repositories() {
    if !live() {
        return;
    }
    let repos = github()
        .search_repositories(&public(), "", 5)
        .expect("a search of the public GitHub");
    for repo in &repos {
        assert!(
            repo.name == "ctxloom" || repo.name.starts_with("ctxloom-"),
            "{repo:?}"
        );
        assert!(repo.url.starts_with("https://github.com/"), "{repo:?}");
    }
}

#[test]
#[ignore = "network: the real GitHub, with CTXLOOM_LIVE_GITHUB=1"]
fn test_live_a_pull_request_without_a_token_is_refused_before_any_request() {
    if !live() {
        return;
    }
    let github = GitHub::with_env(|_| None);
    let request = deps_ports::PullRequest {
        owner: "ctxloom".to_string(),
        repo: "ctxloom".to_string(),
        branch: "ctxloom/bundle/live".to_string(),
        message: "m".to_string(),
        title: "t".to_string(),
        body: "b".to_string(),
        files: std::collections::BTreeMap::from([("a".to_string(), b"a".to_vec())]),
    };
    let refused = github
        .open_pull_request(&public(), &request)
        .expect_err("no token");
    assert!(refused.contains("GH_TOKEN"), "{refused}");
}
