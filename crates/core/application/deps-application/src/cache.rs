//! The cache places the application hands the driven adapters: the segments a
//! repository's clone and a bundle's installed trees are kept under are worked
//! out here, once, from the address; an adapter uses what it is given.

use deps::address::cache_segments;
use deps::reference::RemoteRef;
use deps_ports::{BundleSlot, CachePath, Why};

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
