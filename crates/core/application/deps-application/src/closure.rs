//! The dependency closure: every remote bundle that follows from the
//! project's profiles, each resolved to a commit (Go's `depgraph.go` and
//! `constraint_resolve.go`).
//!
//! A profile names remote bundles and parents. A remote profile is read out
//! of its bundle at the pinned commit and walked in turn, and may name only
//! its own repository. A repository no remote is registered for is refused and
//! never fetched.

use std::collections::{BTreeMap, BTreeSet};

use deps::constraint::{self, Kind, RepoVersions};
use deps::lock::Lock;
use deps::plan::{Closure, Pin};
use deps::reference::RemoteRef;
use deps::registry::Registry;
use deps::short;
use library::model::Bundle;
use library::profile::{canonical_key, Profile};
use library::tree::read_tree;

use crate::cache::ByAddress;
use crate::{AppError, Deps, Notice};

/// The project's own profiles: the closure's roots.
#[derive(Default)]
pub(crate) struct LocalProfiles {
    /// Every local profile, by its key `ctxloom+local:<bundle>#profiles/<name>`.
    pub by_key: BTreeMap<String, Profile>,
    /// The names of the project's bundles.
    pub bundles: BTreeSet<String>,
    /// The bundles that are there and could not be read: whatever only they
    /// reach is unreached, not removed.
    pub unreadable: Vec<String>,
}

impl Deps {
    /// The project's profiles, read from its bundles.
    pub(crate) fn local_profiles(&self) -> Result<LocalProfiles, AppError> {
        let mut local = LocalProfiles::default();
        let wrap = |e| AppError::Cause(e);
        if !self.content.has_root().map_err(wrap)? {
            return Ok(local);
        }
        for name in self.content.bundle_names().map_err(wrap)? {
            let files = self.content.read_tree(&name).map_err(wrap)?;
            local.bundles.insert(name.clone());
            match read_tree(&name, &files) {
                Ok(bundle) => {
                    for (profile, body) in bundle.profiles {
                        let key = format!("ctxloom+local:{name}#profiles/{profile}");
                        let mut body = body;
                        body.name.clone_from(&key);
                        local.by_key.insert(key, body);
                    }
                }
                Err(_) => local.unreadable.push(name),
            }
        }
        Ok(local)
    }
}

/// How a walk resolves a constraint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mode {
    /// Pull, init: every existing pin is carried forward; only a first pin is
    /// resolved. Clones are refreshed.
    Lock,
    /// Upgrade: every unheld pin is re-resolved to the newest commit its
    /// constraint allows. Clones are refreshed.
    Upgrade,
    /// Nothing is fetched and nothing resolved afresh: what the lock settles
    /// is carried, and the rest is skipped.
    Carried,
}

/// A resolved constraint.
#[derive(Clone, Debug)]
pub(crate) struct Resolved {
    pub hash: String,
    pub version: String,
    pub kind: Option<Kind>,
}

/// Where a profile was read from.
#[derive(Clone, Copy)]
enum Source<'s> {
    Local,
    Remote { url: &'s str, sha: &'s str },
}

/// A repository's versions, as the last fetch left them.
pub(crate) struct RepoView<'r> {
    pub(crate) remotes: &'r dyn deps_ports::Remotes,
    pub(crate) url: &'r str,
}

impl RepoVersions for RepoView<'_> {
    fn tags(&self) -> Result<Vec<String>, String> {
        ByAddress(self.remotes).tags(self.url)
    }

    fn resolve_ref(&self, name: &str) -> Result<String, String> {
        ByAddress(self.remotes).resolve_ref(self.url, name)
    }

    fn resolve_tag(&self, name: &str) -> Result<String, String> {
        ByAddress(self.remotes).resolve_tag(self.url, name)
    }

    fn default_branch(&self) -> Result<String, String> {
        ByAddress(self.remotes).default_branch(self.url)
    }
}

/// One walk of the profiles.
pub(crate) struct Walk<'a> {
    deps: &'a Deps,
    registry: &'a Registry,
    active: &'a Lock,
    locals: &'a LocalProfiles,
    mode: Mode,
    pub closure: Closure,
    /// Every remote reference met, as (identity, expression asked).
    pub declared: Vec<(String, String)>,
    /// The repositories refused as unregistered, by address.
    refused: BTreeSet<String>,
    visited: BTreeSet<String>,
    resolved: BTreeMap<String, Option<Resolved>>,
    /// The repositories fetched this walk, and whether they could be.
    pub fetched: BTreeMap<String, Result<(), String>>,
    bundles: BTreeMap<(String, String, String), Option<Bundle>>,
    pub notices: Vec<Notice>,
}

impl<'a> Walk<'a> {
    pub fn new(
        deps: &'a Deps,
        registry: &'a Registry,
        active: &'a Lock,
        locals: &'a LocalProfiles,
        mode: Mode,
    ) -> Walk<'a> {
        Walk {
            deps,
            registry,
            active,
            locals,
            mode,
            closure: Closure::new(),
            declared: Vec::new(),
            refused: BTreeSet::new(),
            visited: BTreeSet::new(),
            resolved: BTreeMap::new(),
            fetched: BTreeMap::new(),
            bundles: BTreeMap::new(),
            notices: Vec::new(),
        }
    }

    /// Walk every local profile, and note the local bundles that could not be
    /// read.
    pub fn walk_all(&mut self) {
        for name in &self.locals.unreadable {
            self.closure.mark_unexpanded(name);
        }
        let profiles: Vec<Profile> = self.locals.by_key.values().cloned().collect();
        for profile in &profiles {
            self.walk_profile(profile, Source::Local);
        }
    }

    /// The registration rule's refusal: the first repository refused, the
    /// others said as notices; none if all were registered.
    pub fn refusal(&mut self) -> Option<AppError> {
        let mut urls = std::mem::take(&mut self.refused).into_iter();
        let first = urls.next()?;
        for url in urls {
            self.notices.push(Notice::with_fix(
                format!("remote not registered: {url} is reached only by reference"),
                format!("ctxloom remote create <name> {url}"),
            ));
        }
        Some(AppError::NotRegistered { url: first })
    }

    fn url_of(&self, alias: &str) -> Option<String> {
        self.registry.get(alias).map(|r| r.url)
    }

    fn walk_profile(&mut self, profile: &Profile, source: Source<'_>) {
        let mut profile = profile.clone();
        if let Source::Remote { url, sha } = source {
            for name in profile.bundles.iter_mut().chain(&mut profile.parents) {
                *name = short::within_repository(name, url, sha);
            }
            if let Err(cause) = profile.check_own_repo(url) {
                self.closure.mark_unexpanded(&profile.name);
                self.notices
                    .push(Notice::unexpanded(profile.name.clone(), &cause));
                return;
            }
        }
        for bundle in &profile.bundles {
            self.record(bundle);
        }
        for parent in &profile.parents {
            self.recurse_parent(parent);
        }
    }

    /// A reference to a bundle: noted in the closure with the commit it
    /// resolves to. None for a local or companion bundle, for a repository no
    /// remote is registered for, and for one that cannot be resolved.
    fn record(&mut self, reference: &str) -> Option<(RemoteRef, Resolved)> {
        let expanded = self.expand(reference);
        let remote = RemoteRef::parse(&expanded)?;
        if self.registry.lookup_url(&remote.url).is_none() {
            self.refused.insert(remote.url.clone());
            return None;
        }
        self.declared
            .push((remote.identity.clone(), remote.constraint.clone()));
        self.freshen(&remote.url);
        let Some(resolved) = self.resolve(&remote) else {
            // Unresolvable: skipped rather than pinned at an empty commit, but
            // its subtree is missing, so the closure is incomplete.
            self.closure.mark_unexpanded(reference);
            return None;
        };
        self.closure.record(Pin {
            identity: remote.identity.clone(),
            hash: resolved.hash.clone(),
            url: remote.url.clone(),
            constraint: remote.constraint.clone(),
            version: resolved.version.clone(),
            kind: resolved.kind,
        });
        Some((remote, resolved))
    }

    /// A short `origin/demo` spelled out; anything else as it is.
    fn expand(&self, reference: &str) -> String {
        let local = |base: &str| self.locals.bundles.contains(base);
        short::expand(reference, &|alias| self.url_of(alias), &local)
    }

    /// Bring a repository's clone up to date, once a walk (not at all when
    /// nothing is to be fetched). A failure is said, and the stale clone is
    /// read: the answer is incomplete.
    pub fn freshen(&mut self, url: &str) {
        if self.mode == Mode::Carried || self.fetched.contains_key(url) {
            return;
        }
        let outcome = ByAddress(self.deps.remotes.as_ref()).refresh(url);
        if let Err(detail) = &outcome {
            self.notices
                .push(Notice::text(format!("fetch {url}: {detail}")));
        }
        self.fetched.insert(url.to_string(), outcome);
    }

    fn lock_entry(&self, remote: &RemoteRef) -> Option<&deps::lock::Entry> {
        self.active
            .entries
            .get(&remote.identity)
            .filter(|entry| !entry.sha.is_empty())
    }

    fn locked(entry: &deps::lock::Entry) -> Resolved {
        Resolved {
            hash: entry.sha.clone(),
            version: entry.version.clone(),
            kind: Some(constraint::entry_kind(
                entry.kind,
                &entry.version,
                &entry.requested_version,
            )),
        }
    }

    /// Resolve a reference's constraint to the commit to pin, by a fixed
    /// precedence so the lock is stable yet always satisfies the manifest:
    ///
    /// 1. what the lock settles: in lock mode every existing pin is carried
    ///    whatever the manifest now says (only `upgrade` moves a pin); a held
    ///    entry is frozen in every mode;
    /// 2. a bare commit name, recorded as it is;
    /// 3. a fresh resolution against the repository's versions;
    /// 4. the last locked commit, when the resolution fails.
    fn resolve(&mut self, remote: &RemoteRef) -> Option<Resolved> {
        let key = format!("{}\0{}", remote.identity, remote.constraint);
        if let Some(done) = self.resolved.get(&key) {
            return done.clone();
        }
        let answer = self.resolution(remote);
        self.resolved.insert(key, answer.clone());
        answer
    }

    fn resolution(&mut self, remote: &RemoteRef) -> Option<Resolved> {
        let locked = self.lock_entry(remote).map(Self::locked);
        let held = self.lock_entry(remote).is_some_and(|e| e.held);
        if locked.is_some() && (held || self.mode != Mode::Upgrade) {
            return locked;
        }
        if constraint::looks_like_commit(&remote.constraint) {
            return Some(Resolved {
                hash: remote.constraint.clone(),
                version: String::new(),
                kind: Some(Kind::Sha),
            });
        }
        if self.mode == Mode::Carried {
            return None;
        }
        let view = RepoView {
            remotes: self.deps.remotes.as_ref(),
            url: &remote.url,
        };
        match constraint::resolve(&remote.constraint, &view) {
            Ok(found) => Some(Resolved {
                hash: found.sha,
                version: found.version,
                kind: Some(found.kind),
            }),
            Err(cause) => {
                if locked.is_none() {
                    self.notices.push(Notice::text(format!(
                        "could not resolve {}@{}; skipping: {cause}",
                        remote.identity, remote.constraint
                    )));
                }
                locked
            }
        }
    }

    /// A parent profile: a local one is walked; a remote one is a bundle
    /// profile (`<bundle>#profiles/<name>`), read out of its bundle at the
    /// pinned commit and walked.
    fn recurse_parent(&mut self, parent: &str) {
        let expanded = self.expand(parent);
        let Some(remote) = RemoteRef::parse(&expanded) else {
            return self.recurse_local_parent(parent);
        };
        let Some(name) = remote.item.strip_prefix("profiles/").map(str::to_string) else {
            self.closure.mark_unexpanded(parent);
            self.notices.push(Notice::text(format!(
                "could not expand remote parent profile {parent}: not a bundle-profile reference (<url>@bundles/x#profiles/y)"
            )));
            return;
        };
        let Some((rec, resolved)) = self.record(&remote.bundle_ref()) else {
            return;
        };
        let guard = format!("{}#profiles/{name}@{}", rec.identity, resolved.hash);
        if !self.visited.insert(guard) {
            return;
        }
        let Some(bundle) = self.bundle_at(&rec, &resolved.hash) else {
            self.closure.mark_unexpanded(&rec.identity);
            return;
        };
        let Some(profile) = bundle.profiles.get(&name) else {
            self.closure.mark_unexpanded(&rec.identity);
            self.notices.push(Notice::text(format!(
                "could not expand remote parent profile {}: bundle has no profile {name:?}",
                rec.identity
            )));
            return;
        };
        let mut profile = profile.clone();
        profile.name = format!("{}#profiles/{name}", rec.identity);
        self.walk_profile(
            &profile,
            Source::Remote {
                url: &rec.url,
                sha: &resolved.hash,
            },
        );
    }

    /// A local parent profile is walked in its own right (every local profile
    /// is a root); here it need only be there, or the closure is incomplete.
    fn recurse_local_parent(&mut self, parent: &str) {
        match canonical_key(parent) {
            Ok(key) if self.locals.by_key.contains_key(&key) => {}
            Ok(key) => self.closure.mark_unexpanded(&key),
            Err(_) => self.closure.mark_unexpanded(parent),
        }
    }

    /// The bundle `remote` is at `sha`, read out of the clone; none (said)
    /// when the clone cannot give it.
    fn bundle_at(&mut self, remote: &RemoteRef, sha: &str) -> Option<Bundle> {
        let key = (remote.url.clone(), sha.to_string(), remote.bundle.clone());
        if let Some(done) = self.bundles.get(&key) {
            return done.clone();
        }
        let read = self.read_bundle(remote, sha);
        self.bundles.insert(key, read.clone());
        read
    }

    fn read_bundle(&mut self, remote: &RemoteRef, sha: &str) -> Option<Bundle> {
        let tree = match ByAddress(self.deps.remotes.as_ref()).read_bundle(
            &remote.url,
            sha,
            &remote.bundle,
        ) {
            Ok(Some(tree)) => tree,
            Ok(None) => {
                self.notices.push(Notice::text(format!(
                    "could not expand remote parent profile {}: no such bundle at {}",
                    remote.identity,
                    sha.get(..7).unwrap_or(sha)
                )));
                return None;
            }
            Err(detail) => {
                self.notices.push(Notice::text(format!(
                    "could not expand remote parent profile {}: {detail}",
                    remote.identity
                )));
                return None;
            }
        };
        let files = tree.into_iter().map(|(k, f)| (k, f.data)).collect();
        match read_tree(&remote.bundle, &files) {
            Ok(bundle) => Some(bundle),
            Err(cause) => {
                self.notices
                    .push(Notice::unexpanded(remote.identity.clone(), &cause));
                None
            }
        }
    }
}
