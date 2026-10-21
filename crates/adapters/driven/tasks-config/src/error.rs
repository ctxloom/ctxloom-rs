//! Why taskloom's configuration could not be read.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

use crate::KEYS;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigError {
    /// A config file that exists could not be read.
    Unreadable { path: String, detail: String },
    /// A config file that is not taskloom's: not YAML, a key it does not
    /// know, or a value of the wrong shape.
    Malformed { path: String, detail: String },
    /// A config file written for a newer taskloom.
    Newer {
        path: String,
        version: u64,
        current: u64,
    },
    /// An environment variable or `--config-set` naming no config key.
    UnknownKey { source: String, key: String },
    /// A `--config-set` entry that is not `<path>=<value>`.
    NotAnOverride { entry: String },
    /// A list value whose quote is never closed.
    Unterminated { source: String, value: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Unreadable { path, detail } => write!(f, "cannot read {path}: {detail}"),
            ConfigError::Malformed { path, detail } => {
                write!(f, "{path} is not taskloom's config: {detail}")
            }
            ConfigError::Newer {
                path,
                version,
                current,
            } => write!(
                f,
                "{path} is schema_version {version}, newer than the {current} this taskloom reads"
            ),
            ConfigError::UnknownKey { source, key } => write!(
                f,
                "{source} sets {key}, which is no config key taskloom knows (known: {})",
                KEYS.join(", ")
            ),
            ConfigError::NotAnOverride { entry } => {
                write!(f, "--config-set {entry}: expected <path>=<value>")
            }
            ConfigError::Unterminated { source, value } => {
                write!(f, "{source}: {value} has an unterminated quote")
            }
        }
    }
}

impl Diagnostic for ConfigError {
    fn code(&self) -> &'static str {
        match self {
            ConfigError::Unreadable { .. } => "tasks-config-unreadable",
            ConfigError::Malformed { .. } => "tasks-config-malformed",
            ConfigError::Newer { .. } => "tasks-config-newer",
            ConfigError::UnknownKey { .. } => "tasks-config-unknown-key",
            ConfigError::NotAnOverride { .. } => "tasks-config-not-an-override",
            ConfigError::Unterminated { .. } => "tasks-config-unterminated",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let s = |v: &String| Arg::from(v.as_str());
        match self {
            ConfigError::Unreadable { path, detail } | ConfigError::Malformed { path, detail } => {
                vec![("path", s(path)), ("detail", s(detail))]
            }
            ConfigError::Newer {
                path,
                version,
                current,
            } => vec![
                ("path", s(path)),
                ("version", (*version).into()),
                ("current", (*current).into()),
            ],
            ConfigError::UnknownKey { source, key } => vec![
                ("source", s(source)),
                ("key", s(key)),
                ("known", KEYS.join(", ").into()),
            ],
            ConfigError::NotAnOverride { entry } => vec![("entry", s(entry))],
            ConfigError::Unterminated { source, value } => {
                vec![("source", s(source)), ("value", s(value))]
            }
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "tasks-config",
    include_str!("../locales/en-US/tasks-config.ftl"),
);

/// One value of every error, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/u/.taskloom/config.yaml".to_string();
    vec![
        Box::new(ConfigError::Unreadable {
            path: path(),
            detail: "permission denied".to_string(),
        }),
        Box::new(ConfigError::Malformed {
            path: path(),
            detail: "unknown field `homeing`".to_string(),
        }),
        Box::new(ConfigError::Newer {
            path: path(),
            version: 2,
            current: 1,
        }),
        Box::new(ConfigError::UnknownKey {
            source: "TASKLOOM_CONFIG_TAGS".to_string(),
            key: "tags".to_string(),
        }),
        Box::new(ConfigError::NotAnOverride {
            entry: "tag_schema".to_string(),
        }),
        Box::new(ConfigError::Unterminated {
            source: "--config-set".to_string(),
            value: "\"a,b".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
