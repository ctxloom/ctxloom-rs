//! `remotes.yaml`, schema 1: the repositories a project may draw bundles from,
//! which of them is the default, and the forges they resolve through (Go's
//! `remote.Registry`).
//!
//! Registering a remote is the trust act: its content reaches the assistant
//! because the project added it, and publishing to it needs no second
//! blessing. The registry records an address; it installs nothing and holds
//! no credential.
//!
//! The file is a person's to edit and commit. A key ctxloom does not manage
//! (a `note:`, a field of a remote it does not know) is the author's, so every
//! rewrite keeps it.

use angzarr_diagnostic::Report;
use angzarr_yaml::versioned::{LoadProblem, Migrations};
use angzarr_yaml::Value;

use crate::address::{same_repository, stored_url};
use crate::DepsError;

/// The forges that need no `forges:` entry.
pub const BUILT_IN_FORGES: [&str; 2] = ["github", "git"];

/// The generation this build writes and reads.
pub const VERSION: u32 = 1;

/// One registered remote.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Remote {
    pub name: String,
    /// The address, as it is fetched from.
    pub url: String,
    /// The forge label it is bound to; empty resolves by the URL's host.
    pub forge: String,
}

/// The fields of a remote an edit may change; none is unchanged. A forge of
/// `Some("")` is a value (resolve by host), not an omission.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Edit {
    pub name: Option<String>,
    pub url: Option<String>,
    pub forge: Option<String>,
}

/// What an edit did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Edited {
    pub before: Remote,
    pub after: Remote,
    /// The fields that really moved, of `name`, `url`, `forge`.
    pub changed: Vec<&'static str>,
    /// A rename that carried the default with it.
    pub default_followed: bool,
}

/// One forge: how a remote bound to it is reached (Go's `ForgeConfig`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ForgeEntry {
    pub label: String,
    /// `github` or `git`; a configured forge that names no type is `git`.
    pub kind: String,
    /// The server it serves, by which a remote with no forge is matched.
    pub base_url: String,
    /// An explicit GitHub REST API endpoint (Enterprise); empty is the public
    /// one.
    pub api_url: String,
    /// The environment variable its token is read from; never the token.
    pub token_env: String,
}

/// The registry: the document as read, so what is not ctxloom's survives.
#[derive(Clone, PartialEq, Debug)]
pub struct Registry {
    doc: angzarr_yaml::Value,
}

impl Registry {
    /// A registry with nothing in it.
    pub fn empty() -> Registry {
        Registry {
            doc: Value::Map(Vec::new()),
        }
    }

    /// Read the registry `text` of the file `path`. A generation other than
    /// 1 (unstamped is 0) is refused, as is a forge of an unknown type and a
    /// remote without a `url`.
    pub fn decode(text: &str, path: &str) -> Result<Registry, DepsError> {
        let loader = Migrations::new(KIND, VERSION)
            .build()
            .map_err(|e| DepsError::Unreadable {
                path: path.to_string(),
                cause: Report::of(&e),
            })?;
        let parsed = angzarr_yaml::parse(text).map_err(|e| DepsError::Unreadable {
            path: path.to_string(),
            cause: Report::of(&e),
        })?;
        // D15: a registry that declares no generation is refused, naming 0;
        // the loader reads an unstamped document of a kind with no steps as
        // current.
        if parsed.get("schema_version").is_none() {
            return Err(DepsError::TooOld {
                path: path.to_string(),
                kind: KIND.to_string(),
                found: 0,
                oldest: u64::from(VERSION),
            });
        }
        let doc = loader
            .migrate(Some(path), parsed)
            .map_err(|error| match error.problem {
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
            })?;
        check(&doc, path)?;
        Ok(Registry { doc })
    }

    /// The registry as it is written: the keys sorted, the stamp among them.
    pub fn encode(&self) -> String {
        let mut root: Vec<(String, Value)> = Vec::new();
        for (key, value) in self.doc.as_map().unwrap_or_default() {
            let keep = match key.as_str() {
                "schema_version" => false,
                "remotes" | "forges" => value.as_map().is_some_and(|m| !m.is_empty()),
                "default" => value.as_str().is_some_and(|d| !d.is_empty()),
                _ => true,
            };
            if keep {
                root.push((key.clone(), sorted(key, value)));
            }
        }
        root.push((
            "schema_version".to_string(),
            Value::scalar(VERSION.to_string()),
        ));
        root.sort_by(|a, b| a.0.cmp(&b.0));
        angzarr_yaml::write(&Value::Map(root), usize::MAX)
    }

    /// Every remote, by name.
    pub fn remotes(&self) -> Vec<Remote> {
        let mut all: Vec<Remote> = self
            .doc
            .get("remotes")
            .and_then(Value::as_map)
            .unwrap_or_default()
            .iter()
            .map(|(name, entry)| remote_of(name, entry))
            .collect();
        all.sort_by(|a, b| a.name.cmp(&b.name));
        all
    }

    /// The remote named `name`.
    pub fn get(&self, name: &str) -> Option<Remote> {
        self.doc
            .get("remotes")
            .and_then(|remotes| remotes.get(name))
            .map(|entry| remote_of(name, entry))
    }

    /// The default remote's name; empty for none.
    pub fn default_name(&self) -> String {
        self.doc
            .get("default")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    /// The remote registered for the repository `url` names, in any spelling.
    pub fn lookup_url(&self, url: &str) -> Option<Remote> {
        self.remotes()
            .into_iter()
            .find(|remote| same_repository(&remote.url, url))
    }

    /// The forge labels there are: the built-in ones and the configured ones.
    pub fn forge_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = BUILT_IN_FORGES.iter().map(|f| f.to_string()).collect();
        for (label, _) in self
            .doc
            .get("forges")
            .and_then(Value::as_map)
            .unwrap_or_default()
        {
            if !labels.contains(label) {
                labels.push(label.clone());
            }
        }
        labels.sort();
        labels
    }

    /// Every forge there is, by label: the built-in ones, and the configured
    /// ones, which replace a built-in of the same label whole.
    pub fn forges(&self) -> Vec<ForgeEntry> {
        let configured = self
            .doc
            .get("forges")
            .and_then(Value::as_map)
            .unwrap_or_default();
        let text = |entry: &Value, key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let mut all: Vec<ForgeEntry> = BUILT_IN_FORGES
            .iter()
            .filter(|label| !configured.iter().any(|(key, _)| key == *label))
            .map(|label| ForgeEntry {
                label: label.to_string(),
                kind: label.to_string(),
                base_url: String::new(),
                api_url: String::new(),
                token_env: String::new(),
            })
            .collect();
        for (label, entry) in configured {
            let kind = text(entry, "type");
            all.push(ForgeEntry {
                label: label.clone(),
                kind: if kind.is_empty() {
                    "git".to_string()
                } else {
                    kind
                },
                base_url: text(entry, "base_url"),
                api_url: text(entry, "api_url"),
                token_env: text(entry, "token_env"),
            });
        }
        all.sort_by(|a, b| a.label.cmp(&b.label));
        all
    }

    /// Register `name` at `url` (as the caller resolved it), bound to
    /// `forge` (empty for none). A taken name, a repository another remote is
    /// registered for, an address naming no repository and an unknown forge
    /// are refused; nothing changes then.
    pub fn add(&mut self, name: &str, url: &str, forge: &str) -> Result<Remote, DepsError> {
        if self.get(name).is_some() {
            return Err(DepsError::RemoteExists {
                name: name.to_string(),
            });
        }
        let stored = stored_url(url)?;
        if let Some(existing) = self.lookup_url(&stored) {
            return Err(DepsError::SameRepository {
                existing: existing.name,
            });
        }
        self.check_forge(forge)?;
        let remote = Remote {
            name: name.to_string(),
            url: stored,
            forge: forge.to_string(),
        };
        self.remotes_mut()
            .push((name.to_string(), entry_of(&remote)));
        Ok(remote)
    }

    /// Change a remote in place. Everything is checked before anything
    /// changes; a rename carries the default.
    pub fn edit(&mut self, name: &str, edit: &Edit) -> Result<Edited, DepsError> {
        if edit.name.is_none() && edit.url.is_none() && edit.forge.is_none() {
            return Err(DepsError::NothingToEdit);
        }
        let before = self.get(name).ok_or_else(|| DepsError::RemoteNotFound {
            name: name.to_string(),
        })?;
        let mut after = before.clone();
        if let Some(new_name) = edit.name.as_deref().filter(|n| *n != name) {
            if new_name.is_empty() {
                return Err(DepsError::EmptyName);
            }
            if self.get(new_name).is_some() {
                return Err(DepsError::RemoteExists {
                    name: new_name.to_string(),
                });
            }
            after.name = new_name.to_string();
        }
        if let Some(url) = &edit.url {
            let stored = stored_url(url)?;
            if let Some(other) = self.lookup_url(&stored).filter(|r| r.name != name) {
                return Err(DepsError::SameRepository {
                    existing: other.name,
                });
            }
            after.url = stored;
        }
        if let Some(forge) = &edit.forge {
            self.check_forge(forge)?;
            after.forge.clone_from(forge);
        }
        let default_followed = after.name != before.name && self.default_name() == name;
        let remotes = self.remotes_mut();
        if let Some(slot) = remotes.iter_mut().find(|(key, _)| key == name) {
            *slot = (after.name.clone(), merged(&slot.1, &after));
        }
        if default_followed {
            *self.doc.entry("default") = Value::scalar(after.name.clone());
        }
        let changed = [
            ("name", before.name != after.name),
            ("url", before.url != after.url),
            ("forge", before.forge != after.forge),
        ]
        .into_iter()
        .filter_map(|(field, moved)| moved.then_some(field))
        .collect();
        Ok(Edited {
            before,
            after,
            changed,
            default_followed,
        })
    }

    /// Unregister `name`; the content already pulled from it is untouched.
    pub fn remove(&mut self, name: &str) -> Result<Remote, DepsError> {
        let removed = self.get(name).ok_or_else(|| DepsError::RemoteNotFound {
            name: name.to_string(),
        })?;
        self.remotes_mut().retain(|(key, _)| key != name);
        if self.default_name() == name {
            self.doc.remove("default");
        }
        Ok(removed)
    }

    /// Make `name` the default; none clears it.
    pub fn set_default(&mut self, name: Option<&str>) -> Result<(), DepsError> {
        match name {
            None => {
                self.doc.remove("default");
            }
            Some(name) => {
                if self.get(name).is_none() {
                    return Err(DepsError::RemoteNotFound {
                        name: name.to_string(),
                    });
                }
                *self.doc.entry("default") = Value::scalar(name);
            }
        }
        Ok(())
    }

    /// A forge label that is empty (resolve by host), built in or configured.
    fn check_forge(&self, label: &str) -> Result<(), DepsError> {
        if label.is_empty() || self.forge_labels().iter().any(|known| known == label) {
            return Ok(());
        }
        Err(DepsError::UnknownForge {
            label: label.to_string(),
        })
    }

    /// The `remotes` mapping's entries, made if need be.
    fn remotes_mut(&mut self) -> &mut Vec<(String, Value)> {
        let remotes = self.doc.entry("remotes");
        if !matches!(remotes, Value::Map(_)) {
            *remotes = Value::Map(Vec::new());
        }
        match remotes {
            Value::Map(entries) => entries,
            _ => unreachable!("made a mapping just above"),
        }
    }
}

/// What the registry's kind is called in an error.
const KIND: &str = "remotes";

fn remote_of(name: &str, entry: &Value) -> Remote {
    let text = |key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Remote {
        name: name.to_string(),
        url: text("url"),
        forge: text("forge"),
    }
}

/// A new remote's entry: its address, and its forge when it has one.
fn entry_of(remote: &Remote) -> Value {
    merged(&Value::Map(Vec::new()), remote)
}

/// `entry` with the remote's address and forge set, and every other key it
/// had (the author's) kept where it was.
fn merged(entry: &Value, remote: &Remote) -> Value {
    let mut entry = entry.clone();
    if !matches!(entry, Value::Map(_)) {
        entry = Value::Map(Vec::new());
    }
    *entry.entry("url") = Value::scalar(remote.url.clone());
    if remote.forge.is_empty() {
        entry.remove("forge");
    } else {
        *entry.entry("forge") = Value::scalar(remote.forge.clone());
    }
    entry
}

/// `value` with the entries of the maps that are ctxloom's sorted by key.
fn sorted(key: &str, value: &Value) -> Value {
    match (key, value) {
        ("remotes" | "forges", Value::Map(entries)) => {
            let mut entries = entries.clone();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Map(entries)
        }
        _ => value.clone(),
    }
}

/// The shape of a registry that was read: every part is what it should be.
fn check(doc: &Value, path: &str) -> Result<(), DepsError> {
    let bad = |key: &str, detail: &str| DepsError::BadShape {
        path: path.to_string(),
        key: key.to_string(),
        detail: detail.to_string(),
    };
    match doc.get("remotes") {
        None | Some(Value::Null) => {}
        Some(Value::Map(remotes)) => {
            for (name, entry) in remotes {
                let key = format!("remotes.{name}");
                if !matches!(entry, Value::Map(_)) {
                    return Err(bad(&key, "expected a mapping with a url"));
                }
                match entry.get("url") {
                    Some(Value::Scalar(url)) if !url.is_empty() => {}
                    _ => {
                        return Err(bad(
                            &format!("{key}.url"),
                            "expected the repository's address",
                        ))
                    }
                }
                if !matches!(
                    entry.get("forge"),
                    None | Some(Value::Null | Value::Scalar(_))
                ) {
                    return Err(bad(&format!("{key}.forge"), "expected a forge label"));
                }
            }
        }
        Some(_) => return Err(bad("remotes", "expected a mapping")),
    }
    match doc.get("forges") {
        None | Some(Value::Null) => {}
        Some(Value::Map(forges)) => {
            for (label, forge) in forges {
                if !matches!(forge, Value::Map(_)) {
                    return Err(bad(&format!("forges.{label}"), "expected a mapping"));
                }
                match forge.get("type") {
                    None | Some(Value::Null) => {}
                    Some(Value::Scalar(kind))
                        if kind.is_empty() || BUILT_IN_FORGES.contains(&kind.as_str()) => {}
                    Some(Value::Scalar(kind)) => {
                        return Err(DepsError::ForgeType {
                            forge: label.clone(),
                            found: kind.clone(),
                        })
                    }
                    Some(_) => return Err(bad(&format!("forges.{label}.type"), "expected a name")),
                }
            }
        }
        Some(_) => return Err(bad("forges", "expected a mapping")),
    }
    if !matches!(
        doc.get("default"),
        None | Some(Value::Null | Value::Scalar(_))
    ) {
        return Err(bad("default", "expected a remote's name"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "registry.test.rs"]
mod tests;
