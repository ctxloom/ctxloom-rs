//! The cache places the application hands the driven adapters: the segments a
//! repository's clone and a bundle's installed trees are kept under are worked
//! out here, once, from the address; an adapter uses what it is given.

use deps::address::cache_segments;
use deps::reference::RemoteRef;
use std::collections::BTreeMap;

use deps_ports::{BundleSlot, CachePath, Remotes, RepoTree, Repository, Why};

/// The repository at `url` and where its clone is cached; why it has no place
/// if its address names no repository.
pub(crate) fn repository(url: &str) -> Result<Repository, Why> {
    let cache = cache_segments(url)
        .and_then(CachePath::new)
        .ok_or_else(|| format!("refusing to cache {url:?}: it names no repository"))?;
    Ok(Repository {
        url: url.to_string(),
        cache,
    })
}

/// The git remotes asked by address: each call works the repository's cache
/// place out (once, here) and hands it across the port.
pub(crate) struct ByAddress<'r>(pub(crate) &'r dyn Remotes);

impl ByAddress<'_> {
    pub(crate) fn ensure(&self, url: &str) -> Result<(), Why> {
        self.0.ensure(&repository(url)?)
    }

    pub(crate) fn refresh(&self, url: &str) -> Result<(), Why> {
        self.0.refresh(&repository(url)?)
    }

    pub(crate) fn tags(&self, url: &str) -> Result<Vec<String>, Why> {
        self.0.tags(&repository(url)?)
    }

    pub(crate) fn resolve_ref(&self, url: &str, name: &str) -> Result<String, Why> {
        self.0.resolve_ref(&repository(url)?, name)
    }

    pub(crate) fn resolve_tag(&self, url: &str, name: &str) -> Result<String, Why> {
        self.0.resolve_tag(&repository(url)?, name)
    }

    pub(crate) fn default_branch(&self, url: &str) -> Result<String, Why> {
        self.0.default_branch(&repository(url)?)
    }

    pub(crate) fn read_bundle(
        &self,
        url: &str,
        sha: &str,
        bundle: &str,
    ) -> Result<Option<RepoTree>, Why> {
        self.0.read_bundle(&repository(url)?, sha, bundle)
    }

    pub(crate) fn list_bundles(&self, url: &str) -> Result<Vec<String>, Why> {
        self.0.list_bundles(&repository(url)?)
    }

    pub(crate) fn has_bundle(&self, url: &str, bundle: &str) -> Result<bool, Why> {
        self.0.has_bundle(&repository(url)?, bundle)
    }

    pub(crate) fn publish(
        &self,
        url: &str,
        files: &BTreeMap<String, Vec<u8>>,
        message: &str,
    ) -> Result<String, Why> {
        self.0.publish(&repository(url)?, files, message)
    }
}

/// Where the trees installed for `remote` are kept.
pub(crate) fn slot(remote: &RemoteRef) -> Result<BundleSlot, Why> {
    let repository = cache_segments(&remote.url)
        .and_then(CachePath::new)
        .ok_or_else(|| format!("{} names no repository", remote.url))?;
    // The reference parser has cleaned the path: no empty, `.` or `..` part.
    let bundle = CachePath::new(remote.bundle.split('/').map(str::to_string).collect())
        .ok_or_else(|| format!("{} names no bundle path", remote.identity))?;
    Ok(BundleSlot {
        identity: remote.identity.clone(),
        repository,
        bundle,
    })
}

/// Where the trees installed for the bundle `identity` are kept.
pub(crate) fn slot_of(identity: &str) -> Result<BundleSlot, Why> {
    let remote = RemoteRef::parse(identity)
        .ok_or_else(|| format!("{identity} is not a remote bundle reference"))?;
    slot(&remote)
}

#[cfg(test)]
#[path = "cache.test.rs"]
mod tests;
