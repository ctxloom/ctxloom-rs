//! `remote discover`: a search of GitHub for ctxloom repositories, through the
//! forge port.

use deps::forge;
use deps_ports::Endpoint;

use crate::views::{Discovered, DiscoveredRepo};
use crate::{AppError, Deps, Notice, Outcome};

/// How many repositories a search asks for when it is not told.
const DEFAULT_LIMIT: usize = 30;

/// What `remote discover` is asked.
#[derive(Clone, Debug, Default)]
pub struct DiscoverRequest {
    /// Words the repositories must match; empty for all.
    pub query: String,
    /// `all` or `github`; empty is `all`.
    pub source: String,
    /// At most this many; 0 is 30.
    pub limit: usize,
    /// At least this many stars.
    pub min_stars: u64,
}

impl Deps {
    /// `remote discover`.
    pub fn remote_discover(&self, request: &DiscoverRequest) -> Outcome<Discovered> {
        let mut notices = Vec::new();
        let result = self.discovering(request, &mut notices);
        Outcome { result, notices }
    }

    fn discovering(
        &self,
        request: &DiscoverRequest,
        notices: &mut Vec<Notice>,
    ) -> Result<Discovered, AppError> {
        // Searching is a GitHub-only capability: plain git clones a known
        // address and cannot enumerate a host. `all` is therefore GitHub.
        if !matches!(request.source.as_str(), "" | "all" | "github") {
            return Err(AppError::DiscoverSource {
                source: request.source.clone(),
            });
        }
        let limit = if request.limit == 0 {
            DEFAULT_LIMIT
        } else {
            request.limit
        };
        let github = forge::resolve("https://github.com/", "github", &self.registry()?);
        let endpoint = Endpoint {
            api_url: github.api(),
            token_vars: github.token_vars(),
        };
        let found = match self
            .forge
            .search_repositories(&endpoint, &request.query, limit)
        {
            Ok(found) => found,
            Err(why) => {
                notices.push(Notice::text(format!("github: {why}")));
                return Err(AppError::DiscoverFailed);
            }
        };
        let repositories: Vec<DiscoveredRepo> = found
            .into_iter()
            .filter(|repo| repo.stars >= request.min_stars)
            .map(|repo| DiscoveredRepo {
                add_command: format!(
                    "ctxloom remote create {} {}/{}",
                    repo.name, repo.owner, repo.name
                ),
                owner: repo.owner,
                name: repo.name,
                description: repo.description,
                stars: repo.stars,
                url: repo.url,
                forge: "github",
            })
            .collect();
        Ok(Discovered {
            count: repositories.len() as u64,
            repositories,
            errors: Vec::new(),
        })
    }
}

#[cfg(test)]
#[path = "discover.test.rs"]
mod tests;
