//! The engines' callbacks: `hook session-start` and `hook skill-mates`
//! (Go's `hook_session_start.go`, `hook_skill_mates.go`,
//! `skill_mates_decide.go`). A hook must never fail the host engine's
//! startup: what it can foresee going wrong is silence, and only a payload it
//! cannot read, or an engine it does not know, is an error.

use library_application::{Flags, PackageRequest};
use surface_ports::{EngineSurfaces, HookResponse};

use crate::{Setup, SetupError};

/// The nudge Go's `operations.AgentSetupNudge` hands a project that has
/// profiles and no agents.
const AGENT_SETUP_NUDGE: &str = "ctxloom: this project has profiles but no agents configured. Run `ctxloom init prompt` (or ask your agent to) to bind engines to profiles - the standard trio is an orchestrator you drive, a containerized developer, and a cheap finder, plus per-language code reviewers and any other roles you want to orchestrate.";

/// The tag that links skills into a group: `ctxloom:link_id=<id>`.
const LINK_TAG: &str = "ctxloom:link_id=";

impl Setup {
    fn firing(&self, engine: &str) -> Result<&dyn EngineSurfaces, SetupError> {
        self.engines
            .get(engine)
            .ok_or_else(|| SetupError::UnknownHookEngine {
                engine: engine.to_string(),
                known: self.engines.names().join(", "),
            })
    }

    /// The engine's answer to a SessionStart: the session-start notice and
    /// never the project's context, which the session already has as its
    /// system prompt.
    pub fn session_start(&self, engine: &str, payload: &str) -> Result<String, SetupError> {
        let engine = self.firing(engine)?;
        let codec = engine.codec();
        codec
            .decode("session_start", payload)
            .map_err(SetupError::cause)?;
        let flags = Flags {
            degraded: true,
            ..Flags::default()
        };
        let package = self.library.package(&flags, &PackageRequest::default());
        let notice = match package.result {
            Ok(p) if !p.has_agents && p.has_profiles => AGENT_SETUP_NUDGE.to_string(),
            _ => String::new(),
        };
        codec
            .encode(
                "session_start",
                &HookResponse {
                    notice,
                    ..HookResponse::default()
                },
            )
            .map_err(SetupError::cause)
    }

    /// The engine's answer to a PostToolUse: when the tool call completed a
    /// skill of a link group, the group's mates the session has not invoked
    /// yet; silence after every other call.
    pub fn skill_mates(&self, engine: &str, payload: &str) -> Result<String, SetupError> {
        let engine = self.firing(engine)?;
        let codec = engine.codec();
        let event = codec
            .decode("post_tool", payload)
            .map_err(SetupError::cause)?;
        let mut response = HookResponse::default();
        if !event.skill.is_empty() {
            let transcript = if event.transcript.is_empty() {
                None
            } else {
                self.store.read_absolute(&event.transcript).ok().flatten()
            };
            let invoked = transcript
                .and_then(|b| String::from_utf8(b).ok())
                .map(|t| codec.invoked_skills(&t))
                .unwrap_or_default();
            response.context = self.mates_line(&event.skill, &invoked);
        }
        codec
            .encode("post_tool", &response)
            .map_err(SetupError::cause)
    }

    /// The one line the hook injects: the completed skill and the mates it
    /// leaves uninvoked, names only. Empty when there are none.
    fn mates_line(&self, completed: &str, invoked: &[String]) -> String {
        let flags = Flags {
            degraded: true,
            ..Flags::default()
        };
        let Ok(package) = self
            .library
            .package(&flags, &PackageRequest::default())
            .result
        else {
            return String::new();
        };
        let links = |tags: &[String]| -> Vec<String> {
            tags.iter()
                .filter_map(|t| t.strip_prefix(LINK_TAG).map(str::to_string))
                .collect()
        };
        let mut mates: Vec<String> = Vec::new();
        for skill in package.skills.iter().filter(|s| s.name == completed) {
            let ids = links(&skill.tags);
            for other in &package.skills {
                let shares = links(&other.tags).iter().any(|id| ids.contains(id));
                if other.bundle == skill.bundle
                    && other.name != completed
                    && shares
                    && !invoked.contains(&other.name)
                    && !mates.contains(&other.name)
                {
                    mates.push(other.name.clone());
                }
            }
        }
        mates.sort();
        if mates.is_empty() {
            return String::new();
        }
        format!(
            "Skill {completed} completed; linked skills not yet invoked this session: {}",
            mates.join(", ")
        )
    }
}

#[cfg(test)]
#[path = "hooks.test.rs"]
mod tests;
