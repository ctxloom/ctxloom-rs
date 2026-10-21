//! Why an agent's configuration could not be found, read or written; and
//! this crate's catalog.

use std::fmt;

use annals_diagnostic::{Arg, Diagnostic};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConfigError {
    /// HOME is not set: there is no user configuration to name.
    NoHome,
    /// An agent this adapter does not know.
    UnknownEngine { engine: String },
    /// The configuration is there and could not be read.
    Read { path: String, detail: String },
    /// The configuration is not the JSON a server can be written into; it
    /// was copied to `backup` and left as it was.
    Corrupt {
        file: String,
        detail: String,
        backup: String,
    },
    /// As [`ConfigError::Corrupt`], and the copy could not be made either.
    CorruptUnsaved {
        file: String,
        detail: String,
        reason: String,
    },
    /// The servers' container (`pointer`) is there and is not an object.
    NotAnObject { path: String, pointer: String },
    /// An entry under the server's name runs another program: someone's
    /// own, and not to be overwritten.
    Foreign { path: String, name: String },
    /// The configuration could not be written.
    Write { path: String, detail: String },
    /// A check could not read the configuration as JSON.
    Parse { detail: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NoHome => f.write_str(
                "the home directory is not set (HOME), so the user's configuration cannot be found",
            ),
            ConfigError::UnknownEngine { engine } => write!(f, "no agent is named {engine}"),
            ConfigError::Read { path, detail } => write!(f, "cannot read {path}: {detail}"),
            ConfigError::Corrupt {
                file,
                detail,
                backup,
            } => write!(
                f,
                "failed to parse {file}: {detail} - original backed up to {backup}; fix the JSON \
                 and re-run (refusing to write {file} to avoid deleting the MCP servers already in it)"
            ),
            ConfigError::CorruptUnsaved {
                file,
                detail,
                reason,
            } => write!(
                f,
                "failed to parse {file}: {detail}; additionally failed to back up the corrupt \
                 file: {reason} - refusing to write {file} to avoid deleting the MCP servers \
                 already in it"
            ),
            ConfigError::NotAnObject { path, pointer } => write!(
                f,
                "failed to write {path}: {pointer} is not an object, so no server can be added to it"
            ),
            ConfigError::Foreign { path, name } => write!(
                f,
                "{path} already has an MCP server named {name} that runs something else; it is \
                 left as it is (remove or rename it to register this one)"
            ),
            ConfigError::Write { path, detail } => write!(f, "failed to write {path}: {detail}"),
            ConfigError::Parse { detail } => write!(f, "parse config: {detail}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Diagnostic for ConfigError {
    fn code(&self) -> &'static str {
        match self {
            ConfigError::NoHome => "agent-configs-no-home",
            ConfigError::UnknownEngine { .. } => "agent-configs-unknown-engine",
            ConfigError::Read { .. } => "agent-configs-read",
            ConfigError::Corrupt { .. } => "agent-configs-corrupt",
            ConfigError::CorruptUnsaved { .. } => "agent-configs-corrupt-unsaved",
            ConfigError::NotAnObject { .. } => "agent-configs-not-an-object",
            ConfigError::Foreign { .. } => "agent-configs-foreign",
            ConfigError::Write { .. } => "agent-configs-write",
            ConfigError::Parse { .. } => "agent-configs-parse",
        }
    }

    fn args(&self) -> Vec<(&'static str, Arg)> {
        let s = |v: &String| Arg::from(v.as_str());
        match self {
            ConfigError::NoHome => Vec::new(),
            ConfigError::UnknownEngine { engine } => vec![("engine", s(engine))],
            ConfigError::Read { path, detail } | ConfigError::Write { path, detail } => {
                vec![("path", s(path)), ("detail", s(detail))]
            }
            ConfigError::Corrupt {
                file,
                detail,
                backup,
            } => vec![
                ("file", s(file)),
                ("detail", s(detail)),
                ("backup", s(backup)),
            ],
            ConfigError::CorruptUnsaved {
                file,
                detail,
                reason,
            } => vec![
                ("file", s(file)),
                ("detail", s(detail)),
                ("reason", s(reason)),
            ],
            ConfigError::NotAnObject { path, pointer } => {
                vec![("path", s(path)), ("pointer", s(pointer))]
            }
            ConfigError::Foreign { path, name } => vec![("path", s(path)), ("name", s(name))],
            ConfigError::Parse { detail } => vec![("detail", s(detail))],
        }
    }
}

/// This crate's en-US messages, its name first: what an edge's catalog
/// embeds to render its diagnostics.
pub const EN_US: (&str, &str) = (
    "agent-configs",
    include_str!("../locales/en-US/agent-configs.ftl"),
);

/// One value of every error this crate defines, for the catalog test.
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let path = || "/home/me/.claude.json".to_string();
    let detail = || "expected a string key at line 3, column 1".to_string();
    vec![
        Box::new(ConfigError::NoHome),
        Box::new(ConfigError::UnknownEngine {
            engine: "codex".to_string(),
        }),
        Box::new(ConfigError::Read {
            path: path(),
            detail: "Permission denied (os error 13)".to_string(),
        }),
        Box::new(ConfigError::Corrupt {
            file: ".claude.json".to_string(),
            detail: detail(),
            backup: "/home/me/.claude.json.corrupt-1792554586".to_string(),
        }),
        Box::new(ConfigError::CorruptUnsaved {
            file: ".claude.json".to_string(),
            detail: detail(),
            reason: "Read-only file system (os error 30)".to_string(),
        }),
        Box::new(ConfigError::NotAnObject {
            path: path(),
            pointer: "/mcpServers".to_string(),
        }),
        Box::new(ConfigError::Foreign {
            path: path(),
            name: "taskloom".to_string(),
        }),
        Box::new(ConfigError::Write {
            path: path(),
            detail: "No space left on device (os error 28)".to_string(),
        }),
        Box::new(ConfigError::Parse { detail: detail() }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
