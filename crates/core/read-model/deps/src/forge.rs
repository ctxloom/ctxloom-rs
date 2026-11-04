//! Which forge a remote is on, how its API is reached, and where its token
//! comes from (Go's `resolveForge`). Pure: the registry is handed in, and the
//! token is named, never read.

use crate::registry::{ForgeEntry, Registry};

/// The public GitHub's REST API.
pub const PUBLIC_API: &str = "https://api.github.com";

/// What a forge is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// GitHub, github.com or Enterprise: search and pull requests by its API.
    Github,
    /// Plain git: no API, auth is the user's own git configuration.
    Git,
}

impl Kind {
    /// The name it has in `forges:` and in `--forge`.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Github => "github",
            Kind::Git => "git",
        }
    }
}

/// The forge a remote is bound to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Resolved {
    pub kind: Kind,
    pub base_url: String,
    /// An explicit API endpoint; empty is the public one.
    pub api_url: String,
    /// The environment variable the forge names its token in; empty for none.
    pub token_env: String,
}

impl Resolved {
    /// Whether the API is the public GitHub's.
    pub fn is_public(&self) -> bool {
        let api = self.api_url.trim_end_matches('/');
        api.is_empty() || api == PUBLIC_API
    }

    /// The API endpoint: the explicit one, else the public GitHub's.
    pub fn api(&self) -> String {
        if self.is_public() {
            PUBLIC_API.to_string()
        } else {
            self.api_url.trim_end_matches('/').to_string()
        }
    }

    /// The environment variables the token is read from, the first one set
    /// winning: the forge's own `token_env`, then, for the public GitHub only,
    /// `GH_TOKEN` and `GITHUB_TOKEN`. A credential of github.com is never
    /// spent at another host.
    pub fn token_vars(&self) -> Vec<String> {
        if self.kind == Kind::Git {
            return Vec::new();
        }
        let mut vars = Vec::new();
        if !self.token_env.is_empty() {
            vars.push(self.token_env.clone());
        }
        if self.is_public() {
            vars.extend(["GH_TOKEN".to_string(), "GITHUB_TOKEN".to_string()]);
        }
        vars
    }
}

/// The forge of the remote at `remote_url`, bound by `label` (empty for none):
/// the label's forge; else the configured forge whose `base_url` names the same
/// server (host and port; labels in ascending order); else the `github` forge
/// for github.com; else the `git` one.
pub fn resolve(remote_url: &str, label: &str, registry: &Registry) -> Resolved {
    let forges = registry.forges();
    let of = |entry: &ForgeEntry| Resolved {
        kind: if entry.kind == "github" {
            Kind::Github
        } else {
            Kind::Git
        },
        base_url: entry.base_url.clone(),
        api_url: entry.api_url.clone(),
        token_env: entry.token_env.clone(),
    };
    if let Some(entry) = forges.iter().find(|entry| entry.label == label) {
        return of(entry);
    }
    let here = server(remote_url);
    if let Some(wanted) = &here {
        let by_server = forges
            .iter()
            .find(|entry| server(&entry.base_url).as_ref() == Some(wanted));
        if let Some(entry) = by_server {
            return of(entry);
        }
    }
    // Neither a label nor a server: github.com is GitHub, anything else plain
    // git, each as the registry has it (the built-in entry, or the one that
    // replaced it).
    let github = here.is_some_and(|(host, _)| host == "github.com");
    let built_in = forges
        .iter()
        .find(|entry| entry.label == if github { "github" } else { "git" })
        .map(of)
        .expect("the built-in forges are always there");
    Resolved {
        base_url: if github && built_in.base_url.is_empty() {
            "https://github.com".to_string()
        } else {
            built_in.base_url
        },
        ..built_in
    }
}

/// The server an address names: its host (lowercase, without `www.`) and its
/// port, empty where it is the scheme's own. A different port is a different
/// server, and must not be handed this forge's endpoint or token.
fn server(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    // `Url` drops a special scheme's own port (443 for https, 80 for http).
    let port = parsed.port().map(|p| p.to_string()).unwrap_or_default();
    Some((host, port))
}

#[cfg(test)]
#[path = "forge.test.rs"]
mod tests;
