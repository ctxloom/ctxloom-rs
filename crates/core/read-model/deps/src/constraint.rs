//! What a reference asks of a repository's versions, and the commit that
//! answers it (Go's `remote.SelectorKind`, `ResolveConstraint`).
//!
//! The selector is the optional `@<expr>` of a reference. Three modes tell the
//! story: PIN (a commit or an exact tag), VERSION (a semver range, matched
//! against tags) and TRACK (a branch tip; empty is the default branch). The
//! lock records the precise kind so "outdated" is declarative.
//!
//! A VERSION selector is a semver range as Cargo reads it, matched against the
//! tags that are semver and resolved to the newest (see [`crate::version`]); a
//! full version (`1.2.3`, `v1.2.3`, `=1.2.3`) is exactly that tag, so a pin
//! stays where it was put. What only looks like a version (`2024-05-01`) is a
//! name, a tag or a branch: nothing is refused for its version not being
//! semver, except an explicit `version:` prefix on what is not a range.

use std::fmt;

use crate::version;

/// How a selector resolves, and whether it ever moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A commit object name: pinned.
    Sha,
    /// An exact tag: pinned.
    Tag,
    /// A semver version or range, matched against tags.
    Version,
    /// A branch tip; an empty selector is the default branch.
    Branch,
}

impl Kind {
    /// The kind's name in the lockfile (`sha`, `tag`, `version`, `branch`).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Sha => "sha",
            Kind::Tag => "tag",
            Kind::Version => "version",
            Kind::Branch => "branch",
        }
    }

    /// The kind a lockfile names, if it is one.
    pub fn parse(name: &str) -> Option<Kind> {
        [Kind::Sha, Kind::Tag, Kind::Version, Kind::Branch]
            .into_iter()
            .find(|kind| kind.name() == name)
    }

    /// Whether the kind is fixed: a commit or an exact tag never goes
    /// outdated.
    pub fn is_pin(self) -> bool {
        matches!(self, Kind::Sha | Kind::Tag)
    }
}

/// Whether `s` is a bare commit name: 7 to 40 lowercase hex digits. Such an
/// expression is already concrete.
pub fn looks_like_commit(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The kind an expression has by its shape alone: an empty one tracks the
/// default branch, a commit name is a commit, one that is a semver range a
/// version; a bare name (`main`, `release`, `2024-05-01`) is a tag or a branch
/// only the repository can tell (None).
pub fn infer_kind(bare: &str) -> Option<Kind> {
    if bare.is_empty() {
        Some(Kind::Branch)
    } else if looks_like_commit(bare) {
        Some(Kind::Sha)
    } else if version::range(bare).is_some() {
        Some(Kind::Version)
    } else {
        None
    }
}

/// An explicit `sha:`, `tag:`, `version:` or `branch:` prefix, which overrides
/// the shape, and the expression after it.
pub fn split_prefix(expr: &str) -> (Option<Kind>, &str) {
    for kind in [Kind::Sha, Kind::Tag, Kind::Version, Kind::Branch] {
        let prefix = format!("{}:", kind.name());
        if let Some(rest) = expr.strip_prefix(&prefix) {
            return (Some(kind), rest);
        }
    }
    (None, expr)
}

/// The kind a lock entry has: the one it records; else a version when it
/// records a tag it chose; else the one its requested expression has by shape;
/// else a branch (the safe, re-resolving default).
pub fn entry_kind(recorded: Option<Kind>, version: &str, requested: &str) -> Kind {
    if let Some(kind) = recorded {
        return kind;
    }
    if !version.is_empty() {
        return Kind::Version;
    }
    infer_kind(requested).unwrap_or(Kind::Branch)
}

/// A repository's versions, as a resolution reads them.
pub trait RepoVersions {
    /// The repository's tag names.
    fn tags(&self) -> Result<Vec<String>, String>;
    /// A tag, branch or commit name as the commit it is (a branch from the
    /// last fetch's remote-tracking ref, never the clone's checked-out one).
    fn resolve_ref(&self, name: &str) -> Result<String, String>;
    /// A tag name as its commit, through the tags alone.
    fn resolve_tag(&self, name: &str) -> Result<String, String>;
    /// The default branch's name.
    fn default_branch(&self) -> Result<String, String>;
}

/// The outcome of a resolution: the commit to lock, the tag chosen (empty for
/// a commit or a branch), and the kind.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Resolution {
    pub sha: String,
    pub version: String,
    pub kind: Kind,
}

/// Why an expression could not be resolved.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResolveError {
    /// The repository could not answer; what it said.
    Repository { what: String, cause: String },
    /// An expression forced to be a version (`version:`) that is not a range.
    Range { expr: String },
    /// No tag is that version.
    NoTag { expr: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::Repository { what, cause } => write!(f, "{what}: {cause}"),
            ResolveError::Range { expr } => {
                write!(f, "\"{expr}\" is not a version or a version range")
            }
            ResolveError::NoTag { expr } => {
                write!(f, "no tag satisfies version constraint \"{expr}\"")
            }
        }
    }
}

/// Resolve `expr` against `repo`: the single home of the selector to commit
/// policy. The commit returned always satisfies the selector.
pub fn resolve(expr: &str, repo: &dyn RepoVersions) -> Result<Resolution, ResolveError> {
    let (forced, bare) = split_prefix(expr);
    match forced.or_else(|| infer_kind(bare)) {
        Some(Kind::Branch) => resolve_branch(bare, repo),
        Some(Kind::Sha) => {
            let sha = repo
                .resolve_ref(bare)
                .map_err(failed(format!("resolve commit \"{bare}\"")))?;
            Ok(Resolution {
                sha,
                version: String::new(),
                kind: Kind::Sha,
            })
        }
        Some(Kind::Version) => resolve_version(bare, repo),
        Some(Kind::Tag) => resolve_tag(bare, repo),
        None => resolve_name(bare, repo),
    }
}

fn failed(what: String) -> impl FnOnce(String) -> ResolveError {
    move |cause| ResolveError::Repository { what, cause }
}

/// The tip of a branch; an empty name is the default branch's.
fn resolve_branch(name: &str, repo: &dyn RepoVersions) -> Result<Resolution, ResolveError> {
    let name = if name.is_empty() {
        repo.default_branch()
            .map_err(failed("determine default branch".to_string()))?
    } else {
        name.to_string()
    };
    let sha = repo
        .resolve_ref(&name)
        .map_err(failed(format!("resolve branch \"{name}\"")))?;
    Ok(Resolution {
        sha,
        version: String::new(),
        kind: Kind::Branch,
    })
}

/// An exact tag, through the tag namespace alone.
fn resolve_tag(name: &str, repo: &dyn RepoVersions) -> Result<Resolution, ResolveError> {
    let sha = repo
        .resolve_tag(name)
        .map_err(failed(format!("resolve tag \"{name}\"")))?;
    Ok(Resolution {
        sha,
        version: name.to_string(),
        kind: Kind::Tag,
    })
}

/// A bare name of no inferable shape: a tag wins (a pin), else a branch (a
/// track). A failed tag listing is reported, never read as "not a tag": that
/// would turn a tag pin into branch tracking behind the user's back.
fn resolve_name(name: &str, repo: &dyn RepoVersions) -> Result<Resolution, ResolveError> {
    let tags = repo
        .tags()
        .map_err(failed(format!("determine whether \"{name}\" is a tag")))?;
    if tags.iter().any(|tag| tag == name) {
        resolve_tag(name, repo)
    } else {
        resolve_branch(name, repo)
    }
}

/// A version range as the newest tag that satisfies it; what is not a range
/// is refused, and a range no tag satisfies finds none.
fn resolve_version(expr: &str, repo: &dyn RepoVersions) -> Result<Resolution, ResolveError> {
    let range = version::range(expr).ok_or_else(|| ResolveError::Range {
        expr: expr.to_string(),
    })?;
    let tags = repo.tags().map_err(failed("list tags".to_string()))?;
    let tag = version::newest_matching(&range, &tags).ok_or_else(|| ResolveError::NoTag {
        expr: expr.to_string(),
    })?;
    let sha = repo
        .resolve_tag(&tag)
        .map_err(failed(format!("resolve matched tag \"{tag}\"")))?;
    Ok(Resolution {
        sha,
        version: tag,
        kind: Kind::Version,
    })
}

#[cfg(test)]
#[path = "constraint.test.rs"]
mod tests;
