//! SemVer 2.0.0 (semver.org), strictly: what a tag schema's `type=semver`
//! admits at write time and orders by at query time. Strict as Go's
//! `Masterminds/semver` `StrictNewVersion`: exactly `MAJOR.MINOR.PATCH`, no
//! `v` prefix, no leading zero in a number, then an optional pre-release
//! (`-`) and build metadata (`+`).

use std::cmp::Ordering;

/// A version, as precedence (§11) needs it: build metadata is kept out,
/// since it takes no part.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Version {
    core: [u64; 3],
    pre: Vec<Identifier>,
}

/// One dot-separated pre-release identifier.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Identifier {
    Numeric(u64),
    Alphanumeric(String),
}

impl Version {
    /// `s` as a version, or why it is not one.
    pub fn parse(s: &str) -> Result<Version, String> {
        let (rest, build) = match s.split_once('+') {
            Some((rest, build)) => (rest, Some(build)),
            None => (s, None),
        };
        if let Some(build) = build {
            identifiers(build, "build metadata")?;
        }
        let (core, pre) = match rest.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (rest, None),
        };
        let numbers: Vec<&str> = core.split('.').collect();
        let [major, minor, patch] = numbers[..] else {
            return Err(format!("{s:?} is not MAJOR.MINOR.PATCH"));
        };
        let core = [number(major)?, number(minor)?, number(patch)?];
        let pre = match pre {
            None => Vec::new(),
            Some(pre) => identifiers(pre, "pre-release")?
                .into_iter()
                .map(|id| match number(id) {
                    Ok(n) => Ok(Identifier::Numeric(n)),
                    Err(_) if id.bytes().all(|b| b.is_ascii_digit()) => {
                        Err(format!("pre-release identifier {id:?} has a leading zero"))
                    }
                    Err(_) => Ok(Identifier::Alphanumeric(id.to_string())),
                })
                .collect::<Result<_, _>>()?,
        };
        Ok(Version { core, pre })
    }
}

/// Precedence (§11): the numbers, then a release above any of its
/// pre-releases, then the pre-release identifiers one by one.
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Numeric identifiers below alphanumeric ones; each kind in its own
/// order (§11.4).
impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Identifier::Numeric(a), Identifier::Numeric(b)) => a.cmp(b),
            (Identifier::Numeric(_), Identifier::Alphanumeric(_)) => Ordering::Less,
            (Identifier::Alphanumeric(_), Identifier::Numeric(_)) => Ordering::Greater,
            (Identifier::Alphanumeric(a), Identifier::Alphanumeric(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A number: digits, with no leading zero unless it is `0`.
fn number(s: &str) -> Result<u64, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{s:?} is not a number"));
    }
    if s.len() > 1 && s.starts_with('0') {
        return Err(format!("{s:?} has a leading zero"));
    }
    s.parse().map_err(|_| format!("{s:?} is too large"))
}

/// The dot-separated identifiers of `s`: each non-empty, of `[0-9A-Za-z-]`.
fn identifiers<'a>(s: &'a str, what: &str) -> Result<Vec<&'a str>, String> {
    s.split('.')
        .map(|id| {
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                Err(format!("{what} identifier {id:?} is not [0-9A-Za-z-]+"))
            } else {
                Ok(id)
            }
        })
        .collect()
}

/// The comparator a tag schema's `type=semver` registers with tagma's
/// index: precedence, and not comparable when either side is no version.
#[derive(Clone, Copy, Debug, Default)]
pub struct Semver;

impl tagma_core::TypeComparator for Semver {
    fn compare(&self, a: &str, b: &str) -> Option<Ordering> {
        Some(Version::parse(a).ok()?.cmp(&Version::parse(b).ok()?))
    }
}

#[cfg(test)]
#[path = "semver.test.rs"]
mod tests;
