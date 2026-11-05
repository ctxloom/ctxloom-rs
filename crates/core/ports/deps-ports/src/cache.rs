//! Where the derived cache keeps a repository and a bundle's installed trees,
//! as data the application computes and an adapter is handed: an adapter never
//! works a path out of an address.

/// A relative path in the cache, as segments: at least one, each a plain name
/// (never empty, `.`, `..`, or holding a separator).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CachePath(Vec<String>);

impl CachePath {
    /// `segments` as a cache path; none if there are none, or any is not a
    /// plain name.
    pub fn new(segments: Vec<String>) -> Option<CachePath> {
        let plain = |s: &String| !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\']);
        (!segments.is_empty() && segments.iter().all(plain)).then_some(CachePath(segments))
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }
}

/// A repository: where it is fetched from, and where its clone is cached.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Repository {
    pub url: String,
    pub cache: CachePath,
}

/// A bundle of a repository, as installed trees are kept for it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BundleSlot {
    /// The version-less canonical reference: what a tree is marked with.
    pub identity: String,
    /// Where the bundle's repository is cached.
    pub repository: CachePath,
    /// The bundle's path in the repository, below that.
    pub bundle: CachePath,
}

#[cfg(test)]
#[path = "cache.test.rs"]
mod tests;
