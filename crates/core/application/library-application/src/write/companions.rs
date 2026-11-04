//! `companion add|remove` (Go's `operations.AddCompanion`, `RemoveCompanion`).
//! A companion is registered by NAME, in the user's home config, never by
//! path: only a registered name is ever run, resolved on the `PATH` each time.

use angzarr_yaml::Value;
use ctxloom_ports::{Scope, Unanswered};

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::write::views::{CompanionAdded, CompanionRemoved, Preview, Removal};
use crate::write::WriteError;
use crate::Flags;

/// The companions that are their own binaries.
const FIRST_PARTY: [&str; 3] = ["ltk", "taskloom", "reprise"];

impl Library {
    /// `companion add <name>`: the binary is found on the `PATH` and asked
    /// for its loadout before anything is recorded.
    pub fn companion_add(&self, flags: &Flags, name: &str) -> Outcome<CompanionAdded> {
        self.run(flags, |p| p.companion_add(name))
    }

    /// `companion remove <name> [--yes]`.
    pub fn companion_remove(
        &self,
        flags: &Flags,
        name: &str,
        yes: bool,
    ) -> Outcome<Removal<CompanionRemoved>> {
        self.run(flags, |p| p.companion_remove(name, yes))
    }
}

/// Whether `name` can be a companion's: it starts with a letter or digit and
/// goes on in letters, digits, dots, hyphens and underscores.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn names(current: Option<&Value>) -> Vec<String> {
    current
        .and_then(Value::as_seq)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

impl Project {
    fn companion_add(&self, name: &str) -> Result<CompanionAdded, AppError> {
        if !valid_name(name) {
            return Err(WriteError::CompanionName {
                name: name.to_string(),
            }
            .into());
        }
        let ports = self.write_ports()?;
        let bin = if FIRST_PARTY.contains(&name) {
            name.to_string()
        } else {
            format!("ctxloom-companion-{name}")
        };
        let Some(path) = self.binaries.find(&bin) else {
            return Err(WriteError::CompanionNotOnPath {
                bin,
                name: name.to_string(),
            }
            .into());
        };
        match ports.probe.answers(&path) {
            Ok(()) => {}
            Err(Unanswered::Failed(detail)) => {
                return Err(WriteError::NotACompanion { path, detail }.into())
            }
            Err(Unanswered::Silent) => return Err(WriteError::CompanionSilent { path }.into()),
        }
        let mut added = false;
        self.update_config(Scope::Home, "companions", |current| {
            let mut held = names(current.as_ref());
            if !held.iter().any(|n| n == name) {
                held.push(name.to_string());
                added = true;
            }
            Ok(Some(Value::Seq(
                held.into_iter().map(Value::Scalar).collect(),
            )))
        })?;
        Ok(CompanionAdded {
            name: name.to_string(),
            bin,
            path,
            added,
        })
    }

    fn companion_remove(
        &self,
        name: &str,
        yes: bool,
    ) -> Result<Removal<CompanionRemoved>, AppError> {
        let missing = || WriteError::CompanionNotRegistered {
            name: name.to_string(),
        };
        if !yes {
            if !self.config.companions().iter().any(|n| n == name) {
                return Err(missing().into());
            }
            return Ok(Removal::Preview(Preview::of(
                format!("companion \"{name}\""),
                Vec::new(),
                format!("ctxloom companion remove {name} --yes"),
            )));
        }
        self.update_config(Scope::Home, "companions", |current| {
            let mut held = names(current.as_ref());
            let Some(at) = held.iter().position(|n| n == name) else {
                return Err(missing().into());
            };
            held.remove(at);
            Ok((!held.is_empty())
                .then(|| Value::Seq(held.into_iter().map(Value::Scalar).collect())))
        })?;
        Ok(Removal::Done(CompanionRemoved {
            name: name.to_string(),
            removed: true,
        }))
    }
}

#[cfg(test)]
#[path = "companions.test.rs"]
mod tests;
