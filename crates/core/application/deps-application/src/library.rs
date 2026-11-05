//! What the library reads of the remotes: the bundles installed from them at
//! their pins, a short reference spelled out, and a search of what they
//! publish.

use deps::reference::RemoteRef;
use deps::short;
use deps_ports::{Failure, InstalledBundle, RemoteHit, RemoteLibrary, RemoteSearch};
use library::tree::read_tree;

use crate::{cache, Deps};

impl RemoteLibrary for Deps {
    fn installed(&self) -> Result<Vec<InstalledBundle>, Failure> {
        let (lock, _) = self.lock().map_err(|e| Box::new(e) as Failure)?;
        let names = self
            .registry()
            .unwrap_or_else(|_| deps::registry::Registry::empty());
        let mut out = Vec::new();
        for (identity, entry) in &lock.entries {
            let Some(remote) = RemoteRef::parse(identity) else {
                continue;
            };
            if entry.sha.is_empty() {
                continue;
            }
            let Ok(slot) = cache::slot(&remote) else {
                continue;
            };
            if let Ok(Some(tree)) = self.files.read_installed(&slot, &entry.sha) {
                let name = names.lookup_url(&remote.url).map_or_else(
                    || identity.clone(),
                    |r| format!("{}/{}", r.name, remote.bundle),
                );
                out.push(InstalledBundle {
                    reference: identity.clone(),
                    name,
                    sha: entry.sha.clone(),
                    path: self.files.installed_path(&slot, &entry.sha),
                    url: remote.url,
                    executable: tree
                        .iter()
                        .filter(|(_, file)| file.executable)
                        .map(|(path, _)| path.clone())
                        .collect(),
                    files: tree
                        .into_iter()
                        .map(|(path, file)| (path, file.data))
                        .collect(),
                });
            }
        }
        Ok(out)
    }

    fn canonical(&self, reference: &str, local_exists: &dyn Fn(&str) -> bool) -> String {
        match self.registry() {
            Ok(registry) => short::expand(
                reference,
                &|alias| registry.get(alias).map(|r| r.url),
                local_exists,
            ),
            Err(_) => reference.to_string(),
        }
    }

    fn within(&self, reference: &str, url: &str, sha: &str) -> String {
        short::within_repository(reference, url, sha)
    }

    fn search(&self, query: &str) -> Result<RemoteSearch, Failure> {
        let registry = self.registry().map_err(|e| Box::new(e) as Failure)?;
        let remotes = registry.remotes();
        let mut found = RemoteSearch::default();
        if remotes.is_empty() {
            found.warnings.push("no remotes configured".to_string());
            return Ok(found);
        }
        let query = query.to_lowercase();
        for remote in remotes {
            match self.published(&remote.url) {
                Ok(bundles) => {
                    for (name, meta) in bundles {
                        let matches = name.to_lowercase().contains(&query)
                            || meta.description.to_lowercase().contains(&query);
                        if matches {
                            found.hits.push(RemoteHit {
                                remote: remote.name.clone(),
                                pull_ref: deps::address::canonical_ref(&remote.url, &name)
                                    .unwrap_or_default(),
                                name,
                                tags: meta.tags,
                                description: meta.description,
                                author: meta.author,
                            });
                        }
                    }
                }
                Err(detail) => found
                    .warnings
                    .push(format!("{} (bundle): {detail}", remote.name)),
            }
        }
        Ok(found)
    }
}

/// What a bundle says of itself.
struct Meta {
    description: String,
    tags: Vec<String>,
    author: String,
}

impl Deps {
    /// The bundles a repository publishes at its default branch's tip, each
    /// with what its manifest says of it, by name.
    fn published(&self, url: &str) -> Result<Vec<(String, Meta)>, String> {
        self.remotes.ensure(url)?;
        let names = self.remotes.list_bundles(url)?;
        let branch = self.remotes.default_branch(url)?;
        let tip = self.remotes.resolve_ref(url, &branch)?;
        let mut out = Vec::new();
        for name in names {
            let meta = match self.remotes.read_bundle(url, &tip, &name) {
                Ok(Some(tree)) => {
                    let files = tree.into_iter().map(|(k, f)| (k, f.data)).collect();
                    read_tree(&name, &files).ok()
                }
                _ => None,
            };
            out.push((
                name,
                meta.map_or(
                    Meta {
                        description: String::new(),
                        tags: Vec::new(),
                        author: String::new(),
                    },
                    |bundle| Meta {
                        description: bundle.description,
                        tags: bundle.tags,
                        author: bundle.author,
                    },
                ),
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
#[path = "library.test.rs"]
mod tests;
