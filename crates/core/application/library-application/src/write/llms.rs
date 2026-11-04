//! `llm create|edit|default|remove` (Go's `operations.SetLLM`,
//! `SetDefaultLLM`, `RemoveLLM`). A label is an entry under `llm.configs` of a
//! config file: the project's, or with `--home` the user's own.

use angzarr_yaml::Value;
use ctxloom_ports::Scope;
use library::config::schema;

use crate::agents::ENGINES;
use crate::error::{AppError, Notice};
use crate::project::{Library, Outcome, Project};
use crate::write::engines;
use crate::write::requests::LlmRequest;
use crate::write::views::{
    LlmDefault, LlmDefaultSet, LlmDefaultShown, LlmRemoved, LlmWritten, Preview, Removal,
};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `llm create <label> ...` (`must_exist` false) and `llm edit <label>
    /// ...` (true).
    pub fn llm_write(
        &self,
        flags: &Flags,
        label: &str,
        request: &LlmRequest,
        home: bool,
        must_exist: bool,
    ) -> Outcome<LlmWritten> {
        self.run(flags, |p| p.llm_write(label, request, home, must_exist))
    }

    /// `llm default [label]`: the default engine label, or make one the default.
    pub fn llm_default(&self, flags: &Flags, name: Option<&str>) -> Outcome<LlmDefault> {
        self.run(flags, |p| p.llm_default(name))
    }

    /// `llm remove <label> [--home] [--yes]`.
    pub fn llm_remove(
        &self,
        flags: &Flags,
        label: &str,
        home: bool,
        yes: bool,
    ) -> Outcome<Removal<LlmRemoved>> {
        self.run(flags, |p| p.llm_remove(label, home, yes))
    }
}

fn scope(home: bool) -> Scope {
    if home {
        Scope::Home
    } else {
        Scope::Project
    }
}

fn text(value: &str) -> Value {
    Value::Scalar(value.to_string())
}

fn as_map(value: Value) -> Value {
    match value {
        map @ Value::Map(_) => map,
        _ => Value::Map(Vec::new()),
    }
}

/// `key` of the mapping set, or taken out where `value` is none or an empty
/// mapping.
fn put(map: &mut Value, key: &str, value: Option<Value>) {
    match value {
        Some(Value::Map(m)) if m.is_empty() => {
            map.remove(key);
        }
        Some(value) => *map.entry(key) = value,
        None => {
            map.remove(key);
        }
    }
}

/// `value` set at the dotted `path` below `map`, the parents made; an empty
/// value takes the key out, and the parents it leaves empty.
fn set_path(map: &mut Value, path: &[&str], value: Option<Value>) {
    match path {
        [] => {}
        [last] => put(map, last, value),
        [first, rest @ ..] => {
            let mut child = as_map(map.get(first).cloned().unwrap_or(Value::Null));
            set_path(&mut child, rest, value);
            put(map, first, Some(child));
        }
    }
}

/// A block on one line: its scalars as `key=value`, sorted.
fn block_line(block: &Value) -> String {
    let Value::Map(entries) = block else {
        return String::new();
    };
    let mut parts: Vec<String> = entries
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|v| format!("{k}={v}")))
        .collect();
    parts.sort();
    parts.join(", ")
}

impl Project {
    fn llm_entry_view(&self, label: &str, entry: &Value, edited: bool) -> LlmWritten {
        let permissions = entry.get("permissions").cloned().unwrap_or(Value::Null);
        LlmWritten {
            label: label.to_string(),
            kind: entry
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            model: entry
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            permissions: crate::json::permissions(&permissions),
            permissions_line: block_line(&permissions),
            edited,
        }
    }

    /// The engine a label's entry takes its permission modes from.
    fn entry_engine(&self, label: &str, entry: &Value) -> String {
        entry
            .get("type")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(ToString::to_string)
            .or_else(|| ENGINES.contains(&label).then(|| label.to_string()))
            .unwrap_or_else(|| "claude-code".to_string())
    }

    /// The fields of `--set` laid on the entry; the whole entry then judged by
    /// the schema.
    fn apply_fields(
        &self,
        label: &str,
        entry: &mut Value,
        fields: &[String],
    ) -> Result<(), AppError> {
        let field = |detail: String| -> AppError {
            WriteError::LlmField {
                label: label.to_string(),
                detail,
            }
            .into()
        };
        for item in fields {
            let Some((key, raw)) = item.split_once('=') else {
                return Err(field(format!(
                    "--set {item}: expected <dotted.key>=<value>"
                )));
            };
            let key = key.trim();
            if key.is_empty() {
                return Err(field(format!("--set {item}: the key is empty")));
            }
            if key == "type" {
                return Err(field("type has its own flag: use --type".to_string()));
            }
            let value = if raw.trim().is_empty() {
                None
            } else {
                Some(
                    angzarr_yaml::parse(raw)
                        .map_err(|e| field(format!("--set {key}: the value is not YAML: {e}")))?,
                )
            };
            let path: Vec<&str> = key.split('.').collect();
            set_path(entry, &path, value);
        }
        // The schema judges an entry by its type: with none given it is the
        // one its label goes by, written out so the file reads by itself.
        if entry.get("type").is_none() {
            *entry.entry("type") = text(&self.entry_engine(label, entry));
        }
        let path = vec!["llm".to_string(), "configs".to_string(), label.to_string()];
        schema::validate_at(&path, entry).map_err(field)
    }

    fn llm_write(
        &self,
        label: &str,
        request: &LlmRequest,
        home: bool,
        must_exist: bool,
    ) -> Result<LlmWritten, AppError> {
        self.require_project()?;
        let exists = self.available_llms().iter().any(|l| l == label);
        if must_exist && !exists {
            return Err(WriteError::LlmMissing {
                label: label.to_string(),
            }
            .into());
        }
        if !must_exist && exists {
            return Err(WriteError::LlmExists {
                label: label.to_string(),
            }
            .into());
        }
        if let Some(kind) = request.kind.as_deref().filter(|k| !k.is_empty()) {
            if !ENGINES.contains(&kind) {
                return Err(WriteError::UnknownLlmType {
                    label: label.to_string(),
                    kind: kind.to_string(),
                    known: ENGINES.join(", "),
                }
                .into());
            }
        }
        let mut written = None;
        self.update_config(scope(home), "llm", |current| {
            let mut llm = as_map(current.unwrap_or(Value::Null));
            let mut configs = as_map(llm.get("configs").cloned().unwrap_or(Value::Null));
            let mut entry = as_map(configs.get(label).cloned().unwrap_or(Value::Null));
            if let Some(kind) = &request.kind {
                put(&mut entry, "type", (!kind.is_empty()).then(|| text(kind)));
            }
            if let Some(model) = &request.model {
                put(
                    &mut entry,
                    "model",
                    (!model.is_empty()).then(|| text(model)),
                );
            }
            if let Some(mode) = &request.permissions {
                let engine = self.entry_engine(label, &entry);
                let canonical = if mode.is_empty() {
                    String::new()
                } else {
                    engines::mode(&engine, mode).map_err(|detail| WriteError::Permissions {
                        subject: format!("llm \"{label}\": permissions"),
                        detail,
                    })?
                };
                let mut block = as_map(entry.get("permissions").cloned().unwrap_or(Value::Null));
                put(
                    &mut block,
                    "mode",
                    (!canonical.is_empty()).then(|| text(&canonical)),
                );
                put(&mut entry, "permissions", Some(block));
            }
            self.apply_fields(label, &mut entry, &request.set)?;
            written = Some(self.llm_entry_view(label, &entry, must_exist));
            put(&mut configs, label, Some(entry));
            put(&mut llm, "configs", Some(configs));
            Ok(Some(llm))
        })?;
        written.ok_or_else(|| WriteError::NotWired {}.into())
    }

    fn llm_default(&self, name: Option<&str>) -> Result<LlmDefault, AppError> {
        let Some(name) = name else {
            return Ok(LlmDefault::Shown(LlmDefaultShown {
                default: self.config.primary_label(),
            }));
        };
        self.require_project()?;
        let usable = self.available_llms();
        if !usable.iter().any(|l| l == name) {
            return Err(AppError::UnknownLlm {
                label: name.to_string(),
                usable: usable.join(", "),
            });
        }
        let mut unchanged = false;
        self.update_config(Scope::Project, "llm", |current| {
            let mut llm = as_map(current.unwrap_or(Value::Null));
            let mut defaults = as_map(llm.get("defaults").cloned().unwrap_or(Value::Null));
            unchanged = defaults.get("primary").and_then(Value::as_str) == Some(name);
            put(&mut defaults, "primary", Some(text(name)));
            put(&mut llm, "defaults", Some(defaults));
            Ok(Some(llm))
        })?;
        Ok(LlmDefault::Set(LlmDefaultSet {
            status: if unchanged { "unchanged" } else { "set" }.to_string(),
            name: name.to_string(),
        }))
    }

    fn llm_remove(
        &mut self,
        label: &str,
        home: bool,
        yes: bool,
    ) -> Result<Removal<LlmRemoved>, AppError> {
        self.require_project()?;
        if !yes {
            if !self.config.authored_labels.contains(label) {
                return Err(WriteError::LlmNotDefined {
                    label: label.to_string(),
                }
                .into());
            }
            let apply = if home {
                format!("ctxloom llm remove {label} --yes --home")
            } else {
                format!("ctxloom llm remove {label} --yes")
            };
            return Ok(Removal::Preview(Preview::of(
                format!("llm \"{label}\""),
                Vec::new(),
                apply,
            )));
        }
        let mut warnings = Vec::new();
        self.update_config(scope(home), "llm", |current| {
            let mut llm = as_map(current.unwrap_or(Value::Null));
            for (role, what) in [
                ("primary", "the configured default (llm.defaults.primary); set a new one with `ctxloom llm default <label>`"),
                ("fast", "the configured fast default (llm.defaults.fast)"),
            ] {
                let held = llm.get("defaults").and_then(|d| d.get(role)).and_then(Value::as_str);
                if held == Some(label) {
                    warnings.push(format!("llm \"{label}\" was {what}"));
                }
            }
            let mut configs = as_map(llm.get("configs").cloned().unwrap_or(Value::Null));
            if configs.get(label).is_none() {
                return Err(WriteError::LlmNotInConfig {
                    label: label.to_string(),
                }
                .into());
            }
            configs.remove(label);
            put(&mut llm, "configs", Some(configs));
            Ok(Some(llm).filter(|llm| !matches!(llm, Value::Map(m) if m.is_empty())))
        })?;
        self.notices.extend(warnings.into_iter().map(Notice::text));
        Ok(Removal::Done(LlmRemoved {
            status: "removed".to_string(),
            label: label.to_string(),
        }))
    }
}

#[cfg(test)]
#[path = "llms.test.rs"]
mod tests;
