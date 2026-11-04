//! `run --dry-run` (the launch a run would carry, without launching it) and
//! `context assemble` (the assembled context of a selection).
//!
//! The launch itself, the engines' surfaces and the isolation a launch settles
//! are later slices'; what a preview shows of them is left out here, and said.

use library::compose::{self, Options, Package, SelectRequest};
use library::findings::{Class, Finding};
use library::profile::ResolvedProfile;
use library::tokens;

use crate::agents::ENGINES;
use crate::config::Agent;
use crate::error::{AppError, Notice};
use crate::project::{Library, Outcome, Project};
use crate::views::{Assembled, DryRun, FindingRow};
use crate::Flags;

/// What `run --dry-run` is asked.
#[derive(Clone, Debug, Default)]
pub struct RunRequest {
    pub agent: String,
    pub profile: String,
    pub fragments: Vec<String>,
    pub tags: Vec<String>,
    pub llm: String,
    pub prompt: String,
    /// `--command <name>`: the saved command whose body is the prompt.
    pub saved_command: String,
    pub workspace: String,
}

/// What `context assemble` is asked.
#[derive(Clone, Debug, Default)]
pub struct AssembleRequest {
    pub profiles: Vec<String>,
    pub fragments: Vec<String>,
    pub tags: Vec<String>,
}

/// A dry run: the plan, and the findings the startup gate refuses on.
pub struct Previewed {
    pub plan: DryRun,
    /// The fatal findings: with these, the plan is shown and the run refused.
    pub fatal: Vec<FindingRow>,
}

impl Library {
    /// `run --dry-run`.
    pub fn dry_run(&self, flags: &Flags, request: &RunRequest) -> Outcome<Previewed> {
        self.run(flags, |p| p.dry_run(request))
    }

    /// `context assemble`.
    pub fn assemble(&self, flags: &Flags, request: &AssembleRequest) -> Outcome<Assembled> {
        self.run(flags, |p| p.assemble(request))
    }
}

/// What a run is bound to.
struct Selection {
    agent: Option<Agent>,
    profiles: Vec<String>,
    fragments: Vec<String>,
    tags: Vec<String>,
}

impl Project {
    /// The workspace and runtime a launch settles on: the flag or the
    /// binding's, else the project's default, else none and the host.
    fn axes(&self, request: &RunRequest, agent: Option<&Agent>) -> (String, String) {
        let first = |declared: &str, default: String, otherwise: &str| {
            [declared.to_string(), default]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_else(|| otherwise.to_string())
        };
        (
            first(&request.workspace, self.config.text("workspace"), "none"),
            first(
                agent.map_or("", |a| a.runtime.as_str()),
                self.config.text("runtime"),
                "host",
            ),
        )
    }

    /// The prompt: the one given, else the saved command's body.
    fn prompt_of(&self, request: &RunRequest) -> Result<String, AppError> {
        if !request.prompt.is_empty() || request.saved_command.is_empty() {
            return Ok(request.prompt.clone());
        }
        let found = self
            .catalog
            .read_command(&request.saved_command)
            .ok()
            .and_then(|found| found.into_iter().next())
            .ok_or_else(|| AppError::SavedCommand {
                name: request.saved_command.clone(),
            })?;
        Ok(found.content)
    }

    /// The profiles, fragments and tags a run takes: the agent's binding, the
    /// hand-assembled flags, or the default agent's binding.
    fn selection(&self, request: &RunRequest) -> Result<Selection, AppError> {
        if !request.agent.is_empty() {
            return self.binding(&request.agent);
        }
        let by_hand = !request.profile.is_empty()
            || !request.fragments.is_empty()
            || !request.tags.is_empty();
        if by_hand {
            return Ok(Selection {
                agent: None,
                profiles: if request.profile.is_empty() {
                    Vec::new()
                } else {
                    vec![request.profile.clone()]
                },
                fragments: request.fragments.clone(),
                tags: request.tags.clone(),
            });
        }
        self.binding(&self.config.default_agent())
    }

    fn binding(&self, name: &str) -> Result<Selection, AppError> {
        match self.config.agent(name) {
            Some(agent) => Ok(Selection {
                profiles: agent.profiles.clone(),
                agent: Some(agent),
                fragments: Vec::new(),
                tags: Vec::new(),
            }),
            None if self.flags.degraded && name == self.config.default_agent() => Ok(Selection {
                agent: None,
                profiles: Vec::new(),
                fragments: Vec::new(),
                tags: Vec::new(),
            }),
            None if !name.is_empty() => Err(AppError::LaunchNoAgent {
                name: name.to_string(),
            }),
            None => Err(self.no_default_agent()),
        }
    }

    fn no_default_agent(&self) -> AppError {
        if self.no_project() {
            AppError::LaunchNoProject
        } else if !self.config.agents().is_empty() {
            AppError::LaunchNoDefault
        } else {
            AppError::LaunchNoAgents
        }
    }

    /// An explicit `--llm` must name a configured label or an engine.
    fn check_llm(&self, label: &str) -> Result<(), AppError> {
        if label.is_empty() || self.config.llm_entry(label).is_some() || ENGINES.contains(&label) {
            return Ok(());
        }
        let mut usable: Vec<String> = self.config.label_names();
        for engine in ENGINES.iter().filter(|e| !e.starts_with("mock")) {
            if !usable.iter().any(|u| u == engine) {
                usable.push((*engine).to_string());
            }
        }
        usable.sort();
        Err(AppError::UnknownLlm {
            label: label.to_string(),
            usable: usable.join(", "),
        })
    }

    /// The selection of `set` and the caller's asks, assembled. Whether the
    /// premised fragments load or are indexed follows the engine.
    pub(crate) fn assemble_set(
        &self,
        set: &[ResolvedProfile],
        fragments: &[String],
        tags: &[String],
        findings: &mut Vec<Finding>,
    ) -> Package {
        let request = SelectRequest {
            fragments: fragments.to_vec(),
            tags: tags.to_vec(),
        };
        let selection = compose::select(set, &self.catalog, &request, findings);
        let options = Options {
            prefer_distilled: self.config.use_distilled(),
            static_assembly: false,
        };
        let package = compose::assemble(&self.catalog, &selection, &options);
        findings.extend(package.findings.iter().cloned());
        package
    }

    fn dry_run(&mut self, request: &RunRequest) -> Result<Previewed, AppError> {
        if !request.agent.is_empty() {
            let other = if !request.profile.is_empty() {
                Some("profile")
            } else if !request.fragments.is_empty() {
                Some("fragment")
            } else if !request.tags.is_empty() {
                Some("tag")
            } else {
                None
            };
            if let Some(other) = other {
                return Err(AppError::AgentExclusive {
                    other: other.to_string(),
                });
            }
        }
        if !matches!(request.workspace.as_str(), "" | "none" | "worktree") {
            return Err(AppError::Workspace {
                value: request.workspace.clone(),
            });
        }
        self.check_llm(&request.llm)?;
        let prompt = self.prompt_of(request)?;
        let selection = self.selection(request)?;
        let mut findings = Vec::new();
        let package = self.compose(&selection, &mut findings)?;
        let agent_llm = selection
            .agent
            .as_ref()
            .map(|a| a.llm.clone())
            .unwrap_or_default();
        let wanted = if request.llm.is_empty() {
            agent_llm
        } else {
            request.llm.clone()
        };
        let (label, backend, _) = self.backend_of(&wanted, &package.selection.llm);
        self.check_llm(&label)?;
        let degraded = self.flags.degraded;
        for finding in &findings {
            self.notices.push(Notice::finding(finding));
        }
        let rows: Vec<FindingRow> = findings
            .iter()
            .filter(|f| f.class == Class::Fail)
            .map(|f| FindingRow {
                kind: f.kind.clone(),
                text: f.text.clone(),
                remedy: f.remedy.clone(),
                fatal: !degraded,
            })
            .collect();
        let fatal = rows.iter().filter(|r| r.fatal).cloned().collect();
        let plan = DryRun {
            agent: request.agent.clone(),
            workspace: request.workspace.clone(),
            runtime: selection
                .agent
                .as_ref()
                .map(|a| a.runtime.clone())
                .unwrap_or_default(),
            llm: label,
            backend,
            profiles: package.selection.profiles.clone(),
            fragments: package.loaded.clone(),
            tokens: tokens::estimate(&package.context) as u64,
            context: package.context,
            prompt,
            findings: rows,
            axes: self.axes(request, selection.agent.as_ref()),
        };
        Ok(Previewed { plan, fatal })
    }

    /// The package of a selection: profiles resolved, the bundles they name
    /// checked, fragments selected and assembled; a selection that asked for
    /// fragments or tags and got none is refused.
    fn compose(
        &self,
        selection: &Selection,
        findings: &mut Vec<Finding>,
    ) -> Result<Package, AppError> {
        if selection.profiles.is_empty()
            && selection.fragments.is_empty()
            && selection.tags.is_empty()
        {
            return Ok(Package::default());
        }
        let set = self.resolve_profiles(&selection.profiles, findings)?;
        self.served(&set, findings);
        let package = self.assemble_set(&set, &selection.fragments, &selection.tags, findings);
        refuse_empty(selection, &package)?;
        Ok(package)
    }

    fn assemble(&mut self, request: &AssembleRequest) -> Result<Assembled, AppError> {
        let selection = Selection {
            agent: None,
            profiles: request.profiles.clone(),
            fragments: request.fragments.clone(),
            tags: request.tags.clone(),
        };
        let mut findings = Vec::new();
        let by_default = selection.profiles.is_empty()
            && selection.fragments.is_empty()
            && selection.tags.is_empty();
        let selection = if by_default {
            Selection {
                profiles: self.config.default_agent_profiles(),
                ..selection
            }
        } else {
            selection
        };
        let package = self.compose(&selection, &mut findings)?;
        for finding in &findings {
            self.notices.push(Notice::finding(finding));
        }
        let missing_fragments = package
            .selection
            .explicit
            .iter()
            .filter(|asked| !package.loaded.contains(asked))
            .cloned()
            .collect();
        Ok(Assembled {
            profiles: package.selection.profiles.clone(),
            context: package.context.clone(),
            fragments_loaded: package.loaded.clone(),
            missing_fragments,
            missing_tags: package.selection.missing_tags.clone(),
        })
    }
}

/// An explicit selection that matched nothing is refused: asked for fragments
/// by name or tag and would get none.
fn refuse_empty(selection: &Selection, package: &Package) -> Result<(), AppError> {
    let missing: Vec<&String> = package
        .selection
        .explicit
        .iter()
        .filter(|asked| !package.loaded.contains(asked))
        .collect();
    if !selection.fragments.is_empty() && missing.len() == selection.fragments.len() {
        return Err(AppError::NoFragments {
            detail: format!(
                "requested fragments not found: {}",
                missing
                    .iter()
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    let missing_tags = &package.selection.missing_tags;
    if !selection.tags.is_empty() && missing_tags.len() == selection.tags.len() {
        return Err(AppError::NoFragments {
            detail: format!("no fragment matches tag(s): {}", missing_tags.join(", ")),
        });
    }
    Ok(())
}
