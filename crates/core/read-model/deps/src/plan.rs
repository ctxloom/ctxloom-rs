//! The decisions of a pull, a check and an upgrade, as functions of what they
//! were shown: the closure's pins, the lock they rebuild, what an upgrade
//! moves and what upstream's absence proves.
//!
//! THE EVIDENCE RULE. Content is treated as deleted upstream only on a
//! not-found answer from a repository this run has separately proved it can
//! read. Reachability is proved per repository, once, before any item is asked
//! about; a repository that fails the proof has none of its dependencies
//! touched, and its items are never probed. The cost is a false negative (a
//! remote that is down keeps content installed that was withdrawn), which is
//! the right way round: an emptied installation reported as a successful sync
//! has no second chance.

use std::collections::{BTreeMap, BTreeSet};

use crate::constraint::Kind;
use crate::lock::{Entry, Lock};
use crate::reference::RemoteRef;

/// One resolved dependency of the closure: a manifest reference whose
/// constraint has been resolved to a commit.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pin {
    /// The lock key.
    pub identity: String,
    /// The commit.
    pub hash: String,
    pub url: String,
    /// The manifest's expression (empty: the default branch).
    pub constraint: String,
    /// The tag a version or tag selector chose.
    pub version: String,
    pub kind: Option<Kind>,
}

impl Pin {
    /// The lock entry this pin resolves to.
    pub fn entry(&self) -> Entry {
        Entry {
            sha: self.hash.clone(),
            url: self.url.clone(),
            requested_version: self.constraint.clone(),
            version: self.version.clone(),
            kind: self.kind,
            held: false,
        }
    }
}

/// The same item referenced at two or more commits.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Conflict {
    pub item: String,
    /// The differing commits, sorted.
    pub hashes: Vec<String>,
}

/// The pins a walk of the profiles has found.
#[derive(Clone, Default, Debug)]
pub struct Closure {
    pins: BTreeMap<String, Pin>,
    hashes: BTreeMap<String, BTreeSet<String>>,
    unexpanded: BTreeSet<String>,
}

impl Closure {
    pub fn new() -> Closure {
        Closure::default()
    }

    /// Note a dependency. The first pin of an identity stays; every commit it
    /// is referenced at is remembered, for a conflict.
    pub fn record(&mut self, pin: Pin) {
        self.hashes
            .entry(pin.identity.clone())
            .or_default()
            .insert(pin.hash.clone());
        self.pins.entry(pin.identity.clone()).or_insert(pin);
    }

    /// Note a part of the closure that could not be read: its subtree is
    /// missing from the pins, so the closure is incomplete.
    pub fn mark_unexpanded(&mut self, what: &str) {
        self.unexpanded.insert(what.to_string());
    }

    /// Every pin, by identity.
    pub fn pins(&self) -> Vec<Pin> {
        self.pins.values().cloned().collect()
    }

    /// The items referenced at more than one commit, by item.
    pub fn conflicts(&self) -> Vec<Conflict> {
        self.hashes
            .iter()
            .filter(|(_, hashes)| hashes.len() > 1)
            .map(|(item, hashes)| Conflict {
                item: item.clone(),
                hashes: hashes.iter().cloned().collect(),
            })
            .collect()
    }

    /// What could not be read, sorted.
    pub fn unexpanded(&self) -> Vec<String> {
        self.unexpanded.iter().cloned().collect()
    }
}

/// The conflicts as one message, a commit shortened to seven characters.
pub fn conflict_text(conflicts: &[Conflict]) -> String {
    if conflicts.is_empty() {
        return String::new();
    }
    let mut text = String::from(
        "dependency hash conflict \u{2014} the same item is referenced at differing commits:",
    );
    for conflict in conflicts {
        let shown: Vec<&str> = conflict
            .hashes
            .iter()
            .map(|hash| hash.get(..7).unwrap_or(hash))
            .collect();
        text.push_str(&format!("\n  {} @ {{{}}}", conflict.item, shown.join(", ")));
    }
    text
}

/// The lock a rebuild writes: each pin as it resolved, carrying what the
/// previous entry of the identity must keep: the hold always, and the
/// requested expression while the commit is the same (only an upgrade
/// applies a changed constraint).
pub fn relock(previous: &Lock, pins: &[Pin]) -> Lock {
    let mut next = Lock::default();
    for pin in pins {
        let mut entry = pin.entry();
        if let Some(prior) = previous.entries.get(&pin.identity) {
            entry.held = prior.held;
            if prior.sha == pin.hash {
                entry.requested_version.clone_from(&prior.requested_version);
            }
        }
        next.entries.insert(pin.identity.clone(), entry);
    }
    next
}

/// Carry every entry of `active` that `next` lacks into `next`: what a walk
/// could not reach is unreached, not removed. How many were carried.
pub fn preserve_unreached(active: &Lock, next: &mut Lock) -> usize {
    let mut carried = 0;
    for (identity, entry) in &active.entries {
        if !next.entries.contains_key(identity) {
            next.entries.insert(identity.clone(), entry.clone());
            carried += 1;
        }
    }
    carried
}

/// The identities `before` holds and `after` no longer does, sorted.
pub fn dropped(before: &Lock, after: &Lock) -> Vec<String> {
    before
        .entries
        .keys()
        .filter(|identity| !after.entries.contains_key(*identity))
        .cloned()
        .collect()
}

/// What an upgrade round decided.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Upgrade {
    /// The lock the round would write.
    pub next: Lock,
    /// The pins that move, or are first made, by identity.
    pub moved: Vec<Pin>,
    /// The identities the lock drops: nothing the project composes reaches
    /// them any more.
    pub removed: Vec<String>,
    /// Part of the closure was not reached: no move is not "all current".
    pub incomplete: bool,
    /// Nothing is declared and nothing is locked: a different fact from
    /// "everything is current".
    pub nothing_declared: bool,
}

/// Decide an upgrade of `active` to the `proposed` pins (each the newest
/// commit its constraint allows). A held entry stays as it is. When part of
/// the closure was not read (`unexpanded` parts) the entries it owned are
/// carried. A repository that could not be fetched (`fetch_failed`) was read
/// from its stale clone, so the round is incomplete too.
pub fn upgrade(active: &Lock, proposed: &[Pin], unexpanded: usize, fetch_failed: bool) -> Upgrade {
    let mut next = Lock::default();
    let mut moved = Vec::new();
    for pin in proposed {
        let current = active.entries.get(&pin.identity);
        if let Some(held) = current.filter(|entry| entry.held) {
            next.entries.insert(pin.identity.clone(), held.clone());
            continue;
        }
        if current.is_none_or(|entry| entry.sha != pin.hash) {
            moved.push(pin.clone());
        }
        next.entries.insert(pin.identity.clone(), pin.entry());
    }
    moved.sort_by(|a, b| a.identity.cmp(&b.identity));
    if unexpanded > 0 {
        preserve_unreached(active, &mut next);
    }
    let incomplete = unexpanded > 0 || fetch_failed;
    Upgrade {
        removed: dropped(active, &next),
        incomplete,
        nothing_declared: next.is_empty() && active.is_empty() && !incomplete,
        next,
        moved,
    }
}

/// A pin kept at its commit although the manifest now asks for something else.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConstraintChange {
    pub identity: String,
    /// The expression the pin was resolved from.
    pub pinned: String,
    /// What the manifest asks for now.
    pub declared: String,
    pub sha: String,
}

/// The unheld pins of `lock` whose declared expression (`(identity,
/// expression)` for each reference in the closure) is no longer the one they
/// were resolved from. A declared commit equal to the pin is satisfied.
pub fn constraint_changes(lock: &Lock, declared: &[(String, String)]) -> Vec<ConstraintChange> {
    let asked: BTreeSet<&(String, String)> = declared.iter().collect();
    asked
        .into_iter()
        .filter_map(|(identity, expression)| {
            let entry = lock.entries.get(identity)?;
            let kept =
                entry.held || entry.requested_version == *expression || entry.sha == *expression;
            (!kept).then(|| ConstraintChange {
                identity: identity.clone(),
                pinned: entry.requested_version.clone(),
                declared: expression.clone(),
                sha: entry.sha.clone(),
            })
        })
        .collect()
}

/// What asking one reference's content of a repository that could be read
/// answered.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Probe {
    /// Still served.
    Present,
    /// Not found: a fact about the repository.
    Gone,
    /// The lookup failed: a fact about the attempt, which says nothing about
    /// what exists.
    Failed(String),
}

/// A repository whose state could not be established, and what that cost.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unchecked {
    pub url: String,
    pub refs: Vec<String>,
    pub reason: String,
}

/// What a reconcile decided.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Reconcile {
    /// Installed references upstream demonstrably no longer serves.
    pub gone: Vec<String>,
    /// Repositories whose state could not be established; their dependencies
    /// are left exactly as they are.
    pub unreachable: Vec<Unchecked>,
}

/// Decide, for every installed reference, whether upstream stopped serving it,
/// under the evidence rule. `reach` proves a repository readable this run
/// (any error means it could not be); `content` asks about one reference's own
/// path, never a listing.
pub fn reconcile(
    installed: &[String],
    reach: &mut dyn FnMut(&str) -> Result<(), String>,
    content: &mut dyn FnMut(&str) -> Probe,
) -> Reconcile {
    let mut by_repository: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut unparseable = Vec::new();
    for reference in installed {
        match RemoteRef::parse(reference) {
            Some(remote) => by_repository
                .entry(remote.url)
                .or_default()
                .push(reference.clone()),
            None => unparseable.push(reference.clone()),
        }
    }
    let mut plan = Reconcile::default();
    if !unparseable.is_empty() {
        // No repository to prove readable, so nothing this reference could
        // be authority about: reported, never removed.
        plan.unreachable.push(Unchecked {
            url: String::new(),
            refs: unparseable,
            reason: "the reference could not be parsed, so no repository could be checked for it"
                .to_string(),
        });
    }
    for (url, refs) in by_repository {
        if let Err(reason) = reach(&url) {
            plan.unreachable.push(Unchecked { url, refs, reason });
            continue;
        }
        for reference in refs {
            match content(&reference) {
                Probe::Present => {}
                Probe::Gone => plan.gone.push(reference),
                Probe::Failed(reason) => plan.unreachable.push(Unchecked {
                    url: url.clone(),
                    refs: vec![reference],
                    reason,
                }),
            }
        }
    }
    plan
}

#[cfg(test)]
#[path = "plan.test.rs"]
mod tests;
