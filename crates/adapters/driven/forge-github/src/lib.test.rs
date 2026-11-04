//! The adapter against a GitHub on a local port (`stub`): the requests it
//! makes, in order, and what it makes of the answers. No network.

use std::collections::BTreeMap;

use serde_json::json;

use super::stub::{Reply, Stub, BASE_SHA, BASE_TREE, NEW_COMMIT, NEW_TREE};
use super::*;

const TOKEN: &str = "s3cret-token-value";

fn env(pairs: &[(&'static str, &'static str)]) -> GitHub {
    let pairs: BTreeMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    GitHub::with_env(move |name| pairs.get(name).cloned()).without_proxy()
}

fn authed() -> GitHub {
    env(&[("GH_TOKEN", TOKEN)])
}

fn endpoint(stub: &Stub) -> Endpoint {
    Endpoint {
        api_url: stub.url(),
        token_vars: vec!["GH_TOKEN".to_string(), "GITHUB_TOKEN".to_string()],
    }
}

fn request(files: &[(&str, &str)]) -> PullRequest {
    PullRequest {
        owner: "acme".to_string(),
        repo: "hub".to_string(),
        branch: "ctxloom/bundle/notes".to_string(),
        message: "Add notes\n\nWhy.".to_string(),
        title: "Add notes".to_string(),
        body: "Why.".to_string(),
        files: files
            .iter()
            .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
            .collect(),
    }
}

fn notes() -> PullRequest {
    request(&[
        ("b/two.md", "# Example Fragment\n"),
        ("a/one.yaml", "description: acceptance fixture bundle\n"),
    ])
}

// ----------------------------------------------------------------- search

// github.go SearchRepos: stars first, GitHub's own search of repositories
// named for ctxloom, and only those that are.
#[test]
fn test_a_search_asks_for_repositories_named_ctxloom_most_starred_first() {
    let stub = Stub::github(None, "");
    let found = env(&[])
        .search_repositories(&endpoint(&stub), "", 30)
        .expect("a search");
    let requests = stub.requests();
    assert_eq!(requests.len(), 1);
    let ask = &requests[0];
    assert_eq!(ask.line(), "GET /search/repositories");
    assert_eq!(ask.param("q").as_deref(), Some("ctxloom in:name"));
    assert_eq!(ask.param("sort").as_deref(), Some("stars"));
    assert_eq!(ask.param("order").as_deref(), Some("desc"));
    assert_eq!(ask.param("per_page").as_deref(), Some("30"));
    assert_eq!(
        found,
        vec![
            ForgeRepo {
                owner: "bob".to_string(),
                name: "ctxloom-golang".to_string(),
                description: "bob's context".to_string(),
                stars: 10,
                url: "https://github.com/bob/ctxloom-golang".to_string(),
            },
            ForgeRepo {
                owner: "alice".to_string(),
                name: "ctxloom".to_string(),
                description: String::new(),
                stars: 3,
                url: "https://github.com/alice/ctxloom".to_string(),
            },
        ],
        "dotfiles and ctxloomer are not ctxloom or ctxloom-*"
    );
}

#[test]
fn test_a_search_puts_the_users_words_before_the_name_qualifier_and_the_limit_in_per_page() {
    let stub = Stub::github(None, "");
    env(&[])
        .search_repositories(&endpoint(&stub), "golang tools", 7)
        .expect("a search");
    let ask = stub.requests().remove(0);
    assert_eq!(
        ask.param("q").as_deref(),
        Some("golang tools ctxloom in:name")
    );
    assert_eq!(ask.param("per_page").as_deref(), Some("7"));
}

// github.go: the client says what it is, and speaks GitHub's JSON.
#[test]
fn test_a_request_names_the_client_and_the_media_type_and_api_version() {
    let stub = Stub::github(None, "");
    env(&[])
        .search_repositories(&endpoint(&stub), "", 1)
        .expect("a search");
    let ask = stub.requests().remove(0);
    assert_eq!(ask.header("user-agent"), Some("ctxloom"));
    assert_eq!(ask.header("accept"), Some("application/vnd.github+json"));
    assert_eq!(ask.header("x-github-api-version"), Some("2022-11-28"));
}

// The API of an Enterprise is under a path of its own.
#[test]
fn test_an_api_url_with_a_path_is_honoured_and_a_trailing_slash_does_not_double() {
    let stub = Stub::github(None, "/api/v3");
    for suffix in ["/api/v3", "/api/v3/"] {
        let endpoint = Endpoint {
            api_url: format!("{}{suffix}", stub.url()),
            token_vars: Vec::new(),
        };
        env(&[])
            .search_repositories(&endpoint, "", 1)
            .unwrap_or_else(|e| panic!("{suffix}: {e}"));
    }
    assert_eq!(stub.lines(), vec!["GET /api/v3/search/repositories"; 2]);
}

// ------------------------------------------------------------------ token

// Owner rule: GH_TOKEN wins over GITHUB_TOKEN; the token goes as a bearer.
#[test]
fn test_gh_token_wins_over_github_token_and_goes_as_a_bearer() {
    let stub = Stub::github(None, "");
    env(&[("GITHUB_TOKEN", "from-github"), ("GH_TOKEN", "from-gh")])
        .search_repositories(&endpoint(&stub), "", 1)
        .expect("a search");
    assert_eq!(
        stub.requests()[0].header("authorization"),
        Some("Bearer from-gh")
    );
    let stub = Stub::github(None, "");
    env(&[("GITHUB_TOKEN", "from-github")])
        .search_repositories(&endpoint(&stub), "", 1)
        .expect("a search");
    assert_eq!(
        stub.requests()[0].header("authorization"),
        Some("Bearer from-github")
    );
}

#[test]
fn test_a_forge_s_own_variable_comes_first_and_an_empty_one_is_not_a_token() {
    let stub = Stub::github(None, "");
    let own = Endpoint {
        token_vars: vec!["CORP_TOKEN".to_string(), "GH_TOKEN".to_string()],
        ..endpoint(&stub)
    };
    env(&[("CORP_TOKEN", "corp"), ("GH_TOKEN", "gh")])
        .search_repositories(&own, "", 1)
        .expect("a search");
    env(&[("CORP_TOKEN", ""), ("GH_TOKEN", "gh")])
        .search_repositories(&own, "", 1)
        .expect("a search");
    let sent: Vec<_> = stub
        .requests()
        .iter()
        .map(|r| r.header("authorization").map(str::to_string))
        .collect();
    assert_eq!(
        sent,
        vec![
            Some("Bearer corp".to_string()),
            Some("Bearer gh".to_string())
        ]
    );
}

#[test]
fn test_a_variable_the_endpoint_does_not_name_is_never_read() {
    let stub = Stub::github(None, "");
    let corp = Endpoint {
        token_vars: vec!["CORP_TOKEN".to_string()],
        ..endpoint(&stub)
    };
    env(&[("GH_TOKEN", "github.com's token"), ("GITHUB_TOKEN", "too")])
        .search_repositories(&corp, "", 1)
        .expect("a search");
    assert_eq!(stub.requests()[0].header("authorization"), None);
}

// A search is public: with no token it goes unauthenticated.
#[test]
fn test_a_search_with_no_token_goes_unauthenticated() {
    let stub = Stub::github(None, "");
    env(&[])
        .search_repositories(&endpoint(&stub), "", 1)
        .expect("a search");
    assert_eq!(stub.requests()[0].header("authorization"), None);
}

// Owner rule: never in an error, a log or Debug.
#[test]
fn test_the_token_is_not_in_debug_output() {
    let github = authed();
    let shown = format!("{github:?} {:?}", endpoint(&Stub::github(None, "")));
    assert!(!shown.contains(TOKEN), "{shown}");
    assert!(shown.contains("GitHub"), "{shown}");
}

#[test]
fn test_a_token_that_is_not_a_header_value_is_refused_by_variable_name_not_value() {
    let stub = Stub::github(None, "");
    let refused = env(&[("GH_TOKEN", "bad\ntoken-value")])
        .search_repositories(&endpoint(&stub), "", 1)
        .expect_err("refused");
    assert!(refused.contains("GH_TOKEN"), "{refused}");
    assert!(!refused.contains("token-value"), "{refused}");
    assert!(stub.requests().is_empty());
}

// ----------------------------------------------------------------- errors

// github.go: `search failed: %w`; the API's own words say why.
#[test]
fn test_an_error_answer_is_the_status_and_githubs_own_message() {
    let stub = Stub::replying(
        403,
        r#"{"message":"API rate limit exceeded","documentation_url":"x"}"#,
    );
    let refused = authed()
        .search_repositories(&endpoint(&stub), "", 1)
        .expect_err("refused");
    assert_eq!(refused, "search failed: HTTP 403: API rate limit exceeded");
}

#[test]
fn test_an_error_answer_that_is_not_json_is_the_status_alone() {
    let stub = Stub::replying(502, "<html>Bad gateway</html>");
    let refused = authed()
        .search_repositories(&endpoint(&stub), "", 1)
        .expect_err("refused");
    assert_eq!(refused, "search failed: HTTP 502");
}

#[test]
fn test_a_server_that_is_not_there_is_an_error_and_not_a_panic() {
    let stub = Stub::github(None, "");
    let gone = Endpoint {
        api_url: stub.url(),
        token_vars: Vec::new(),
    };
    drop(stub);
    let refused = env(&[])
        .search_repositories(&gone, "", 1)
        .expect_err("nothing listens");
    assert!(refused.starts_with("search failed: "), "{refused}");
}

#[test]
fn test_an_answer_that_is_not_the_search_shape_is_an_error() {
    let stub = Stub::replying(200, r#"{"unexpected": true}"#);
    let refused = env(&[])
        .search_repositories(&endpoint(&stub), "", 1)
        .expect_err("no items");
    assert!(refused.starts_with("search failed: "), "{refused}");
}

// ------------------------------------------------------------ pull request

// github.go CreateOrUpdateFiles and CreatePullRequest, in Rust's order: every
// object is made before the branch that points at it, so a failure leaves no
// branch behind.
#[test]
fn test_a_pull_request_is_one_commit_of_every_file_on_a_new_branch_and_the_pull_request() {
    let stub = Stub::github(Some(TOKEN), "");
    let opened = authed()
        .open_pull_request(&endpoint(&stub), &notes())
        .expect("opened");
    assert_eq!(
        stub.lines(),
        vec![
            "GET /repos/acme/hub",
            "GET /repos/acme/hub/git/ref/heads/main",
            &format!("GET /repos/acme/hub/git/commits/{BASE_SHA}"),
            "POST /repos/acme/hub/git/blobs",
            "POST /repos/acme/hub/git/blobs",
            "POST /repos/acme/hub/git/trees",
            "POST /repos/acme/hub/git/commits",
            "POST /repos/acme/hub/git/refs",
            "POST /repos/acme/hub/pulls",
        ]
    );
    assert_eq!(opened.url, "https://github.com/acme/hub/pull/7");
    assert_eq!(opened.commit, NEW_COMMIT);
    let digits = opened
        .branch
        .strip_prefix("ctxloom/bundle/notes-")
        .expect("the prefix, a dash");
    assert!(
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        "{}",
        opened.branch
    );
}

#[test]
fn test_every_write_carries_the_token_and_a_json_body() {
    let stub = Stub::github(Some(TOKEN), "");
    authed()
        .open_pull_request(&endpoint(&stub), &notes())
        .expect("opened");
    for ask in stub.requests() {
        assert_eq!(
            ask.header("authorization"),
            Some(format!("Bearer {TOKEN}").as_str()),
            "{}",
            ask.line()
        );
        if ask.method == "POST" {
            assert_eq!(
                ask.header("content-type"),
                Some("application/json"),
                "{}",
                ask.line()
            );
        }
    }
}

// github.go: a blob per file, base64 so that any bytes arrive as they are.
#[test]
fn test_each_file_is_a_base64_blob_in_path_order() {
    let stub = Stub::github(Some(TOKEN), "");
    authed()
        .open_pull_request(&endpoint(&stub), &notes())
        .expect("opened");
    let blobs: Vec<_> = stub
        .requests()
        .into_iter()
        .filter(|r| r.path.ends_with("/git/blobs"))
        .map(|r| r.json())
        .collect();
    assert_eq!(
        blobs,
        vec![
            json!({"content": "ZGVzY3JpcHRpb246IGFjY2VwdGFuY2UgZml4dHVyZSBidW5kbGUK", "encoding": "base64"}),
            json!({"content": "IyBFeGFtcGxlIEZyYWdtZW50Cg==", "encoding": "base64"}),
        ]
    );
}

#[test]
fn test_the_tree_is_every_blob_at_its_path_on_the_base_tree_and_the_commit_has_the_base_as_parent()
{
    let stub = Stub::github(Some(TOKEN), "");
    authed()
        .open_pull_request(&endpoint(&stub), &notes())
        .expect("opened");
    let requests = stub.requests();
    let body_of = |tail: &str| {
        requests
            .iter()
            .find(|r| r.path.ends_with(tail))
            .unwrap_or_else(|| panic!("{tail}"))
            .json()
    };
    assert_eq!(
        body_of("/git/trees"),
        json!({
            "base_tree": BASE_TREE,
            "tree": [
                {"path": "a/one.yaml", "mode": "100644", "type": "blob", "sha": format!("b10b{:036}", 1)},
                {"path": "b/two.md", "mode": "100644", "type": "blob", "sha": format!("b10b{:036}", 2)},
            ]
        })
    );
    assert_eq!(
        body_of("/git/commits"),
        json!({
            "message": "Add notes\n\nWhy.",
            "tree": NEW_TREE,
            "parents": [BASE_SHA],
        })
    );
}

#[test]
fn test_the_branch_points_at_the_commit_and_the_pull_request_asks_for_it_into_the_default_branch() {
    let stub = Stub::github(Some(TOKEN), "");
    let opened = authed()
        .open_pull_request(&endpoint(&stub), &notes())
        .expect("opened");
    let requests = stub.requests();
    let ref_body = requests
        .iter()
        .find(|r| r.path.ends_with("/git/refs"))
        .expect("a ref")
        .json();
    assert_eq!(
        ref_body,
        json!({"ref": format!("refs/heads/{}", opened.branch), "sha": NEW_COMMIT})
    );
    let pull = requests
        .iter()
        .find(|r| r.path.ends_with("/pulls"))
        .expect("a pull")
        .json();
    assert_eq!(
        pull,
        json!({
            "title": "Add notes",
            "body": "Why.",
            "head": opened.branch,
            "base": "main",
        })
    );
}

#[test]
fn test_an_enterprise_api_path_is_the_one_every_step_goes_to() {
    let stub = Stub::github(Some(TOKEN), "/api/v3");
    let endpoint = Endpoint {
        api_url: format!("{}/api/v3", stub.url()),
        token_vars: vec!["GH_TOKEN".to_string()],
    };
    authed()
        .open_pull_request(&endpoint, &notes())
        .expect("opened");
    assert!(stub
        .lines()
        .iter()
        .all(|l| l.contains(" /api/v3/repos/acme/hub")));
    assert_eq!(stub.lines().len(), 9);
}

// A pull request is a write: with no token there is no point in asking.
#[test]
fn test_a_pull_request_with_no_token_is_refused_before_any_request_naming_the_variables() {
    let stub = Stub::github(Some(TOKEN), "");
    let refused = env(&[])
        .open_pull_request(&endpoint(&stub), &notes())
        .expect_err("no token");
    assert_eq!(
        refused,
        "no GitHub token: set GH_TOKEN or GITHUB_TOKEN to open a pull request"
    );
    assert!(stub.requests().is_empty());
}

#[test]
fn test_the_token_a_forge_names_is_the_one_the_refusal_asks_for() {
    let stub = Stub::github(Some(TOKEN), "");
    let corp = Endpoint {
        token_vars: vec!["CORP_TOKEN".to_string()],
        ..endpoint(&stub)
    };
    let refused = env(&[("GH_TOKEN", "x")])
        .open_pull_request(&corp, &notes())
        .expect_err("no token");
    assert_eq!(
        refused,
        "no GitHub token: set CORP_TOKEN to open a pull request"
    );
}

// github.go: a token the API turns down is said as the API says it, and the
// token is not in it.
#[test]
fn test_a_token_the_api_turns_down_is_reported_as_the_api_says_and_not_echoed() {
    let stub = Stub::github(Some("another-token"), "");
    let refused = env(&[("GH_TOKEN", TOKEN)])
        .open_pull_request(&endpoint(&stub), &notes())
        .expect_err("rejected");
    assert!(refused.contains("HTTP 401: Bad credentials"), "{refused}");
    assert!(!refused.contains(TOKEN), "{refused}");
}

// A failure says which step it was at, and what GitHub said.
#[test]
fn test_a_failure_names_the_step_it_was_at() {
    for (at, status, message, expected) in [
        (
            "GET /repos/acme/hub",
            404,
            "Not Found",
            "get repository acme/hub: HTTP 404: Not Found",
        ),
        (
            "/git/ref/heads/main",
            404,
            "Not Found",
            "resolve branch main: HTTP 404: Not Found",
        ),
        (
            "/git/commits/b45e000000000000000000000000000000000000",
            404,
            "Not Found",
            "read base commit b45e000000000000000000000000000000000000: HTTP 404: Not Found",
        ),
        (
            "/git/blobs",
            422,
            "Invalid",
            "create blob for a/one.yaml: HTTP 422: Invalid",
        ),
        (
            "/git/trees",
            422,
            "Bad tree",
            "create tree: HTTP 422: Bad tree",
        ),
        (
            "POST /repos/acme/hub/git/commits",
            409,
            "Conflict",
            "create commit: HTTP 409: Conflict",
        ),
        (
            "/git/refs",
            422,
            "Reference already exists",
            "create branch: HTTP 422: Reference already exists",
        ),
        (
            "/pulls",
            422,
            "No commits between main and head",
            "failed to create pull request: HTTP 422: No commits between main and head",
        ),
    ] {
        let stub = Stub::github_failing(Some(TOKEN), "", Some((at, status, message)));
        let refused = authed()
            .open_pull_request(&endpoint(&stub), &notes())
            .expect_err(at);
        assert_eq!(refused, expected, "{at}");
        assert!(!refused.contains(TOKEN), "{refused}");
    }
}

// The default branch is whatever the repository says, not `main`.
#[test]
fn test_the_base_is_the_repository_s_default_branch() {
    let stub = Stub::start(|ask| match ask.line().as_str() {
        "GET /repos/acme/hub" => Reply::json(200, &json!({"default_branch": "release/1.x"})),
        l if l.starts_with("GET /repos/acme/hub/git/ref/heads/") => {
            Reply::json(404, &json!({"message": "Not Found"}))
        }
        _ => Reply::json(404, &json!({"message": "Not Found"})),
    });
    let refused = authed()
        .open_pull_request(&endpoint(&stub), &notes())
        .expect_err("no such ref");
    assert_eq!(refused, "resolve branch release/1.x: HTTP 404: Not Found");
    assert_eq!(
        stub.lines()[1],
        "GET /repos/acme/hub/git/ref/heads/release/1.x"
    );
}

// github.go refuseEmptyPublish.
#[test]
fn test_an_empty_file_set_or_an_empty_file_is_refused_before_any_request() {
    let stub = Stub::github(Some(TOKEN), "");
    let none = authed()
        .open_pull_request(&endpoint(&stub), &request(&[]))
        .expect_err("nothing");
    assert_eq!(
        none,
        "refusing to publish an empty file set to acme/hub: nothing would be written"
    );
    let hollow = authed()
        .open_pull_request(
            &endpoint(&stub),
            &request(&[("a/x.md", "x"), ("a/empty.md", "")]),
        )
        .expect_err("empty");
    assert_eq!(
        hollow,
        "refusing to publish empty content to acme/hub/a/empty.md: a 0-byte write would replace the remote file with nothing"
    );
    assert!(stub.requests().is_empty());
}

// An owner or a repository is a path segment: it cannot end the path.
#[test]
fn test_an_owner_or_repository_that_is_not_a_github_name_is_refused() {
    let stub = Stub::github(Some(TOKEN), "");
    for (owner, repo) in [
        ("..", "hub"),
        ("acme", "a/b"),
        ("a c", "hub"),
        ("", "hub"),
        ("acme", ""),
    ] {
        let refused = authed()
            .open_pull_request(
                &endpoint(&stub),
                &PullRequest {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    ..notes()
                },
            )
            .expect_err("refused");
        assert_eq!(
            refused,
            format!("{owner}/{repo} is not an owner and repository GitHub names")
        );
    }
    assert!(stub.requests().is_empty());
}

#[test]
fn test_a_github_name_may_hold_letters_digits_dots_dashes_and_underscores() {
    let stub = Stub::github(Some(TOKEN), "");
    authed()
        .open_pull_request(
            &endpoint(&stub),
            &PullRequest {
                owner: "Acme-Co_1".to_string(),
                repo: "hub.v2-x_y".to_string(),
                ..notes()
            },
        )
        .expect("opened");
    assert_eq!(stub.requests()[0].line(), "GET /repos/Acme-Co_1/hub.v2-x_y");
}

// An Enterprise forge with no token_env names no variable to read.
#[test]
fn test_an_endpoint_that_names_no_token_variable_says_to_give_the_forge_one() {
    let stub = Stub::github(Some(TOKEN), "");
    let nameless = Endpoint {
        token_vars: Vec::new(),
        ..endpoint(&stub)
    };
    let refused = env(&[("GH_TOKEN", "x")])
        .open_pull_request(&nameless, &notes())
        .expect_err("no token");
    assert_eq!(
        refused,
        "no GitHub token: give the forge a token_env naming the variable that holds one"
    );
    assert!(stub.requests().is_empty());
}

// ------------------------------------------------- the adapter's own parts

#[test]
fn test_the_debug_form_says_whether_a_proxy_is_used_and_nothing_of_the_token() {
    let with = GitHub::with_env(|_| Some(TOKEN.to_string()));
    let shown = format!("{with:?}");
    assert!(shown.contains("proxy: true"), "{shown}");
    assert!(!shown.contains(TOKEN), "{shown}");
    let without = format!("{:?}", with.without_proxy());
    assert!(without.contains("proxy: false"), "{without}");
}

#[test]
fn test_a_branch_name_is_made_new_by_the_clock() {
    // After 2023-11-14 and before the year 2100: a real clock reading.
    let now = unix_seconds();
    assert!(now > 1_700_000_000 && now < 4_102_444_800, "{now}");
}

#[derive(Debug)]
struct Layered(&'static str, Option<Box<Layered>>);

impl std::fmt::Display for Layered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Layered {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.1.as_deref().map(|e| e as _)
    }
}

#[test]
fn test_an_error_and_every_cause_under_it_is_one_line() {
    let error = Layered(
        "outer",
        Some(Box::new(Layered(
            "middle",
            Some(Box::new(Layered("root", None))),
        ))),
    );
    assert_eq!(chain(&error), "outer: middle: root");
    assert_eq!(chain(&Layered("alone", None)), "alone");
}

// A client built with its proxy on goes through the environment's proxy, and
// one built without never does. The environment is the process's and is read
// once, so each case runs in a process of its own: this test run again, told
// which case it is.
const PROXY_CASE: &str = "CTXLOOM_TEST_PROXY_CASE";

fn search_through_environment_proxy(proxy: bool) -> bool {
    let github = GitHub::with_env(|_| None);
    let github = if proxy {
        github
    } else {
        github.without_proxy()
    };
    let endpoint = Endpoint {
        api_url: "http://github.invalid".to_string(),
        token_vars: Vec::new(),
    };
    github.search_repositories(&endpoint, "", 1).is_ok()
}

#[test]
fn test_the_proxy_of_the_environment_is_used_unless_the_client_is_made_without_one() {
    if let Ok(case) = std::env::var(PROXY_CASE) {
        // The child: report by exit status.
        let reached = search_through_environment_proxy(case == "on");
        assert_eq!(reached, case == "on", "case {case}");
        return;
    }
    let me = std::env::current_exe().expect("this test binary");
    let name = format!(
        "{}::test_the_proxy_of_the_environment_is_used_unless_the_client_is_made_without_one",
        module_path!().split_once("::").map_or("", |(_, rest)| rest)
    );
    for (case, reaches) in [("on", true), ("off", false)] {
        let proxy = Stub::replying(200, r#"{"items": []}"#);
        let out = std::process::Command::new(&me)
            .args(["--exact", &name, "--nocapture"])
            .env(PROXY_CASE, case)
            .env("HTTP_PROXY", proxy.url())
            .env("http_proxy", proxy.url())
            .env_remove("NO_PROXY")
            .env_remove("no_proxy")
            .output()
            .expect("the child test");
        assert!(
            out.status.success(),
            "{case}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(proxy.requests().len(), usize::from(reaches), "{case}");
    }
}

// ------------------------------------------------------------ the stub itself

fn raw(stub: &Stub, request: &str) -> String {
    use std::io::{Read, Write};
    let address = stub.url().trim_start_matches("http://").to_string();
    let mut stream = std::net::TcpStream::connect(address).expect("the stub");
    stream.write_all(request.as_bytes()).expect("a request");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("an answer");
    answer
}

#[test]
fn test_the_stub_refuses_a_write_with_no_credentials_and_one_with_the_wrong_ones() {
    let post = |auth: &str| {
        format!("POST /repos/a/b/git/blobs HTTP/1.1\r\n{auth}Content-Length: 0\r\n\r\n")
    };
    let open = Stub::github(None, "");
    assert!(raw(&open, &post("")).starts_with("HTTP/1.1 401"));
    assert!(raw(&open, &post("Authorization: Bearer any\r\n")).starts_with("HTTP/1.1 201"));
    let closed = Stub::github(Some("right"), "");
    assert!(raw(&closed, &post("Authorization: Bearer wrong\r\n")).starts_with("HTTP/1.1 401"));
    assert!(raw(&closed, &post("Authorization: Bearer right\r\n")).starts_with("HTTP/1.1 201"));
}

#[test]
fn test_the_stub_decodes_percent_escapes_and_plus_in_a_query() {
    let stub = Stub::replying(200, "{}");
    raw(
        &stub,
        "GET /x?a=b%20c%2Fd%25&e=f+g&h=100%zz&i=%2f%41 HTTP/1.1\r\n\r\n",
    );
    let seen = &stub.requests()[0];
    assert_eq!(seen.param("a").as_deref(), Some("b c/d%"));
    assert_eq!(seen.param("e").as_deref(), Some("f g"));
    assert_eq!(seen.param("h").as_deref(), Some("100%zz"));
    assert_eq!(seen.param("i").as_deref(), Some("/A"));
    assert_eq!(seen.param("missing"), None);
}

#[test]
fn test_the_stub_answers_a_request_it_was_started_for() {
    let stub = Stub::replying(200, "{}");
    assert!(raw(&stub, "GET /x HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 200"));
    assert_eq!(stub.lines(), vec!["GET /x".to_string()]);
}
