//! A bundle named in a profile, taken apart (Go's `remote.ParseReference` and
//! `Reference.LockKey`): which repository it is in, which bundle, and which
//! version the profile asks for. A reference to this project's own bundles,
//! to a companion or to nothing at all is not remote.

use library::refs::{has_scheme, Parts, SourceClass};

/// A remote bundle reference.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RemoteRef {
    /// The version-less canonical reference: the lock key.
    pub identity: String,
    /// The repository's address, as it is fetched from.
    pub url: String,
    /// The bundle's path in the repository's bundles.
    pub bundle: String,
    /// The version expression after `@` (a tag, a branch, a commit, a
    /// range); empty for the default branch.
    pub constraint: String,
    /// The item selector after `#` (`profiles/reach`), without the `#`;
    /// empty for the bundle itself.
    pub item: String,
}

impl RemoteRef {
    /// `raw` as a remote bundle reference: the canonical grammar
    /// (`ctxloom+git://` and `ctxloom+file://`), or a fetch address
    /// (`<url>@bundles/<path>[@version]`, whose repository path is kept as
    /// written). None for a local or companion reference, a bare name, and
    /// anything that does not parse.
    pub fn parse(raw: &str) -> Option<RemoteRef> {
        let raw = raw.trim();
        let (base, item) = match raw.split_once('#') {
            Some((base, item)) => (base, item),
            None => (raw, ""),
        };
        let parts = if has_scheme(base) {
            Parts::parse(base).ok()?
        } else {
            fetch_address(base)?
        };
        let url = match parts.class {
            SourceClass::Git => format!("https://{}{}", parts.host, parts.repo_path),
            SourceClass::File => format!("file://{}", parts.repo_path),
            _ => return None,
        };
        // Version-less by `render(false)`; `base` holds no `#`, so no fragment.
        let identity = parts.render(false);
        Some(RemoteRef {
            identity,
            url,
            bundle: parts.bundle,
            constraint: parts.version,
            item: item.to_string(),
        })
    }

    /// The reference to the bundle alone, at its version, without an item.
    pub fn bundle_ref(&self) -> String {
        match Parts::parse(&self.identity) {
            Ok(parts) => Parts {
                version: self.constraint.clone(),
                ..parts
            }
            .render(true),
            Err(_) => self.identity.clone(),
        }
    }
}

/// A fetch address (`<url>@bundles/<path>[@version]`) as the reference it
/// names. The repository path is kept as written, ".git" included: it is
/// the directory that is there.
fn fetch_address(base: &str) -> Option<Parts> {
    let (class, host, path) = if let Some(rest) = base.strip_prefix("file://") {
        (SourceClass::File, "", rest.to_string())
    } else if let Some(rest) = base
        .strip_prefix("https://")
        .or_else(|| base.strip_prefix("http://"))
    {
        let (host, path) = rest.split_once('/')?;
        (SourceClass::Git, host, format!("/{path}"))
    } else if let Some((user_host, path)) = base.split_once(':').filter(|(u, _)| !u.contains('/')) {
        let host = user_host.rsplit_once('@')?.1;
        (SourceClass::Git, host, format!("/{path}"))
    } else {
        return None;
    };
    let (repo, after) = path.split_once('@')?;
    let rest = after.strip_prefix("bundles/")?;
    let (bundle, version) = rest.split_once('@').unwrap_or((rest, ""));
    let repo = repo.trim_end_matches('/');
    let mut parts = match class {
        SourceClass::File => Parts::file(repo, bundle),
        _ => Parts::git(host, repo, bundle),
    }
    .ok()?;
    parts.version = version.to_string();
    Some(parts)
}

#[cfg(test)]
#[path = "reference.test.rs"]
mod tests;
