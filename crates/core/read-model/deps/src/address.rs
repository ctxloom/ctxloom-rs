//! A repository's address: how a remote's URL is stored, which repository two
//! spellings name, where its clone is cached, and the reference to one of its
//! bundles (Go's `refuri.ParseRepoURL`, `CanonicalRepoURL` and `RepoCache.
//! RepoDirForURL`).

use crate::DepsError;

/// The address a remote is stored under: `owner/repo` is GitHub's, an scp-like
/// `user@host:path` is its https URL, an http(s) or file URL is as written.
/// Anything else names no repository.
pub fn stored_url(raw: &str) -> Result<String, DepsError> {
    let raw = raw.trim();
    let refused = |detail: &str| DepsError::NoRepository {
        url: raw.to_string(),
        detail: detail.to_string(),
    };
    let stored = expand(raw).ok_or_else(|| {
        if is_local_path(raw) {
            refused("a path on this machine is resolved to the repository there first")
        } else if raw.starts_with('~') {
            refused("write the absolute path instead of ~")
        } else if raw.is_empty() || !raw.contains(['/', ':']) {
            refused("a bare word names no repository: write owner/repo or a URL")
        } else {
            refused("write owner/repo, or an https, http or file URL")
        }
    })?;
    match repository_key(&stored) {
        Some(_) => Ok(stored),
        None => Err(refused("it names no host and path")),
    }
}

/// The address `raw` stands for, in the spellings a remote may be written in.
fn expand(raw: &str) -> Option<String> {
    if raw.starts_with("file://") || raw.starts_with("https://") || raw.starts_with("http://") {
        return Some(raw.to_string());
    }
    if raw.contains("://") || raw.is_empty() {
        return None;
    }
    if let Some((user_host, path)) = raw.split_once(':') {
        let host = user_host.rsplit_once('@')?.1;
        let path = path.trim_start_matches('/');
        return (!host.is_empty() && !path.is_empty() && !user_host.contains('/'))
            .then(|| format!("https://{host}/{path}"));
    }
    if is_local_path(raw) || raw.starts_with('~') || raw.starts_with('/') {
        return None;
    }
    let (first, rest) = raw.split_once('/')?;
    if first.is_empty() || rest.is_empty() {
        return None;
    }
    // `host.tld/owner/repo` names its host; `owner/repo` is GitHub's.
    Some(if first.contains('.') {
        format!("https://{raw}")
    } else {
        format!("https://github.com/{raw}")
    })
}

/// The identity two spellings of one repository share, or none for an address
/// that names no repository.
pub fn repository_key(url: &str) -> Option<String> {
    library::refs::repository_key(url)
}

/// Whether `a` and `b` name one repository.
pub fn same_repository(a: &str, b: &str) -> bool {
    library::refs::same_repository(a, b)
}

/// The path segments a repository's clone is cached under, below the cache
/// root: the host and path of an https address, the path of a file address;
/// never an empty, `.` or `..` segment.
pub fn cache_segments(url: &str) -> Option<Vec<String>> {
    let key = repository_key(url)?;
    let path = key.strip_prefix("file://").unwrap_or(&key);
    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .map(str::to_string)
        .collect();
    (!segments.is_empty()).then_some(segments)
}

/// The owner and repository an http(s) address names (`https://host/owner/
/// repo`, with or without `.git` or a trailing slash): a forge API's path
/// segments. None for any other address, or a path of other than two segments.
pub fn owner_repo(url: &str) -> Option<(String, String)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (_, path) = rest.split_once('/')?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut segments = path.split('/');
    let (owner, repo) = (segments.next()?, segments.next()?);
    if segments.next().is_some() || owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

/// The version-less reference to the bundle `bundle` of the repository at
/// `url`: the key a lock entry is under.
pub fn canonical_ref(url: &str, bundle: &str) -> Option<String> {
    use library::refs::Parts;
    let parts = if let Some(path) = url.strip_prefix("file://") {
        Parts::file(path.trim_end_matches('/'), bundle)
    } else {
        let rest = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?;
        let (host, path) = rest.split_once('/')?;
        Parts::git(host, path.trim_end_matches('/'), bundle)
    };
    parts.ok().map(|p| p.render(false))
}

/// Whether `raw` is spelled as a path on this machine (absolute, or relative
/// with a leading `.`), so that the repository there is the one meant.
pub fn is_local_path(raw: &str) -> bool {
    raw.starts_with('/')
        || raw.starts_with("./")
        || raw.starts_with("../")
        || raw == "."
        || raw == ".."
}

#[cfg(test)]
#[path = "address.test.rs"]
mod tests;
