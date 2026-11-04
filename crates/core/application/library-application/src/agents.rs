//! `agent list|show`, `llm list`, `companion list` and `mcp server list|show`.

use std::collections::{BTreeMap, BTreeSet};

use angzarr_diagnostic::Diagnostic;
use angzarr_yaml::Value;
use library::model::McpServer;
use library::profile::ResolvedProfile;

use crate::config::{mcp_row, Agent};
use crate::error::AppError;
use crate::project::{Library, Outcome, Project};
use crate::views::{
    AgentRow, AgentShow, CompanionRow, LlmRow, McpList, McpListRow, McpRow, McpShow, Requirement,
    RequirementSource, ResolvedAgent,
};
use crate::Flags;

/// The engines this build knows by name.
pub const ENGINES: [&str; 6] = [
    "claude-code",
    "mock",
    "mock-launch",
    "mock-lossy",
    "mock-noskills",
    "opencode",
];

/// The built-in default engine.
const DEFAULT_ENGINE: &str = "claude-code";

/// The runtimes an engine can be given.
pub(crate) const RUNTIMES: [&str; 3] = ["host", "container-rootless", "container-rootful"];

/// Commands the container image stages itself (and so are not required of
/// the host).
const STAGED: [&str; 4] = ["ctxloom", "ltk", "taskloom", "reprise"];

impl Library {
    /// `agent list`.
    pub fn agent_list(&self, flags: &Flags) -> Outcome<Vec<AgentRow>> {
        self.run(flags, |p| {
            Ok(p.config.agents().iter().map(Agent::row).collect())
        })
    }

    /// `agent show <name>`; `say` words a diagnostic.
    pub fn agent_show(
        &self,
        flags: &Flags,
        name: &str,
        say: &dyn Fn(&dyn Diagnostic) -> String,
    ) -> Outcome<AgentShow> {
        self.run(flags, |p| p.agent_show(name, say))
    }

    /// `llm list`.
    pub fn llm_list(&self, flags: &Flags) -> Outcome<Vec<LlmRow>> {
        self.run(flags, |p| Ok(p.llm_list()))
    }

    /// `companion list`.
    pub fn companion_list(&self, flags: &Flags) -> Outcome<Vec<CompanionRow>> {
        self.run(flags, |p| Ok(p.companion_list()))
    }

    /// `mcp server list`.
    pub fn mcp_list(&self, flags: &Flags) -> Outcome<McpList> {
        self.run(flags, |p| Ok(p.mcp_list()))
    }

    /// `mcp server show <name>`.
    pub fn mcp_show(&self, flags: &Flags, name: &str) -> Outcome<McpShow> {
        self.run(flags, |p| p.mcp_show(name))
    }
}

/// A server a bundle ships and where from.
pub(crate) struct Served {
    pub server: McpServer,
    /// The canonical reference of the shipping bundle.
    pub source: String,
}

impl Project {
    fn llm_list(&self) -> Vec<LlmRow> {
        let mut names: BTreeSet<String> = ENGINES.iter().map(ToString::to_string).collect();
        names.extend(self.config.label_names());
        let primary = self.config.primary_label();
        names
            .into_iter()
            .map(|label| LlmRow {
                default: !label.is_empty() && label == primary,
                authored: self.config.authored_labels.contains(&label),
                runtimes: RUNTIMES.iter().map(ToString::to_string).collect(),
                label,
            })
            .collect()
    }

    fn companion_list(&self) -> Vec<CompanionRow> {
        self.config
            .companions()
            .into_iter()
            .map(|name| {
                let bin = if matches!(name.as_str(), "ltk" | "taskloom" | "reprise") {
                    name.clone()
                } else {
                    format!("ctxloom-companion-{name}")
                };
                let path = self.binaries.find(&bin);
                CompanionRow {
                    name,
                    resolves: path.is_some(),
                    path: path.unwrap_or_default(),
                    bin,
                }
            })
            .collect()
    }

    /// The servers the profiles' bundles ship, a name claimed by two
    /// different bundles withheld from the second; those the profiles
    /// exclude left out. A bundle that cannot be read is a finding.
    pub(crate) fn served(
        &self,
        set: &[ResolvedProfile],
        findings: &mut Vec<library::findings::Finding>,
    ) -> BTreeMap<String, Served> {
        let excluded: BTreeSet<&str> = set
            .iter()
            .flat_map(|p| p.exclude_mcp.iter().map(String::as_str))
            .collect();
        let mut out: BTreeMap<String, Served> = BTreeMap::new();
        let mut note = |finding: library::findings::Finding| {
            if !findings.contains(&finding) {
                findings.push(finding);
            }
        };
        for reference in set.iter().flat_map(|p| &p.bundles) {
            let Ok(entry) = self.catalog.lookup(reference) else {
                note(library::findings::Finding::fail(
                    "bundle",
                    format!(
                        "failed to load bundle \"{reference}\"; the MCP servers and hooks it ships are not applied: bundle not found: {reference}"
                    ),
                    "run `ctxloom deps pull` or fix the bundle ref, or pass --degraded",
                ));
                continue;
            };
            let source = format!("ctxloom+local:{}", entry.name);
            for (name, server) in &entry.bundle.mcp {
                if excluded.contains(name.as_str()) {
                    continue;
                }
                match out.get(name) {
                    Some(held) if held.source != source => {
                        note(contest(name, &held.source, &source))
                    }
                    Some(_) => {}
                    None => {
                        out.insert(
                            name.clone(),
                            Served {
                                server: server.clone(),
                                source: source.clone(),
                            },
                        );
                    }
                }
            }
        }
        out
    }

    /// The servers of the default agent's profiles, as the project registers them.
    pub(crate) fn registered(&self) -> BTreeMap<String, Served> {
        let mut findings = Vec::new();
        let names = self.config.default_agent_profiles();
        let set = self
            .resolve_profiles(&names, &mut findings)
            .unwrap_or_default();
        self.served(&set, &mut findings)
    }

    fn mcp_list(&self) -> McpList {
        let mut servers: Vec<McpListRow> = self
            .registered()
            .into_iter()
            .map(|(name, s)| McpListRow {
                name,
                command: s.server.command,
                args: s.server.args,
                url: s.server.url,
                served_by: s.server.served_by,
                source: s.source,
            })
            .collect();
        servers.sort_by_key(|row| row.name.to_lowercase());
        McpList { servers }
    }

    fn mcp_show(&self, name: &str) -> Result<McpShow, AppError> {
        let registered = self.registered();
        let Some(found) = registered.get(name) else {
            return Err(AppError::McpNotFound {
                name: name.to_string(),
            });
        };
        let entries: Vec<McpRow> = vec![mcp_row(name, &found.server, &found.source)];
        Ok(McpShow {
            name: name.to_string(),
            found: true,
            entries,
        })
    }

    /// The label and the engine behind it that a launch under `llm` would take
    /// (`profile_llm` when none was named): the configured label's type and
    /// model, the engine a bare engine name is, else the built-in default.
    pub(crate) fn backend_of(&self, llm: &str, profile_llm: &str) -> (String, String, String) {
        let label = [llm, profile_llm]
            .into_iter()
            .find(|l| !l.is_empty())
            .map_or_else(|| self.config.primary_label(), ToString::to_string);
        let (backend, model) = match self.config.llm_entry(&label) {
            Some(entry) => (
                entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_ENGINE)
                    .to_string(),
                entry
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            None if ENGINES.contains(&label.as_str()) => (label.clone(), String::new()),
            None => (DEFAULT_ENGINE.to_string(), String::new()),
        };
        (label, backend, model)
    }

    fn agent_show(
        &mut self,
        name: &str,
        say: &dyn Fn(&dyn Diagnostic) -> String,
    ) -> Result<AgentShow, AppError> {
        let Some(agent) = self.config.agent(name) else {
            return Err(AppError::AgentNotFound {
                name: name.to_string(),
            });
        };
        let definition = agent.row();
        match self.resolve_agent(&agent) {
            Ok((resolved, requirements, requirements_error)) => Ok(AgentShow {
                definition,
                resolved: Some(resolved),
                resolution_error: String::new(),
                requirements,
                requirements_error,
            }),
            Err(error) => Ok(AgentShow {
                definition,
                resolved: None,
                resolution_error: format!("agent {name:?}: {}", why(&error, say)),
                requirements: Vec::new(),
                requirements_error: String::new(),
            }),
        }
    }

    /// The engine and context an agent resolves to, and what it requires.
    fn resolve_agent(
        &mut self,
        agent: &Agent,
    ) -> Result<(ResolvedAgent, Vec<Requirement>, String), AppError> {
        let mut findings = Vec::new();
        let names = if agent.profiles.is_empty() {
            self.config.default_agent_profiles()
        } else {
            agent.profiles.clone()
        };
        let set = self.resolve_profiles(&names, &mut findings)?;
        let assembled = self.assemble_set(&set, &[], &[], &mut findings);
        let (label, backend, model) = self.backend_of(
            &agent.llm,
            &set.iter()
                .map(|p| p.llm.clone())
                .find(|l| !l.is_empty())
                .unwrap_or_default(),
        );
        let runtime = if agent.runtime.is_empty() {
            self.config.text("runtime")
        } else {
            agent.runtime.clone()
        };
        let entry = self.config.llm_entry(&label);
        let effective = effective_posture(&backend, &agent.permissions, entry);
        let approver = approver_of(&agent.permissions, entry, &self.config.value);
        let home = if agent.home_mode.is_empty() {
            "session".to_string()
        } else {
            agent.home_mode.clone()
        };
        let resolved = ResolvedAgent {
            name: agent.name.clone(),
            llm: agent.llm.clone(),
            profiles: agent.profiles.clone(),
            label,
            backend,
            model,
            context: assembled.context.clone(),
            fragments: assembled.loaded.clone(),
            runtime,
            permissions: crate::json::permissions(&agent.permissions),
            effective_permissions: effective,
            approver,
            driving: agent.driving.clone(),
            home_mode: home,
        };
        let (requirements, requirements_error) = self.requirements(&set);
        Ok((resolved, requirements, requirements_error))
    }

    /// The commands an agent needs on its PATH and who asks for each.
    fn requirements(&self, set: &[ResolvedProfile]) -> (Vec<Requirement>, String) {
        let mut found: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
        for profile in set {
            for reference in &profile.bundles {
                match self.catalog.lookup(reference) {
                    Ok(entry) => {
                        for command in &entry.bundle.requires_commands {
                            found
                                .entry(command.clone())
                                .or_default()
                                .insert(("bundle".to_string(), reference.clone()));
                        }
                    }
                    Err(_) => {
                        return (
                            Vec::new(),
                            format!(
                                "reading bundle {reference} for its requirements: bundle not found: {reference}"
                            ),
                        )
                    }
                }
            }
        }
        for (name, served) in self.served(set, &mut Vec::new()) {
            if !served.server.url.is_empty() || !served.server.served_by.is_empty() {
                continue;
            }
            let Some(first) = served.server.command.split_whitespace().next() else {
                continue;
            };
            if STAGED.contains(&first) {
                continue;
            }
            found
                .entry(first.to_string())
                .or_default()
                .insert(("mcp".to_string(), name));
        }
        let requirements = found
            .into_iter()
            .map(|(command, sources)| Requirement {
                command,
                sources: sources
                    .into_iter()
                    .map(|(kind, reference)| RequirementSource { kind, reference })
                    .collect(),
            })
            .collect();
        (requirements, String::new())
    }
}

/// The posture a run takes without `--permissions`, in the engine's words:
/// the agent's block for the engine, else the label's, else the engine's own
/// default (only an engine with a permission model has one).
fn effective_posture(backend: &str, binding: &Value, label: Option<&Value>) -> String {
    if backend != "claude-code" {
        return String::new();
    }
    if let Value::Map(blocks) = binding {
        let engines: Vec<&(String, Value)> = blocks
            .iter()
            .filter(|(k, _)| {
                !matches!(
                    k.as_str(),
                    "approver" | "approval_timeout" | "sandbox" | "network"
                )
            })
            .collect();
        if let Some((_, block)) = engines.iter().find(|(k, _)| k == backend) {
            if let Some(mode) = block.get("mode").and_then(Value::as_str) {
                return mode.to_string();
            }
        } else if !engines.is_empty() {
            return String::new();
        }
    }
    if let Some(mode) = label
        .and_then(|l| l.get("permissions"))
        .and_then(|p| p.get("mode"))
        .and_then(Value::as_str)
    {
        return mode.to_string();
    }
    "acceptEdits".to_string()
}

/// Who answers what the posture leaves open in a headless run: the binding's
/// declaration, then the label's, then the project's, else the human.
fn approver_of(binding: &Value, label: Option<&Value>, config: &Value) -> String {
    let declared = |block: Option<&Value>| -> Option<String> {
        block
            .and_then(|b| b.get("approver"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    };
    declared(Some(binding))
        .or_else(|| declared(label.and_then(|l| l.get("permissions"))))
        .or_else(|| declared(config.get("permissions")))
        .unwrap_or_else(|| "human".to_string())
}

/// Why an agent did not resolve, in the words Go gives.
fn why(error: &AppError, say: &dyn Fn(&dyn Diagnostic) -> String) -> String {
    match error {
        AppError::Compose { profile, cause } => {
            format!(
                "compose profiles: profile {profile}: {}",
                say(cause.as_ref())
            )
        }
        other => say(other),
    }
}

/// A server name two different bundles declare: the second is withheld.
fn contest(name: &str, first: &str, second: &str) -> library::findings::Finding {
    library::findings::Finding::fail(
        "bundle",
        format!(
            "MCP server name {name:?} is claimed by two different sources: {first:?} claimed it first and {second:?} also declares it; {second:?}'s server is withheld so one bundle's server cannot silently answer for another's declared name"
        ),
        "rename the server in one of the two bundles, or exclude it with `exclude_mcp:` in your profile",
    )
}
