//! What a profile set carries for an engine's native files (S9): the assembled
//! context, the commands, skills, MCP servers and hooks its bundles ship, and
//! ctxloom's own callbacks. The engines' surfaces are delivered by the setup
//! use cases; this is the read side they deliver from.

use std::collections::{BTreeMap, BTreeSet};

use angzarr_yaml::Value;
use ctxloom_ports::Scope;
use library::catalog::{item_tags, Catalog, Entry};
use library::document;
use library::findings::{Class, Finding};
use library::frontmatter::split_front_matter;
use library::model::Hook;
use library::refs;

use crate::agents::ENGINES;
use crate::error::{AppError, Notice};
use crate::project::{Library, Outcome, Project};
use crate::Flags;

/// A profile set to package.
#[derive(Clone, Debug, Default)]
pub struct PackageRequest {
    /// The profiles; none is the default agent's.
    pub profiles: Vec<String>,
}

/// A command a profile set carries.
#[derive(Clone, Debug)]
pub struct PackagedCommand {
    /// `<bundle>/<name>`.
    pub name: String,
    pub description: String,
    pub body: String,
}

/// One file of a skill package.
#[derive(Clone, Debug)]
pub struct PackagedFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

/// A skill a profile set carries.
#[derive(Clone, Debug)]
pub struct PackagedSkill {
    /// The `SKILL.md` frontmatter's `name`, else the package's.
    pub name: String,
    pub description: String,
    pub bundle: String,
    /// The bundle's tags then the skill's own.
    pub tags: Vec<String>,
    pub files: Vec<PackagedFile>,
}

/// An MCP server a profile set carries.
#[derive(Clone, Debug)]
pub struct PackagedMcp {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    /// `session-endpoint` when the running session serves it: nothing to
    /// register at rest.
    pub served_by: String,
}

/// A hook a profile set carries, and where it came from.
#[derive(Clone, Debug)]
pub struct PackagedHook {
    /// The unified event.
    pub event: String,
    pub hook: Hook,
    /// `profile-directory`, `bundle` or `context` (ctxloom's own).
    pub source_kind: String,
    /// The profile's name, or the bundle's reference; empty for ctxloom's own.
    pub source: String,
    /// Its place among the event's hooks, from 1.
    pub position: usize,
}

/// What a profile set carries.
#[derive(Debug, Default)]
pub struct EnginePackage {
    pub profiles: Vec<String>,
    pub context: String,
    pub commands: Vec<PackagedCommand>,
    pub skills: Vec<PackagedSkill>,
    pub mcp: Vec<PackagedMcp>,
    pub hooks: Vec<PackagedHook>,
    pub deny_tools: Vec<String>,
    /// Whether ctxloom manages the HUD statusline.
    pub statusline: bool,
    /// Whether distilled forms are preferred (`config.use_distilled`).
    pub use_distilled: bool,
    /// The engines the project's labels name.
    pub configured_engines: Vec<String>,
    /// The findings a strict run refuses over (none under `--degraded`).
    pub fatal: Vec<Notice>,
    /// Whether the project has any agent.
    pub has_agents: bool,
    /// Whether the project has any profile to bind an agent to.
    pub has_profiles: bool,
}

/// `entry`'s commands as `<owner>/<name>`, each once.
fn push_commands(
    owner: &str,
    entry: &Entry,
    seen: &mut BTreeSet<String>,
    package: &mut EnginePackage,
) {
    for (name, command) in &entry.bundle.commands {
        let full = format!("{owner}/{name}");
        if seen.insert(full.clone()) {
            package.commands.push(PackagedCommand {
                name: full,
                description: command.description.clone(),
                body: command.body.content.clone(),
            });
        }
    }
}

impl Library {
    /// What a profile set carries for an engine's native files.
    pub fn package(&self, flags: &Flags, request: &PackageRequest) -> Outcome<EnginePackage> {
        self.run(flags, |p| p.package(request))
    }

    /// `manage statusline install|uninstall`: whether ctxloom manages the
    /// HUD statusline, written to the project's config. The path written.
    pub fn set_statusline(&self, flags: &Flags, enabled: bool) -> Outcome<String> {
        self.run(flags, |p| p.set_statusline(enabled))
    }

    /// The engines this build knows by name.
    pub fn engine_names(&self) -> Vec<String> {
        ENGINES.iter().map(ToString::to_string).collect()
    }
}

impl Project {
    fn package(&mut self, request: &PackageRequest) -> Result<EnginePackage, AppError> {
        let names = if request.profiles.is_empty() {
            self.config.default_agent_profiles()
        } else {
            request.profiles.clone()
        };
        let mut findings: Vec<Finding> = Vec::new();
        let set = self.resolve_profiles(&names, &mut findings)?;
        let assembled = self.assemble_set(&set, &[], &[], &mut findings);
        // The registered companions' loadouts (asked once for the command)
        // ride the catalog the items are read from.
        let catalog = self.catalog_with_companions();
        let served = self.served_from(&catalog, &set, &mut findings);
        let selection = &assembled.selection;

        let mut package = EnginePackage {
            profiles: assembled.selection.profiles.clone(),
            context: assembled.context.clone(),
            deny_tools: selection.deny_tools.clone(),
            statusline: self.config.statusline(),
            use_distilled: self.config.use_distilled(),
            configured_engines: self.config.configured_engines(),
            has_agents: !self.config.agents().is_empty(),
            has_profiles: !self.config.default_agent_profiles().is_empty()
                || self
                    .catalog
                    .entries()
                    .iter()
                    .any(|e| !e.bundle.profiles.is_empty()),
            ..EnginePackage::default()
        };
        for (name, served) in served {
            let s = served.server;
            package.mcp.push(PackagedMcp {
                name,
                command: s.command,
                args: s.args,
                env: s.env,
                url: s.url,
                headers: s.headers,
                served_by: s.served_by,
            });
        }
        self.items_of(&catalog, selection, &mut package);
        package.hooks = self.hooks_of(&catalog, &set, selection);

        let degraded = self.flags.degraded;
        for finding in &findings {
            if finding.class == Class::Fail && !degraded {
                package.fatal.push(Notice::finding(finding));
            }
            self.notices.push(Notice::finding(finding));
        }
        Ok(package)
    }

    /// The commands and skills the profiles curate by reference when they
    /// curate any of that kind; otherwise those of the bundles they include.
    /// Each once (Go's composite assembly: a curation replaces the bundles'
    /// set, it does not add to it).
    fn items_of(
        &self,
        catalog: &Catalog,
        selection: &library::compose::Selection,
        package: &mut EnginePackage,
    ) {
        let mut commands: BTreeSet<String> = BTreeSet::new();
        let mut skills: BTreeSet<String> = BTreeSet::new();
        for reference in &selection.bundles {
            let Ok(entry) = catalog.lookup(reference) else {
                continue;
            };
            if selection.commands.is_empty() {
                push_commands(&entry.name, entry, &mut commands, package);
            }
            if selection.skills.is_empty() {
                for name in entry.bundle.skills.keys() {
                    self.add_skill(&entry.name, name, &mut skills, package);
                }
            }
        }
        for ask in &selection.commands {
            for read in catalog.read_command(ask).unwrap_or_default() {
                if commands.insert(read.name.clone()) {
                    package.commands.push(PackagedCommand {
                        name: read.name,
                        description: read.description,
                        body: read.content,
                    });
                }
            }
        }
        // A companion's commands are unconditional: a curation names bundle
        // commands, never theirs (Go's `fromBundles(CompanionRefs())`).
        for (companion, entry) in catalog.companion_entries() {
            push_commands(companion, entry, &mut commands, package);
        }
        for ask in &selection.skills {
            let Some((base, selector)) = ask.split_once('#') else {
                continue;
            };
            let Ok((refs::ItemKind::Skill, item)) = refs::parse_selector(selector) else {
                continue;
            };
            if let Ok(entry) = catalog.lookup(base) {
                self.add_skill(&entry.name, &item, &mut skills, package);
            }
        }
    }

    fn add_skill(
        &self,
        bundle: &str,
        name: &str,
        seen: &mut BTreeSet<String>,
        package: &mut EnginePackage,
    ) {
        let Ok(entry) = self.catalog.lookup(&format!("ctxloom+local:{bundle}")) else {
            return;
        };
        let Some(skill) = entry.bundle.skills.get(name) else {
            return;
        };
        if !seen.insert(format!("{bundle}#{name}")) {
            return;
        }
        let descriptor = skill.files.iter().find(|f| f.path == "SKILL.md");
        let (declared, description) = descriptor
            .and_then(|f| String::from_utf8(f.bytes.clone()).ok())
            .and_then(|text| {
                let split = split_front_matter(&text)?;
                let meta = angzarr_yaml::parse(split.meta).ok()?;
                let field = |key: &str| {
                    meta.get(key)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                Some((field("name"), field("description")))
            })
            .unwrap_or_default();
        package.skills.push(PackagedSkill {
            name: if declared.is_empty() {
                name.to_string()
            } else {
                declared
            },
            description,
            bundle: bundle.to_string(),
            tags: item_tags(&entry.bundle.tags, &skill.tags),
            files: skill
                .files
                .iter()
                .map(|f| PackagedFile {
                    path: f.path.clone(),
                    bytes: f.bytes.clone(),
                    executable: f.executable,
                })
                .collect(),
        });
    }

    /// Every hook that fires, per event, in the order it fires in: the
    /// profiles' own, then the companions' (by name), then the profiles'
    /// bundles' (each bundle's `order:` sequencing only its own), then
    /// ctxloom's callbacks (Go's `ResolveBundleHooksFor`). Merging is by pure
    /// append.
    fn hooks_of(
        &self,
        catalog: &Catalog,
        set: &[library::profile::ResolvedProfile],
        selection: &library::compose::Selection,
    ) -> Vec<PackagedHook> {
        let mut by_event: BTreeMap<String, Vec<PackagedHook>> = BTreeMap::new();
        let mut add = |event: &str, hook: Hook, kind: &str, source: &str| {
            by_event
                .entry(event.to_string())
                .or_default()
                .push(PackagedHook {
                    event: event.to_string(),
                    hook,
                    source_kind: kind.to_string(),
                    source: source.to_string(),
                    position: 0,
                });
        };
        for profile in set {
            for (event, hooks) in &profile.hooks.unified {
                for (i, value) in hooks.iter().enumerate() {
                    let at = format!("profiles.{}.hooks.{event}[{i}]", profile.name);
                    if let Ok(hook) = document::hook(value, event, &i.to_string(), &at) {
                        add(event, hook, "profile-directory", &profile.name);
                    }
                }
            }
        }
        let companions = catalog.companion_entries().into_iter().map(|(_, e)| e);
        let bundles = selection
            .bundles
            .iter()
            .filter_map(|reference| catalog.lookup(reference).ok());
        for entry in companions.chain(bundles) {
            let source = catalog.source_of(entry);
            for hook in &entry.bundle.hooks {
                add(&hook.event.clone(), hook.clone(), "bundle", &source);
            }
        }
        for (event, verb, tool) in [
            ("session_start", "session-start", ""),
            ("post_tool", "skill-mates", "skill"),
        ] {
            add(
                event,
                Hook {
                    event: event.to_string(),
                    name: verb.to_string(),
                    command: "ctxloom".to_string(),
                    args: vec!["hook".to_string(), verb.to_string()],
                    kind: "command".to_string(),
                    timeout: 15,
                    tool: tool.to_string(),
                    ..Hook::default()
                },
                "context",
                "",
            );
        }
        by_event
            .into_values()
            .flat_map(|hooks| {
                hooks.into_iter().enumerate().map(|(i, mut h)| {
                    h.position = i + 1;
                    h
                })
            })
            .collect()
    }

    fn set_statusline(&mut self, enabled: bool) -> Result<String, AppError> {
        self.require_project()?;
        self.update_config(Scope::Project, "config", |held| {
            let mut entries = match held {
                Some(Value::Map(entries)) => entries,
                _ => Vec::new(),
            };
            entries.retain(|(key, _)| key != "statusline");
            entries.push(("statusline".to_string(), Value::Scalar(enabled.to_string())));
            Ok(Some(Value::Map(entries)))
        })
    }
}

#[cfg(test)]
#[path = "package.test.rs"]
mod tests;
