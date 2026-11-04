//! `agent create|edit|default|remove` (Go's `operations.SetAgent`,
//! `RemoveAgent` and `cli/agent.go`). An agent is a LOCAL binding under the
//! `agents:` key of the project's `config.yaml`; create and edit are not an
//! upsert (each refuses the case the other owns), and every axis is checked
//! before the file is touched.

use angzarr_yaml::Value;
use ctxloom_ports::Scope;

use crate::agents::RUNTIMES;
use crate::config::Agent;
use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::views::AgentRow;
use crate::write::engines;
use crate::write::requests::AgentRequest;
use crate::write::views::{AgentRemoved, DefaultAgent, Preview, Removal};
use crate::write::WriteError;
use crate::Flags;

impl Library {
    /// `agent create <name> ...` (`must_exist` false) and `agent edit <name>
    /// ...` (true): only the flags named change; one named empty clears.
    pub fn agent_write(
        &self,
        flags: &Flags,
        name: &str,
        request: &AgentRequest,
        must_exist: bool,
    ) -> Outcome<AgentRow> {
        self.run(flags, |p| p.agent_write(name, request, must_exist))
    }

    /// `agent default [name]`: the agent a bare run binds, or bind one (one
    /// not defined yet is bound, and said).
    pub fn agent_default(&self, flags: &Flags, name: Option<&str>) -> Outcome<DefaultAgent> {
        self.run(flags, |p| p.agent_default(name))
    }

    /// `agent remove <name> [--yes]`.
    pub fn agent_remove(
        &self,
        flags: &Flags,
        name: &str,
        yes: bool,
    ) -> Outcome<Removal<AgentRemoved>> {
        self.run(flags, |p| p.agent_remove(name, yes))
    }
}

fn text(value: &str) -> Value {
    Value::Scalar(value.to_string())
}

/// `key` of the mapping set to `value`, or taken out where `value` is none.
fn put(map: &mut Value, key: &str, value: Option<Value>) {
    match value {
        Some(value) => *map.entry(key) = value,
        None => {
            map.remove(key);
        }
    }
}

fn some_text(value: &str) -> Option<Value> {
    (!value.is_empty()).then(|| text(value))
}

fn pairs_map(pairs: &[(String, String)]) -> Option<Value> {
    (!pairs.is_empty()).then(|| {
        Value::Map(
            pairs
                .iter()
                .map(|(k, v)| (k.trim().to_string(), text(v.trim())))
                .collect(),
        )
    })
}

impl Project {
    /// The backend of the engine label a write results in: the request's, else
    /// the one the layer's entry holds, else the one the merged configuration
    /// holds.
    fn resulting_backend(
        &self,
        name: &str,
        request: &AgentRequest,
        held: &Value,
    ) -> Option<String> {
        let label = match &request.llm {
            Some(label) => label.clone(),
            None => {
                let own = held.get("llm").and_then(Value::as_str).unwrap_or_default();
                if own.is_empty() {
                    self.config.agent(name).map(|a| a.llm).unwrap_or_default()
                } else {
                    own.to_string()
                }
            }
        };
        if label.is_empty() {
            return None;
        }
        self.engine_of(&label)
    }

    /// The axes of a request, judged before anything is written; the mode in
    /// the spelling its engine writes it.
    fn check_agent(
        &self,
        name: &str,
        request: &AgentRequest,
        held: &Value,
    ) -> Result<Option<String>, AppError> {
        if let Some(llm) = request.llm.as_deref().filter(|l| !l.is_empty()) {
            let valid = self.available_llms();
            if !valid.iter().any(|v| v == llm) {
                return Err(WriteError::UnknownEngine {
                    name: name.to_string(),
                    engine: llm.to_string(),
                    valid: valid.join(", "),
                }
                .into());
            }
        }
        if let Some(runtime) = request.runtime.as_deref().filter(|r| !r.is_empty()) {
            if !RUNTIMES.contains(&runtime) {
                return Err(WriteError::UnknownRuntime {
                    name: name.to_string(),
                    value: runtime.to_string(),
                }
                .into());
            }
        }
        if let Some(home) = request.engine_home.as_deref().filter(|h| !h.is_empty()) {
            if !["host", "session"].contains(&home) {
                return Err(WriteError::UnknownEngineHome {
                    name: name.to_string(),
                    value: home.to_string(),
                }
                .into());
            }
        }
        let backend = self.resulting_backend(name, request, held);
        self.check_surfaces(name, request, backend.as_deref())?;
        self.check_roots(name, request, backend.as_deref())?;
        let Some(mode) = request.permissions.as_deref() else {
            return Ok(None);
        };
        let Some(backend) = backend else {
            return Err(WriteError::PermissionsNeedEngine {
                name: name.to_string(),
            }
            .into());
        };
        if mode.is_empty() {
            return Ok(Some(String::new()));
        }
        engines::mode(&backend, mode).map(Some).map_err(|detail| {
            WriteError::Permissions {
                subject: format!("agent \"{name}\""),
                detail,
            }
            .into()
        })
    }

    fn check_surfaces(
        &self,
        name: &str,
        request: &AgentRequest,
        backend: Option<&str>,
    ) -> Result<(), AppError> {
        let Some(pairs) = request.surfaces.as_ref().filter(|p| !p.is_empty()) else {
            return Ok(());
        };
        let Some(backend) = backend else {
            return Err(WriteError::SurfaceNeedsEngine {
                name: name.to_string(),
            }
            .into());
        };
        for (kind, approach) in pairs {
            let (kind, approach) = (kind.trim(), approach.trim());
            let refuse = |detail: String| -> AppError {
                WriteError::SurfaceRefused {
                    name: name.to_string(),
                    detail,
                }
                .into()
            };
            if !engines::SURFACE_KINDS.contains(&kind) {
                return Err(refuse(format!(
                    "surfaces {kind}={approach}: \"{kind}\" is not a surface kind"
                )));
            }
            if let Some(offered) = engines::approaches(backend, kind) {
                if !offered.contains(&approach) {
                    return Err(refuse(format!(
                        "surfaces {kind}={approach}: {backend} does not support it (supports: {})",
                        offered.join(", ")
                    )));
                }
            }
        }
        Ok(())
    }

    fn check_roots(
        &self,
        name: &str,
        request: &AgentRequest,
        backend: Option<&str>,
    ) -> Result<(), AppError> {
        let Some(pairs) = request.roots.as_ref().filter(|p| !p.is_empty()) else {
            return Ok(());
        };
        if backend.is_none() {
            return Err(WriteError::RootNeedsEngine {
                name: name.to_string(),
            }
            .into());
        }
        for (kind, root) in pairs {
            let (kind, root) = (kind.trim(), root.trim());
            let refuse = |detail: String| -> AppError {
                WriteError::RootRefused {
                    name: name.to_string(),
                    detail,
                }
                .into()
            };
            if !engines::SURFACE_KINDS.contains(&kind) {
                return Err(refuse(format!(
                    "roots {kind}={root}: \"{kind}\" is not a surface kind"
                )));
            }
            if !engines::ROOTS.contains(&root) {
                return Err(refuse(format!(
                    "roots {kind}={root}: \"{root}\" is not a root (session-home, project-root, work-dir)"
                )));
            }
        }
        Ok(())
    }

    fn agent_write(
        &self,
        name: &str,
        request: &AgentRequest,
        must_exist: bool,
    ) -> Result<AgentRow, AppError> {
        self.require_project()?;
        let exists = self.config.agent(name).is_some();
        if must_exist && !exists {
            return Err(WriteError::AgentMissing {
                name: name.to_string(),
            }
            .into());
        }
        if !must_exist && exists {
            return Err(WriteError::AgentExists {
                name: name.to_string(),
            }
            .into());
        }
        let mut written = None;
        let layer_held = |agents: &Option<Value>| {
            agents
                .as_ref()
                .and_then(|a| a.get(name))
                .cloned()
                .unwrap_or_else(|| Value::Map(Vec::new()))
        };
        // The checks need the entry the layer holds; the layer is read inside
        // the update, where it is held, so they are made there.
        self.update_config(Scope::Project, "agents", |current| {
            let held = layer_held(&current);
            let mode = self.check_agent(name, request, &held)?;
            let entry = self.agent_entry(name, request, held, mode)?;
            written = Some(Agent::of(name, &entry).row());
            let mut agents = match current {
                Some(map @ Value::Map(_)) => map,
                _ => Value::Map(Vec::new()),
            };
            *agents.entry(name) = entry;
            Ok(Some(agents))
        })?;
        written.ok_or_else(|| WriteError::NotWired {}.into())
    }

    /// The entry a request leaves: the flags named change their keys.
    fn agent_entry(
        &self,
        name: &str,
        request: &AgentRequest,
        mut entry: Value,
        mode: Option<String>,
    ) -> Result<Value, AppError> {
        if !matches!(entry, Value::Map(_)) {
            entry = Value::Map(Vec::new());
        }
        if let Some(llm) = &request.llm {
            put(&mut entry, "llm", some_text(llm));
        }
        if let Some(profiles) = &request.profiles {
            let list = Value::Seq(
                profiles
                    .iter()
                    .filter(|p| !p.is_empty())
                    .map(|p| text(p))
                    .collect(),
            );
            put(
                &mut entry,
                "profiles",
                (!matches!(&list, Value::Seq(l) if l.is_empty())).then_some(list),
            );
        }
        if let Some(runtime) = &request.runtime {
            put(&mut entry, "runtime", some_text(runtime));
        }
        if let Some(surfaces) = &request.surfaces {
            put(&mut entry, "surfaces", pairs_map(surfaces));
        }
        if let Some(roots) = &request.roots {
            put(&mut entry, "roots", pairs_map(roots));
        }
        if let Some(home) = &request.engine_home {
            put(&mut entry, "engine_home", some_text(home));
        }
        if let Some(on) = request.env_host {
            put(
                &mut entry,
                "env_host",
                Some(text(if on { "true" } else { "false" })),
            );
        }
        if let Some(env) = &request.env {
            let names: Vec<Value> = env
                .iter()
                .filter(|n| !n.is_empty())
                .map(|n| text(n))
                .collect();
            let list = Value::Seq(names);
            put(
                &mut entry,
                "env",
                (!matches!(&list, Value::Seq(l) if l.is_empty())).then_some(list),
            );
        }
        if let Some(mode) = mode {
            self.set_mode(name, request, &mut entry, &mode);
        }
        self.check_written(name, &entry)?;
        Ok(entry)
    }

    /// The mode written into (or taken out of) the block of the engine the
    /// agent binds.
    fn set_mode(&self, name: &str, request: &AgentRequest, entry: &mut Value, mode: &str) {
        let label = request
            .llm
            .clone()
            .or_else(|| {
                entry
                    .get("llm")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .or_else(|| self.config.agent(name).map(|a| a.llm))
            .unwrap_or_default();
        let Some(backend) = self.engine_of(&label) else {
            return;
        };
        let mut permissions = entry
            .get("permissions")
            .cloned()
            .unwrap_or(Value::Map(Vec::new()));
        if !matches!(permissions, Value::Map(_)) {
            permissions = Value::Map(Vec::new());
        }
        let mut block = permissions
            .get(&backend)
            .cloned()
            .unwrap_or(Value::Map(Vec::new()));
        if !matches!(block, Value::Map(_)) {
            block = Value::Map(Vec::new());
        }
        put(&mut block, "mode", some_text(mode));
        let empty = matches!(&block, Value::Map(m) if m.is_empty());
        put(&mut permissions, &backend, (!empty).then_some(block));
        let none_left = matches!(&permissions, Value::Map(m) if m.is_empty());
        put(entry, "permissions", (!none_left).then_some(permissions));
    }

    /// A record that cannot launch as written is refused.
    fn check_written(&self, name: &str, entry: &Value) -> Result<(), AppError> {
        let agent = Agent::of(name, entry);
        if agent.llm.is_empty() && agent.profiles.is_empty() {
            return Err(WriteError::AgentWithoutEngine {
                name: name.to_string(),
            }
            .into());
        }
        let bad = |detail: String| -> AppError {
            WriteError::BadEnv {
                name: name.to_string(),
                detail,
            }
            .into()
        };
        if let Some(entry) = agent.env.iter().find(|e| e.contains('=')) {
            return Err(bad(format!(
                "env entry \"{entry}\" must be a bare variable name, not NAME=value"
            )));
        }
        if !agent.env.is_empty() && agent.env_host != Some(false) {
            return Err(bad(
                "env lists names to pass through, but env_host is not false, so the host environment is passed whole; set --env-host=false to curate it".to_string(),
            ));
        }
        Ok(())
    }

    fn agent_default(&self, name: Option<&str>) -> Result<DefaultAgent, AppError> {
        let Some(name) = name else {
            return Ok(DefaultAgent::Current(self.config.default_agent()));
        };
        self.require_project()?;
        let defined = self.config.agent(name).is_some();
        self.update_config(Scope::Project, "default_agent", |_| Ok(Some(text(name))))?;
        Ok(DefaultAgent::Set {
            name: name.to_string(),
            defined,
        })
    }

    fn agent_remove(&self, name: &str, yes: bool) -> Result<Removal<AgentRemoved>, AppError> {
        self.require_project()?;
        if !yes {
            if self.config.agent(name).is_none() {
                return Err(WriteError::AgentNotDefined {
                    name: name.to_string(),
                }
                .into());
            }
            return Ok(Removal::Preview(Preview::of(
                format!("agent \"{name}\""),
                Vec::new(),
                format!("ctxloom agent remove {name} --yes"),
            )));
        }
        self.update_config(Scope::Project, "agents", |current| {
            let Some(mut agents) = current.filter(|a| a.get(name).is_some()) else {
                return Err(WriteError::AgentNotInConfig {
                    name: name.to_string(),
                }
                .into());
            };
            agents.remove(name);
            let empty = matches!(&agents, Value::Map(m) if m.is_empty());
            Ok((!empty).then_some(agents))
        })?;
        Ok(Removal::Done(AgentRemoved {
            status: "removed".to_string(),
            name: name.to_string(),
        }))
    }
}

#[cfg(test)]
#[path = "agents.test.rs"]
mod tests;
