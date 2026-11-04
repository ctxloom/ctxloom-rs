//! Versions, where they are semver, and what a move between two is worth.
//!
//! ctxloom uses semver, but a bundle and its users are not required to: a tag
//! or a manifest's `version` that parses is compared by the semver spec, one
//! that does not is compared as the text it is, and any difference between
//! two such is evidence of a possible break, never a refusal.

use semver::{Version, VersionReq};
use serde::Serialize;

/// What a move between two versions is worth to whoever depends on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Compat {
    /// Across a compatibility line: the major from 1.0.0, the minor of a
    /// 0.y.z, the patch of a 0.0.z (Cargo's caret rule).
    Breaking,
    /// Up within a line.
    Compatible,
    /// Nothing says: a version that is not semver, a pre-release, a move down
    /// a line, or no change of version at all.
    Unknown,
}

impl Compat {
    /// The word it is shown and serialized as.
    pub fn name(self) -> &'static str {
        match self {
            Compat::Breaking => "breaking",
            Compat::Compatible => "compatible",
            Compat::Unknown => "unknown",
        }
    }
}

/// `tag` as a semver version, after a `v` prefix. Semver has three numbers:
/// a `v1` or a `1.2` is a name, not a version.
pub fn parse(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(['v', 'V']).unwrap_or(tag)).ok()
}

/// What `expr` asks of versions: a full version (`1.2.3`, `v1.2.3`) is exactly
/// that one, so a pin stays where it was put; anything else is a range as
/// Cargo reads it (`^1.2`, `~1.2`, `>=1, <2`, `1.x`, `*`, and a short `1.2` as
/// `^1.2`). None where it is neither.
pub fn range(expr: &str) -> Option<VersionReq> {
    let expr = expr.trim();
    let bare = expr.strip_prefix(['v', 'V']).unwrap_or(expr);
    match Version::parse(bare) {
        Ok(version) => VersionReq::parse(&format!("={version}")).ok(),
        Err(_) => VersionReq::parse(bare).ok(),
    }
}

/// The compatibility line a version is on: what a bump of it may not leave.
/// From 1.0.0 that is the major; below it the minor; below 0.1.0 the patch
/// (Cargo's caret rule).
fn line(version: &Version) -> (u64, u64, u64) {
    match (version.major, version.minor) {
        (0, 0) => (0, 0, version.patch),
        (0, minor) => (0, minor, 0),
        (major, _) => (major, 0, 0),
    }
}

/// The move from `from` to `to`: across a line is breaking, up within one is
/// compatible, and anything else (a name that is not semver, a pre-release, a
/// move down a line, the same version) says nothing.
pub fn classify(from: &str, to: &str) -> Compat {
    let (Some(from), Some(to)) = (parse(from), parse(to)) else {
        return Compat::Unknown;
    };
    if !from.pre.is_empty() || !to.pre.is_empty() {
        return Compat::Unknown;
    }
    if line(&from) != line(&to) {
        Compat::Breaking
    } else if to.cmp_precedence(&from).is_gt() {
        Compat::Compatible
    } else {
        Compat::Unknown
    }
}

/// The tag, in the form it was written, of the highest semver version among
/// `tags` that `range` allows; tags that are not semver are not candidates.
pub fn newest_matching(range: &VersionReq, tags: &[String]) -> Option<String> {
    tags.iter()
        .filter_map(|tag| parse(tag).map(|version| (version, tag)))
        .filter(|(version, _)| range.matches(version))
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)))
        .map(|(_, tag)| tag.clone())
}

#[cfg(test)]
#[path = "version.test.rs"]
mod tests;
