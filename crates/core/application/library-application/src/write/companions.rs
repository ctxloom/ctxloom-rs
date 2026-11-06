//! `companion add|remove` (Go's `operations.AddCompanion`, `RemoveCompanion`).
//! A companion is registered in the user's home config as a name and the
//! command that runs it; only a registered command is ever run, and nothing
//! is searched for. Adding one asks it for its loadout once, as Go's add does,
//! before anything is recorded.

use angzarr_yaml::Value;
use ctxloom_ports::{Scope, Unanswered};

use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::write::views::{CompanionAdded, CompanionRemoved, Preview, Removal};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `companion add <name> --command <program> [args...]`: the command is
    /// asked for its loadout before anything is recorded.
    pub fn companion_add(
        &self,
        flags: &Flags,
        name: &str,
        command: &[String],
    ) -> Outcome<CompanionAdded> {
        self.run(flags, |p| p.companion_add(name, command))
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
pub(crate) fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A registration as the config holds it.
fn registration(command: &[String]) -> Value {
    Value::Map(vec![(
        "command".to_string(),
        Value::Seq(command.iter().cloned().map(Value::Scalar).collect()),
    )])
}

/// The companions a `companions` value registers, as `(name, registration)`.
fn registered(current: Option<&Value>) -> Vec<(String, Value)> {
    current
        .and_then(Value::as_map)
        .map(<[_]>::to_vec)
        .unwrap_or_default()
}

impl Project {
    fn companion_add(&self, name: &str, command: &[String]) -> Result<CompanionAdded, AppError> {
        if !valid_name(name) {
            return Err(WriteError::CompanionName {
                name: name.to_string(),
            }
            .into());
        }
        if command.first().is_none_or(|program| program.is_empty()) {
            return Err(WriteError::CompanionNoCommand {
                name: name.to_string(),
            }
            .into());
        }
        let ports = self.write_ports()?;
        let shown = command.join(" ");
        match ports.probe.answers(command) {
            Ok(()) => {}
            Err(Unanswered::Missing(_)) => {
                return Err(WriteError::CompanionProgramMissing {
                    program: command[0].clone(),
                    name: name.to_string(),
                }
                .into())
            }
            Err(Unanswered::Failed(detail) | Unanswered::Exited(detail)) => {
                return Err(WriteError::NotACompanion {
                    command: shown,
                    detail,
                }
                .into())
            }
            Err(Unanswered::Silent) => {
                return Err(WriteError::CompanionSilent { command: shown }.into())
            }
        }
        let mut added = false;
        self.update_config(Scope::Home, "companions", |current| {
            let mut held = registered(current.as_ref());
            let wanted = registration(command);
            match held.iter_mut().find(|(n, _)| n == name) {
                Some((_, existing)) if *existing == wanted => {}
                Some((_, existing)) => {
                    *existing = wanted;
                    added = true;
                }
                None => {
                    held.push((name.to_string(), wanted));
                    added = true;
                }
            }
            Ok(Some(Value::Map(held)))
        })?;
        Ok(CompanionAdded {
            name: name.to_string(),
            command: command.to_vec(),
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
            if !self.config.companions().iter().any(|c| c.name == name) {
                return Err(missing().into());
            }
            return Ok(Removal::Preview(Preview::of(
                format!("companion \"{name}\""),
                Vec::new(),
                format!("ctxloom companion remove {name} --yes"),
            )));
        }
        self.update_config(Scope::Home, "companions", |current| {
            let mut held = registered(current.as_ref());
            let Some(at) = held.iter().position(|(n, _)| n == name) else {
                return Err(missing().into());
            };
            held.remove(at);
            let any = !held.is_empty();
            Ok(any.then_some(Value::Map(held)))
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
