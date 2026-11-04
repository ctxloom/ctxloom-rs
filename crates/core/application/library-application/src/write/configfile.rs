//! The write side's way into a config file: one section replaced, the rest of
//! the file left as it is, and nothing written that would not read or that
//! replaces a file that does not parse.

use angzarr_yaml::Value;
use ctxloom_ports::Scope;
use library::config::layer;
use library::config::schema;
use library::write::section;

use super::engines;
use super::plan::Plan;
use super::{WriteError, WritePorts};
use crate::error::AppError;
use crate::project::Project;

impl Project {
    /// The write side's ports, or the refusal that this build has none.
    pub(crate) fn write_ports(&self) -> Result<&WritePorts, AppError> {
        self.ports
            .as_ref()
            .ok_or_else(|| WriteError::NotWired {}.into())
    }

    /// A content write with no project to write into is refused.
    pub(crate) fn require_project(&self) -> Result<(), AppError> {
        if self.no_project() {
            return Err(WriteError::NoProject {}.into());
        }
        Ok(())
    }

    /// The top-level `key` of `scope`'s config file, changed by `edit`: given
    /// what the file holds there (none where it holds nothing), it answers the
    /// section to leave, or none to take the key out. The file is rewritten
    /// only where that changes it; the section is written with its keys sorted
    /// and the rest of the file as it was. A file that does not parse, or a
    /// result that would break the schema, is refused and the file is as it
    /// was. The path of the file written, as shown.
    pub(crate) fn update_config(
        &self,
        scope: Scope,
        key: &str,
        edit: impl FnOnce(Option<Value>) -> Result<Option<Value>, AppError>,
    ) -> Result<String, AppError> {
        let path = self.files.layer_path(scope);
        let held = self.files.layer(scope).map_err(AppError::Cause)?;
        let text = held.map(|layer| layer.text).unwrap_or_default();
        let corrupt = |cause: &dyn std::fmt::Display| -> AppError {
            WriteError::ConfigCorrupt {
                path: path.clone(),
                cause: cause.to_string(),
            }
            .into()
        };
        let current = section::section(&text, key).map_err(|e| corrupt(&e))?;
        let base = section::stamped(&text).map_err(|e| corrupt(&e))?;
        let next = match edit(current)? {
            Some(value) => section::set_section(&base, key, &value),
            None => section::remove_section(&base, key),
        }
        .map_err(|e| corrupt(&e))?;
        if next == text {
            return Ok(path);
        }
        let read = layer::read(&next).map_err(|e| corrupt(&e))?;
        let violations: Vec<String> = schema::validate(&read.value)
            .iter()
            .map(|v| format!("\n  - [config] {}", v.text(&path)))
            .collect();
        if !violations.is_empty() {
            return Err(WriteError::ConfigBreaks {
                violations: violations.concat(),
            }
            .into());
        }
        self.files
            .write_layer(scope, &next)
            .map_err(AppError::Cause)?;
        Ok(path)
    }
}

impl Project {
    /// The plan over the tree of the bundle `name` as it is now.
    pub(crate) fn plan_of(&self, name: &str) -> Result<Plan, AppError> {
        let files = self.content.read_tree(name).map_err(AppError::Cause)?;
        let exec = self
            .content
            .executable_files(name)
            .map_err(AppError::Cause)?;
        Ok(Plan::of(files, exec))
    }

    /// The plan's changes handed to the port, once the tree they leave is
    /// seen to read: a write never leaves a bundle that does not load. A plan
    /// that changed nothing hands over nothing.
    pub(crate) fn commit(&self, name: &str, plan: Plan) -> Result<(), AppError> {
        if plan.is_unchanged() {
            return Ok(());
        }
        library::tree::read_tree(name, plan.files()).map_err(|cause| {
            AppError::BundleUnreadable {
                ask: name.to_string(),
                cause: Box::new(cause),
            }
        })?;
        self.content
            .apply(name, &plan.into_changes())
            .map_err(AppError::Cause)
    }

    /// The names an engine or a label can go by: the engines of this build
    /// and the labels of the configuration, sorted.
    pub(crate) fn available_llms(&self) -> Vec<String> {
        let mut names: Vec<String> = crate::agents::ENGINES
            .iter()
            .map(ToString::to_string)
            .collect();
        names.extend(self.config.label_names());
        names.sort();
        names.dedup();
        names
    }

    /// The backend a label goes by: itself where it is an engine, else the
    /// `type` of its entry.
    pub(crate) fn engine_of(&self, label: &str) -> Option<String> {
        if engines::known(label) {
            return Some(label.to_string());
        }
        self.config
            .llm_entry(label)
            .and_then(|entry| entry.get("type"))
            .and_then(Value::as_str)
            .filter(|kind| engines::known(kind))
            .map(ToString::to_string)
    }
}

#[cfg(test)]
#[path = "configfile.test.rs"]
mod tests;
