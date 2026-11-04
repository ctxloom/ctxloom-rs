//! `lock.yaml`, schema 3: every dependency pinned to a commit (Go's
//! `remote.Lockfile` and `LockfileManager`).
//!
//! The lock records no time, so it changes only when a pin, a URL, a kind or a
//! requested version does: a pull at unchanged pins rewrites it byte for byte.
//! A lock that cannot be read is refused and never overwritten: the pins and
//! holds in it are not ours to replace.

use std::collections::BTreeMap;

use angzarr_diagnostic::{Diagnostic, Report};
use angzarr_yaml::versioned::{remove_key, LoadError, LoadProblem, Loader, Migrations};
use angzarr_yaml::{from_value, Value};
use library::refs::Parts;
use serde::Deserialize;

use crate::constraint::Kind;
use crate::DepsError;

/// The generation this build writes and the newest it reads.
pub const VERSION: u32 = 3;

/// One locked dependency.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Entry {
    /// The commit.
    pub sha: String,
    /// The repository's address.
    pub url: String,
    /// The expression the manifest asked for (empty: the default branch).
    pub requested_version: String,
    /// The tag a version or tag selector chose.
    pub version: String,
    /// The kind of the selector; none for an entry written before it was.
    pub kind: Option<Kind>,
    /// Frozen against `deps upgrade`.
    pub held: bool,
}

/// The lock: entries by bundle identity.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Lock {
    pub entries: BTreeMap<String, Entry>,
}

impl Lock {
    /// Read the lock `text` of the file `path`, migrated in memory to the
    /// current generation. An empty file (no document), an unstamped or
    /// older-than-2 or newer one, a field the format does not model, and an
    /// entry not keyed by its own bundle identity are refused.
    pub fn decode(text: &str, path: &str) -> Result<Lock, DepsError> {
        let unreadable = |e: &dyn Diagnostic| DepsError::Unreadable {
            path: path.to_string(),
            cause: Report::of(e),
        };
        let parsed = angzarr_yaml::parse(text).map_err(|e| unreadable(&e))?;
        if is_documentless(&parsed) {
            return Err(DepsError::LockEmpty {
                path: path.to_string(),
            });
        }
        let loader = loader()?;
        let doc = loader
            .migrate(Some(path), parsed)
            .map_err(|e| load_refusal(e, path))?;
        let raw: Raw = from_value(&doc).map_err(|e| unreadable(&e))?;
        raw.lock(path)
    }

    /// The lock as it is written: the generation, then the entries by
    /// identity, each field set only when it says something.
    pub fn encode(&self) -> String {
        let mut root = vec![(
            "schema_version".to_string(),
            Value::scalar(VERSION.to_string()),
        )];
        if !self.entries.is_empty() {
            let bundles = self
                .entries
                .iter()
                .map(|(key, entry)| (key.clone(), entry.value()))
                .collect();
            root.push(("bundles".to_string(), Value::Map(bundles)));
        }
        angzarr_yaml::write(&Value::Map(root), usize::MAX)
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The refusals a write of `incoming` over the file now at `path` (its text,
/// or none where there is no file) meets: a newer lock is not ours to
/// downgrade, an unreadable one is never overwritten, and an empty lock never
/// replaces a populated one unless emptying it is `allow_empty`'s intent.
pub fn guard_write(
    current: Option<&str>,
    incoming: &Lock,
    allow_empty: bool,
    path: &str,
) -> Result<(), DepsError> {
    let Some(text) = current else {
        return Ok(());
    };
    let doc = angzarr_yaml::parse(text).map_err(|e| DepsError::LockUnreadable {
        path: path.to_string(),
        detail: e.to_string(),
    })?;
    if let Ok(found) = loader()?.version_of(&doc) {
        if found > VERSION {
            return Err(DepsError::TooNew {
                path: path.to_string(),
                kind: KIND.to_string(),
                found: u64::from(found),
                current: u64::from(VERSION),
            });
        }
    }
    let held = match doc.get("bundles") {
        Some(Value::Map(entries)) => entries.len(),
        _ => 0,
    };
    if held == 0 || !incoming.is_empty() || allow_empty {
        return Ok(());
    }
    Err(DepsError::LockWouldErase {
        path: path.to_string(),
        count: held as u64,
    })
}

/// What the lock's kind is called in an error.
const KIND: &str = "lockfile";

/// The loader of lockfiles: generation 2 to 3 stopped recording when the lock
/// was written (`locked_at`) and when each bundle was fetched (`fetched_at`),
/// which changed on every pull at unchanged pins.
fn loader() -> Result<Loader, DepsError> {
    Migrations::new(KIND, VERSION)
        .step(2, |doc| {
            remove_key(doc, "locked_at")?;
            if let Some(Value::Map(entries)) = doc.get_mut("bundles") {
                for (_, entry) in entries.iter_mut() {
                    remove_key(entry, "fetched_at")?;
                }
            }
            Ok(())
        })
        .build()
        .map_err(|e| DepsError::Unreadable {
            path: String::new(),
            cause: Report::of(&e),
        })
}

/// A load that did not happen, as this crate says it.
fn load_refusal(error: LoadError, path: &str) -> DepsError {
    match error.problem {
        LoadProblem::TooNew { found, current } => DepsError::TooNew {
            path: path.to_string(),
            kind: KIND.to_string(),
            found: u64::from(found),
            current: u64::from(current),
        },
        LoadProblem::TooOld { found, oldest } => DepsError::TooOld {
            path: path.to_string(),
            kind: KIND.to_string(),
            found: u64::from(found),
            oldest: u64::from(oldest),
        },
        _ => DepsError::Unreadable {
            path: path.to_string(),
            cause: Report::of(&error),
        },
    }
}

/// Whether a parsed text holds no document: empty, or only comments, or an
/// explicit null.
fn is_documentless(doc: &Value) -> bool {
    match doc {
        Value::Null => true,
        Value::Scalar(s) => matches!(s.trim(), "null" | "Null" | "NULL" | "~"),
        _ => false,
    }
}

/// The lockfile as the current generation decodes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[allow(dead_code, reason = "the stamp the loader leaves")]
    schema_version: u32,
    bundles: Option<BTreeMap<String, RawEntry>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    sha: Option<String>,
    url: Option<String>,
    requested_version: Option<String>,
    version: Option<String>,
    kind: Option<String>,
    held: Option<bool>,
}

impl Raw {
    fn lock(self, path: &str) -> Result<Lock, DepsError> {
        let mut entries = BTreeMap::new();
        for (key, raw) in self.bundles.unwrap_or_default() {
            if !is_identity(&key) {
                return Err(DepsError::LockKey {
                    path: path.to_string(),
                    key,
                });
            }
            let kind = match raw.kind.as_deref().filter(|k| !k.is_empty()) {
                None => None,
                Some(name) => Some(Kind::parse(name).ok_or_else(|| DepsError::BadShape {
                    path: path.to_string(),
                    key: key.clone(),
                    detail: format!("unknown kind \"{name}\" (want sha, tag, version or branch)"),
                })?),
            };
            entries.insert(
                key,
                Entry {
                    sha: raw.sha.unwrap_or_default(),
                    url: raw.url.unwrap_or_default(),
                    requested_version: raw.requested_version.unwrap_or_default(),
                    version: raw.version.unwrap_or_default(),
                    kind,
                    held: raw.held.unwrap_or(false),
                },
            );
        }
        Ok(Lock { entries })
    }
}

/// Whether `key` is exactly the bundle identity it parses to: a lookup is by
/// identity, so an entry under anything else is one no lookup reaches.
fn is_identity(key: &str) -> bool {
    Parts::parse(key).is_ok_and(|parts| parts.render(false) == key)
}

impl Entry {
    /// The entry as a mapping: `sha` and `url` always, the rest when it says
    /// something.
    fn value(&self) -> Value {
        let mut fields = vec![
            ("sha".to_string(), Value::scalar(self.sha.clone())),
            ("url".to_string(), Value::scalar(self.url.clone())),
        ];
        for (name, text) in [
            ("requested_version", &self.requested_version),
            ("version", &self.version),
        ] {
            if !text.is_empty() {
                fields.push((name.to_string(), Value::scalar(text.clone())));
            }
        }
        if let Some(kind) = self.kind {
            fields.push(("kind".to_string(), Value::scalar(kind.name())));
        }
        if self.held {
            fields.push(("held".to_string(), Value::scalar("true")));
        }
        Value::Map(fields)
    }
}

#[cfg(test)]
#[path = "lock.test.rs"]
mod tests;
