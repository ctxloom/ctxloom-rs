//! `search`: the project's own content by name, tag or description.

use crate::agents::Served;
use crate::error::{AppError, Notice};
use crate::project::{Library, Outcome, Project};
use crate::views::{RemoteFound, SearchHit, SearchResults};
use crate::Flags;

/// The most local results a search answers.
const LOCAL_LIMIT: usize = 100;

/// What a search is asked.
#[derive(Clone, Debug, Default)]
pub struct SearchRequest {
    pub query: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub local: bool,
    pub remote: bool,
}

impl Library {
    /// `search`.
    pub fn search(&self, flags: &Flags, request: &SearchRequest) -> Outcome<SearchResults> {
        self.run(flags, |p| p.search(request))
    }
}

const LOCAL_KINDS: [&str; 5] = ["fragment", "command", "skill", "profile", "mcp_server"];

fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(needle)
}

fn has_tag(tags: &[String], query: &str) -> bool {
    tags.iter().any(|t| t.to_lowercase() == query)
}

impl Project {
    fn search(&mut self, request: &SearchRequest) -> Result<SearchResults, AppError> {
        if request.query.is_empty() && request.tags.is_empty() {
            return Err(AppError::SearchNothing);
        }
        let types: Vec<&str> = match request.kind.as_str() {
            "" => LOCAL_KINDS.to_vec(),
            kind if LOCAL_KINDS.contains(&kind) => vec![kind],
            "bundle" => Vec::new(),
            other => {
                return Err(AppError::SearchType {
                    value: other.to_string(),
                })
            }
        };
        // The scopes: neither flag (or both) searches both sources.
        let local_scope = !request.remote || request.local;
        let remote_scope = !request.local || request.remote;
        // Bundles are how content is distributed: they exist upstream only.
        let remote_wanted = remote_scope && matches!(request.kind.as_str(), "" | "bundle");
        if remote_wanted && request.remote && !request.local && self.remotes.is_none() {
            return Err(AppError::SearchRemote);
        }
        let remote = if remote_wanted && !request.query.is_empty() {
            self.search_remote(&request.query)
        } else {
            Vec::new()
        };
        let types: Vec<&str> = if local_scope { types } else { Vec::new() };
        let query = request.query.to_lowercase();
        let mut hits = Vec::new();
        for kind in &types {
            if query.is_empty() && *kind != "fragment" {
                continue;
            }
            match *kind {
                "fragment" => hits.extend(self.find_fragments(&query, &request.tags)),
                "command" => hits.extend(self.find_commands(&query)),
                "skill" => hits.extend(self.find_skills(&query)?),
                "profile" => hits.extend(self.find_profiles(&query)),
                _ => hits.extend(self.find_servers(&query)),
            }
        }
        sort_by_relevance(&mut hits);
        let total = hits.len();
        hits.truncate(LOCAL_LIMIT);
        let hidden = total - hits.len();
        if hidden > 0 {
            self.notices.push(Notice::text(format!(
                "{hidden} more local match(es) hidden by the {LOCAL_LIMIT}-result cap — narrow with --type or --tag to see the rest"
            )));
        }
        Ok(SearchResults {
            query: request.query.clone(),
            count: (hits.len() + remote.len()) as u64,
            local: hits,
            remote,
            hidden_local: hidden as u64,
        })
    }

    /// What the remotes publish whose name or description holds the query;
    /// what could not be searched is said, and the search goes on.
    fn search_remote(&mut self, query: &str) -> Vec<RemoteFound> {
        let Some(remotes) = self.remotes.clone() else {
            return Vec::new();
        };
        match remotes.search(query) {
            Ok(found) => {
                self.notices.extend(
                    found
                        .warnings
                        .into_iter()
                        .map(|w| Notice::text(format!("remote search: {w}"))),
                );
                found
                    .hits
                    .into_iter()
                    .map(|hit| RemoteFound {
                        remote: hit.remote,
                        name: hit.name,
                        kind: "bundle".to_string(),
                        tags: hit.tags,
                        description: hit.description,
                        author: hit.author,
                        pull_ref: hit.pull_ref,
                    })
                    .collect()
            }
            Err(cause) => {
                self.notices.push(Notice::text(format!(
                    "remote search error: {}",
                    cause.code()
                )));
                Vec::new()
            }
        }
    }

    fn find_fragments(&self, query: &str, tags: &[String]) -> Vec<SearchHit> {
        let infos = if tags.is_empty() {
            self.catalog.list_fragments()
        } else {
            self.catalog.by_tags(tags)
        };
        infos
            .into_iter()
            .filter_map(|info| {
                let how = if contains(&info.name, query) {
                    "name"
                } else if has_tag(&info.tags, query) {
                    "tag"
                } else {
                    return None;
                };
                Some(SearchHit {
                    kind: "fragment".to_string(),
                    name: info.name,
                    tags: info.tags,
                    source: info.bundle,
                    r#match: how.to_string(),
                })
            })
            .collect()
    }

    fn find_commands(&self, query: &str) -> Vec<SearchHit> {
        self.catalog
            .list_commands()
            .into_iter()
            .filter(|info| contains(&info.name, query))
            .map(|info| SearchHit {
                kind: "command".to_string(),
                name: info.name,
                tags: Vec::new(),
                source: info.bundle,
                r#match: "name".to_string(),
            })
            .collect()
    }

    fn find_skills(&mut self, query: &str) -> Result<Vec<SearchHit>, AppError> {
        let list = self.skill_rows()?;
        Ok(list
            .into_iter()
            .filter_map(|row| {
                let how = if contains(&row.name, query) {
                    "name"
                } else if contains(&row.description, query) {
                    "description"
                } else if has_tag(&row.tags, query) {
                    "tag"
                } else {
                    return None;
                };
                Some(SearchHit {
                    kind: "skill".to_string(),
                    name: row.name,
                    tags: row.tags,
                    source: row.source,
                    r#match: how.to_string(),
                })
            })
            .collect())
    }

    fn find_profiles(&self, query: &str) -> Vec<SearchHit> {
        self.profile_list()
            .rows
            .into_iter()
            .filter_map(|row| {
                let how = if contains(&row.name, query) {
                    "name"
                } else if contains(&row.description, query) {
                    "description"
                } else if has_tag(&row.tags, query) {
                    "tag"
                } else {
                    return None;
                };
                Some(SearchHit {
                    kind: "profile".to_string(),
                    name: row.name,
                    tags: row.tags,
                    source: String::new(),
                    r#match: how.to_string(),
                })
            })
            .collect()
    }

    fn find_servers(&self, query: &str) -> Vec<SearchHit> {
        self.registered()
            .into_iter()
            .filter_map(|(name, served): (String, Served)| {
                let target = if !served.server.served_by.is_empty() {
                    served.server.served_by.clone()
                } else if !served.server.url.is_empty() {
                    served.server.url.clone()
                } else {
                    served.server.command.clone()
                };
                (contains(&name, query) || contains(&target, query)).then(|| SearchHit {
                    kind: "mcp_server".to_string(),
                    name,
                    tags: Vec::new(),
                    source: target,
                    r#match: "name".to_string(),
                })
            })
            .collect()
    }
}

/// Name matches first, then tag, then the rest; by name then type within.
fn sort_by_relevance(hits: &mut [SearchHit]) {
    let score = |m: &str| match m {
        "name" => 2,
        "tag" => 1,
        _ => 0,
    };
    hits.sort_by(|a, b| {
        score(&b.r#match)
            .cmp(&score(&a.r#match))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.kind.cmp(&b.kind))
    });
}
