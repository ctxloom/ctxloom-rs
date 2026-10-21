//! Taskloom's configuration, as Go's `internal/taskloom/config` layers it,
//! and as far as the tag schema needs it. Each layer beats the one before:
//!
//! 1. `~/.taskloom/config.yaml`;
//! 2. `<root>/.taskloom/config.yaml`, the work root's;
//! 3. `TASKLOOM_CONFIG_<KEY>` environment variables (`TASKLOOM_CONFIG_TAG_SCHEMA`);
//! 4. `--config-set <path>=<value>` overrides, in order.
//!
//! `tag_schema` is a list, and a layer that sets it replaces the whole list
//! below it, as Go's does: lists are never merged. An environment variable
//! or override gives the list comma-separated; unlike Go, a comma inside a
//! double-quoted span does not split, so an enum declaration's members
//! (`tagma.enum:"k:x"="a,b"`) stay one declaration. A missing file is no
//! layer; a file that cannot be read, is not YAML, names a key taskloom does
//! not know or is a newer `schema_version` is refused, as is an environment
//! variable or override naming no key. `homing` (repo homing is dropped)
//! and `schema_version` are known and ignored. Nothing is written: Go's
//! `--write-upgrades` has no version to upgrade from here.

mod error;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use annals_diagnostic::Report;
use ports::Configuration;
use serde::Deserialize;

pub use error::{diagnostic_samples, ConfigError, EN_US};

/// The directory a config file is in, at the home and the work root.
pub const DIR: &str = ".taskloom";
/// The config file's name.
pub const FILE: &str = "config.yaml";
/// What an environment variable's name starts with to override a key.
pub const ENV_PREFIX: &str = "TASKLOOM_CONFIG_";
/// The newest `schema_version` this taskloom reads.
pub const SCHEMA_VERSION: u64 = 1;
/// The config keys taskloom knows.
pub const KEYS: [&str; 3] = ["homing", "schema_version", "tag_schema"];

/// One config file's keys.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct File {
    schema_version: Option<u64>,
    #[allow(dead_code)]
    homing: Option<String>,
    tag_schema: Option<Vec<String>>,
}

/// Taskloom's configuration over the filesystem and the environment.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TaskloomConfig {
    home: Option<PathBuf>,
    root: Option<PathBuf>,
    env: Vec<(String, String)>,
}

impl TaskloomConfig {
    /// Configuration read from `home`'s and `root`'s config files, when
    /// they are known, and from `env`, the process's environment.
    pub fn new(home: Option<PathBuf>, root: Option<PathBuf>, env: Vec<(String, String)>) -> Self {
        TaskloomConfig { home, root, env }
    }

    fn files(&self) -> impl Iterator<Item = PathBuf> + '_ {
        [&self.home, &self.root]
            .into_iter()
            .flatten()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| dir.join(DIR).join(FILE))
    }

    fn resolve(&self, overrides: &[String]) -> Result<Vec<String>, ConfigError> {
        let mut tag_schema = Vec::new();
        for path in self.files() {
            if let Some(list) = read(&path)?.tag_schema {
                tag_schema = list;
            }
        }
        for (name, value) in &self.env {
            let Some(key) = name.strip_prefix(ENV_PREFIX) else {
                continue;
            };
            if let Some(list) = set(name, &key.to_ascii_lowercase(), value)? {
                tag_schema = list;
            }
        }
        for entry in overrides {
            let (path, value) = entry
                .split_once('=')
                .filter(|(path, _)| !path.trim().is_empty())
                .ok_or_else(|| ConfigError::NotAnOverride {
                    entry: entry.clone(),
                })?;
            let key = path.trim().to_ascii_lowercase();
            if let Some(list) = set("--config-set", &key, value)? {
                tag_schema = list;
            }
        }
        Ok(tag_schema)
    }
}

impl Configuration for TaskloomConfig {
    fn tag_schema(&mut self, overrides: &[String]) -> Result<Vec<String>, Report> {
        self.resolve(overrides).map_err(|e| Report::of(&e))
    }
}

/// The config file at `path`; no keys when there is none.
fn read(path: &Path) -> Result<File, ConfigError> {
    let shown = || path.display().to_string();
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(File::default()),
        Err(e) => {
            return Err(ConfigError::Unreadable {
                path: shown(),
                detail: e.to_string(),
            })
        }
    };
    // An empty file, or one of comments, is a document of nothing.
    let file: Option<File> = serde_norway::from_str(&text).map_err(|e| ConfigError::Malformed {
        path: shown(),
        detail: e.to_string(),
    })?;
    let file = file.unwrap_or_default();
    match file.schema_version {
        Some(version) if version > SCHEMA_VERSION => Err(ConfigError::Newer {
            path: shown(),
            version,
            current: SCHEMA_VERSION,
        }),
        _ => Ok(file),
    }
}

/// What `source` setting `key` to `value` does to the tag schema: `Some`
/// list when it sets it, `None` for another known key.
fn set(source: &str, key: &str, value: &str) -> Result<Option<Vec<String>>, ConfigError> {
    match key {
        "tag_schema" => split(value)
            .map(Some)
            .ok_or_else(|| ConfigError::Unterminated {
                source: source.to_string(),
                value: value.to_string(),
            }),
        "homing" | "schema_version" => Ok(None),
        _ => Err(ConfigError::UnknownKey {
            source: source.to_string(),
            key: key.to_string(),
        }),
    }
}

/// `value`'s comma-separated items, trimmed, the empty ones dropped; a
/// comma inside a double-quoted span (where `\` escapes the next
/// character) does not split. `None` when a quote is never closed.
fn split(value: &str) -> Option<Vec<String>> {
    let mut items = Vec::new();
    let mut item = String::new();
    let (mut quoted, mut escaped) = (false, false);
    for c in value.chars() {
        if escaped {
            escaped = false;
        } else if quoted && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c == ',' && !quoted {
            items.push(std::mem::take(&mut item));
            continue;
        }
        item.push(c);
    }
    if quoted {
        return None;
    }
    items.push(item);
    Some(
        items
            .into_iter()
            .map(|i| i.trim().to_string())
            .filter(|i| !i.is_empty())
            .collect(),
    )
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
