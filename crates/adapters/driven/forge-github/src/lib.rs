//! The Forge port over GitHub's REST API, on reqwest (S3 follow-up): search
//! GitHub for ctxloom repositories, and publish a bundle as one commit on a
//! new branch with a pull request into the default branch (Go's
//! `GitHubFetcher.SearchRepos` and `GitHubPublisher`, through the Git Data API).
//!
//! The token is read from the environment (the snapshot the program took once,
//! through [`GitHub::with_env`]), by the variables the endpoint names, at the
//! moment of a request: it is never an argument, a field, a log line, an
//! error message or Debug output.

mod encode;
#[cfg(any(test, feature = "test-support"))]
#[path = "stub.test.rs"]
pub mod stub;

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use deps_ports::{Endpoint, Forge, ForgeRepo, Opened, PullRequest, Why};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};

/// What the environment answers to a variable's name.
type Env = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// GitHub's REST API.
#[derive(Clone)]
pub struct GitHub {
    env: Env,
    proxy: bool,
}

impl GitHub {
    /// GitHub with the environment as `env` answers it.
    pub fn with_env(env: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> GitHub {
        GitHub {
            env: Arc::new(env),
            proxy: true,
        }
    }

    /// The same, never through a proxy (for an API on a local port).
    pub fn without_proxy(self) -> GitHub {
        GitHub {
            proxy: false,
            ..self
        }
    }

    /// The `Authorization` header for the first of `vars` the environment
    /// sets (and does not leave empty); none where none is. The value is
    /// marked sensitive, so no debug output of a request shows it, and a value
    /// that cannot be a header is refused by the variable's name, never its
    /// content.
    fn bearer(&self, vars: &[String]) -> Result<Option<HeaderValue>, Why> {
        for var in vars {
            let Some(token) = (self.env)(var).filter(|token| !token.is_empty()) else {
                continue;
            };
            let mut header = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| format!("the token in {var} is not a valid header value"))?;
            header.set_sensitive(true);
            return Ok(Some(header));
        }
        Ok(None)
    }

    fn session(&self, endpoint: &Endpoint, auth: Option<HeaderValue>) -> Result<Session, Why> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        headers.insert(
            "x-github-api-version",
            HeaderValue::from_static("2022-11-28"),
        );
        let mut client = Client::builder()
            .user_agent("ctxloom")
            .default_headers(headers)
            .timeout(TIMEOUT);
        if !self.proxy {
            client = client.no_proxy();
        }
        Ok(Session {
            client: client
                .build()
                .map_err(|e| format!("cannot make an HTTP client: {}", chain(&e)))?,
            base: endpoint.api_url.trim_end_matches('/').to_string(),
            auth,
        })
    }
}

impl fmt::Debug for GitHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The environment is a function and holds the token: show neither.
        f.debug_struct("GitHub")
            .field("proxy", &self.proxy)
            .finish_non_exhaustive()
    }
}

/// How long any one request may take.
const TIMEOUT: Duration = Duration::from_secs(30);

/// A client at an endpoint, with the credential for it.
struct Session {
    client: Client,
    base: String,
    auth: Option<HeaderValue>,
}

impl Session {
    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Why> {
        self.send(self.client.get(format!("{}{path}", self.base)).query(query))
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, Why> {
        self.send(
            self.client
                .post(format!("{}{path}", self.base))
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string()),
        )
    }

    /// The answer's JSON; an error answer is its status and what GitHub says
    /// of it.
    fn send(&self, request: RequestBuilder) -> Result<Value, Why> {
        let request = match &self.auth {
            Some(auth) => request.header(AUTHORIZATION, auth.clone()),
            None => request,
        };
        let response = request.send().map_err(|e| chain(&e))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            let said = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|body| body["message"].as_str().map(str::to_string))
                .filter(|message| !message.is_empty());
            return Err(match said {
                Some(message) => format!("HTTP {}: {message}", status.as_u16()),
                None => format!("HTTP {}", status.as_u16()),
            });
        }
        serde_json::from_str(&text).map_err(|_| "the answer is not JSON".to_string())
    }
}

/// An error and what caused it, as one line.
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

/// The string at `value` of `path`, or why there is none.
fn string_at(value: &Value, path: &[&str], what: &str) -> Result<String, Why> {
    let mut at = value;
    for key in path {
        at = &at[*key];
    }
    at.as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("the answer has no {what}"))
}

/// Whether `name` is something GitHub names an owner or a repository: letters,
/// digits, dots, dashes and underscores. It is a path segment, so nothing else
/// is let through.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// The files as one commit on `base_sha`: a blob each, one tree over the base
/// tree, one commit. Nothing points at it yet.
fn commit_files(
    session: &Session,
    repo_path: &str,
    base_sha: &str,
    base_tree: &str,
    request: &PullRequest,
) -> Result<String, Why> {
    let step = |what: String| move |why: Why| format!("{what}: {why}");
    // Base64, so that any bytes arrive as they are: the tree's own content
    // field must be UTF-8 and mangles what is not.
    let mut entries = Vec::new();
    for (path, data) in &request.files {
        let blob = session
            .post(
                &format!("{repo_path}/git/blobs"),
                &json!({"content": encode::base64(data), "encoding": "base64"}),
            )
            .map_err(step(format!("create blob for {path}")))?;
        entries.push(json!({
            "path": path,
            "mode": "100644",
            "type": "blob",
            "sha": string_at(&blob, &["sha"], "blob")?,
        }));
    }
    let tree = session
        .post(
            &format!("{repo_path}/git/trees"),
            &json!({"base_tree": base_tree, "tree": entries}),
        )
        .map_err(step("create tree".to_string()))?;
    string_at(
        &session
            .post(
                &format!("{repo_path}/git/commits"),
                &json!({
                    "message": request.message,
                    "tree": string_at(&tree, &["sha"], "tree")?,
                    "parents": [base_sha],
                }),
            )
            .map_err(step("create commit".to_string()))?,
        &["sha"],
        "commit",
    )
}

impl Forge for GitHub {
    fn search_repositories(
        &self,
        endpoint: &Endpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ForgeRepo>, Why> {
        let failed = |why: Why| format!("search failed: {why}");
        let session = self
            .session(endpoint, self.bearer(&endpoint.token_vars).map_err(failed)?)
            .map_err(failed)?;
        let q = if query.is_empty() {
            "ctxloom in:name".to_string()
        } else {
            format!("{query} ctxloom in:name")
        };
        let answer = session
            .get(
                "/search/repositories",
                &[
                    ("q", q),
                    ("sort", "stars".to_string()),
                    ("order", "desc".to_string()),
                    ("per_page", limit.to_string()),
                ],
            )
            .map_err(failed)?;
        let items = answer["items"]
            .as_array()
            .ok_or_else(|| failed("the answer has no items".to_string()))?;
        Ok(items
            .iter()
            .filter_map(|item| {
                let name = item["name"].as_str()?;
                if name != "ctxloom" && !name.starts_with("ctxloom-") {
                    return None;
                }
                Some(ForgeRepo {
                    owner: item["owner"]["login"].as_str()?.to_string(),
                    name: name.to_string(),
                    description: item["description"].as_str().unwrap_or_default().to_string(),
                    stars: item["stargazers_count"].as_u64().unwrap_or(0),
                    url: item["html_url"].as_str()?.to_string(),
                })
            })
            .collect())
    }

    fn open_pull_request(&self, endpoint: &Endpoint, request: &PullRequest) -> Result<Opened, Why> {
        let (owner, repo) = (&request.owner, &request.repo);
        if !is_name(owner) || !is_name(repo) {
            return Err(format!(
                "{owner}/{repo} is not an owner and repository GitHub names"
            ));
        }
        if request.files.is_empty() {
            return Err(format!(
                "refusing to publish an empty file set to {owner}/{repo}: nothing would be written"
            ));
        }
        if let Some((path, _)) = request.files.iter().find(|(_, data)| data.is_empty()) {
            return Err(format!(
                "refusing to publish empty content to {owner}/{repo}/{path}: a 0-byte write would replace the remote file with nothing"
            ));
        }
        let Some(auth) = self.bearer(&endpoint.token_vars)? else {
            return Err(match endpoint.token_vars.as_slice() {
                [] => {
                    "no GitHub token: give the forge a token_env naming the variable that holds one"
                        .to_string()
                }
                vars => format!(
                    "no GitHub token: set {} to open a pull request",
                    vars.join(" or ")
                ),
            });
        };
        let session = self.session(endpoint, Some(auth))?;
        let step = |what: String| move |why: Why| format!("{what}: {why}");
        let repo_path = format!("/repos/{owner}/{repo}");

        let base = string_at(
            &session
                .get(&repo_path, &[])
                .map_err(step(format!("get repository {owner}/{repo}")))?,
            &["default_branch"],
            "default branch",
        )?;
        let base_sha = string_at(
            &session
                .get(
                    &format!("{repo_path}/git/ref/heads/{}", encode::path(&base)),
                    &[],
                )
                .map_err(step(format!("resolve branch {base}")))?,
            &["object", "sha"],
            "commit",
        )?;
        let base_tree = string_at(
            &session
                .get(&format!("{repo_path}/git/commits/{base_sha}"), &[])
                .map_err(step(format!("read base commit {base_sha}")))?,
            &["tree", "sha"],
            "tree",
        )?;

        let commit = commit_files(&session, &repo_path, &base_sha, &base_tree, request)?;

        // The branch last: every object it points at exists by then, so a
        // failure before this leaves nothing behind.
        let branch = format!("{}-{}", request.branch, unix_seconds());
        session
            .post(
                &format!("{repo_path}/git/refs"),
                &json!({"ref": format!("refs/heads/{branch}"), "sha": commit}),
            )
            .map_err(step("create branch".to_string()))?;
        let pull = session
            .post(
                &format!("{repo_path}/pulls"),
                &json!({
                    "title": request.title,
                    "body": request.body,
                    "head": branch,
                    "base": base,
                }),
            )
            .map_err(step("failed to create pull request".to_string()))?;
        Ok(Opened {
            url: string_at(&pull, &["html_url"], "page")?,
            commit,
            branch,
        })
    }
}

/// Seconds since the epoch: what makes a branch name new.
fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;

#[cfg(test)]
#[path = "live.test.rs"]
mod live;
