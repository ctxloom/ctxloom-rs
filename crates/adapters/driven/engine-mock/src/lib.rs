//! The mock engine's surface half (S9; Go's `internal/engines/mock`
//! `surfaces.go`): a registered engine with real surfaces of its own. Every
//! kind is a file a test can read without an engine binary: the context file
//! at the root, the rest under `.mock/`.
//!
//! Four engines are registered from here. `mock` carries every kind.
//! `mock-launch` is the home-keyed variant: its declaration carries a context
//! surface and no other, the plan routes the rest to nothing as accepted
//! losses, and the materialize report names each with the reason.
//! `mock-lossy` carries every kind but has no native `session_start` or
//! `session_end` hook event: its hook file leaves them out and the report
//! names each the profile configures. `mock-noskills` declares a skills
//! surface and exports nothing to it, for the callers' missing-skills arm.

mod codec;

use std::collections::BTreeMap;

use surface_ports::{
    Claim, EngineSurfaces, Hook, HookCodec, Inputs, Json, Kind, Planned, SurfaceError,
    SurfaceStore, Wiring,
};

pub use codec::Codec;

/// The context file the mock reads, at the root where a human would look.
pub const CONTEXT_FILE: &str = "MOCK_CONTEXT.md";
const MCP_FILE: &str = ".mock/mcp.json";
const SETTINGS_FILE: &str = ".mock/settings.json";
const HOOKS_FILE: &str = ".mock/hooks.json";
const COMMANDS_DIR: &str = ".mock/commands";
const SKILLS_DIR: &str = ".mock/skills";

/// A mock engine.
pub struct Mock {
    name: &'static str,
    /// Whether it carries every kind (else the context alone).
    full: bool,
    /// The hook events it has no native form for (the lossy variant's two).
    lost_hooks: &'static [&'static str],
    /// Whether it writes the skills it is handed (else it declares the
    /// surface and exports nothing to it).
    exports_skills: bool,
    codec: Codec,
}

/// The events `mock-lossy` drops: two, so a report that groups several is
/// exercised.
const LOST_EVENTS: &[&str] = &["session_start", "session_end"];

impl Mock {
    /// `mock`: every kind.
    pub fn standard() -> Mock {
        Mock {
            name: "mock",
            full: true,
            lost_hooks: &[],
            exports_skills: true,
            codec: Codec,
        }
    }

    /// `mock-lossy`: every kind, but no native `session_start` or
    /// `session_end` hook event.
    pub fn lossy() -> Mock {
        Mock {
            name: "mock-lossy",
            lost_hooks: LOST_EVENTS,
            ..Mock::standard()
        }
    }

    /// `mock-noskills`: a skills surface that is never written to.
    pub fn no_skills() -> Mock {
        Mock {
            name: "mock-noskills",
            exports_skills: false,
            ..Mock::standard()
        }
    }

    /// `mock-launch`: the context alone.
    pub fn home_keyed() -> Mock {
        Mock {
            name: "mock-launch",
            full: false,
            lost_hooks: &[],
            exports_skills: true,
            codec: Codec,
        }
    }
}

fn whole(path: &str, doc: &Json) -> Claim {
    Claim::Whole {
        path: path.to_string(),
        bytes: format!("{}\n", doc.pretty()).into_bytes(),
        executable: false,
    }
}

/// A name that is safe to make a path of.
fn safe(name: &str) -> bool {
    ctxloom_ports::is_plain_relative(name) && !name.contains('\\')
}

/// The hook as the mock's native hook file spells it.
fn hook_json(engine: &str, hook: &Hook) -> Json {
    let mut members = hook.json_members(engine);
    if !hook.tool.is_empty() {
        members.push(("tool", Json::text(hook.tool.as_str())));
    }
    if !hook.matcher.is_empty() {
        members.push(("matcher", Json::text(hook.matcher.as_str())));
    }
    Json::object(members)
}

impl EngineSurfaces for Mock {
    fn name(&self) -> &str {
        self.name
    }

    fn carries(&self, kind: Kind) -> bool {
        self.full || kind == Kind::Context
    }

    fn hook_losses(&self) -> Vec<(String, String)> {
        self.lost_hooks
            .iter()
            .map(|event| {
                (
                    (*event).to_string(),
                    format!("{} has no native {event} event", self.name),
                )
            })
            .collect()
    }

    fn plan(&self, kind: Kind, inputs: &Inputs) -> Result<Planned, SurfaceError> {
        let mut planned = Planned::default();
        if !self.carries(kind) {
            return Ok(planned);
        }
        match kind {
            Kind::Context => planned.claims.push(Claim::Section {
                path: inputs
                    .context_file
                    .clone()
                    .unwrap_or_else(|| CONTEXT_FILE.to_string()),
                text: inputs.context.clone(),
            }),
            Kind::Mcp => {
                let servers = inputs.mcp.iter().map(|s| {
                    let mut members = Vec::new();
                    if !s.command.is_empty() {
                        members.push(("command", Json::text(s.command.as_str())));
                    }
                    if !s.args.is_empty() {
                        members.push(("args", Json::strings(&s.args)));
                    }
                    if !s.url.is_empty() {
                        members.push(("url", Json::text(s.url.as_str())));
                    }
                    (s.name.clone(), Json::object(members))
                });
                planned.claims.push(whole(
                    MCP_FILE,
                    &Json::object([("mcpServers", Json::Object(servers.collect()))]),
                ));
            }
            Kind::Settings => planned.claims.push(whole(
                SETTINGS_FILE,
                &Json::object([
                    ("denyTools", Json::strings(&inputs.deny_tools)),
                    ("statusline", Json::Bool(inputs.statusline)),
                ]),
            )),
            Kind::Hooks => {
                let events = inputs
                    .hooks
                    .iter()
                    .filter(|(event, _)| !self.lost_hooks.contains(&event.as_str()))
                    .map(|(event, list)| {
                        (
                            event.clone(),
                            Json::Array(list.iter().map(|h| hook_json(self.name, h)).collect()),
                        )
                    });
                planned
                    .claims
                    .push(whole(HOOKS_FILE, &Json::Object(events.collect())));
            }
            Kind::Commands => {
                for command in &inputs.commands {
                    if !safe(&command.name) {
                        planned.skipped.push(format!(
                            "skipping command {:?}: its name is not a safe file name",
                            command.name
                        ));
                        continue;
                    }
                    planned.claims.push(Claim::Whole {
                        path: format!("{COMMANDS_DIR}/{}.md", command.name),
                        bytes: command.body.clone().into_bytes(),
                        executable: false,
                    });
                }
            }
            Kind::Skills if !self.exports_skills => {}
            Kind::Skills => {
                for skill in &inputs.skills {
                    if !safe(&skill.name) {
                        planned.skipped.push(format!(
                            "skipping skill {:?}: its name is not a safe file name",
                            skill.name
                        ));
                        continue;
                    }
                    for file in skill.files.iter().filter(|f| safe(&f.path)) {
                        planned.claims.push(Claim::Whole {
                            path: format!("{SKILLS_DIR}/{}/{}", skill.name, file.path),
                            bytes: file.bytes.clone(),
                            executable: file.executable,
                        });
                    }
                }
            }
        }
        Ok(planned)
    }

    fn wiring(&self, store: &dyn SurfaceStore, target: &str) -> Result<Wiring, SurfaceError> {
        let parse = |rel: &str| -> Result<Option<Json>, SurfaceError> {
            Ok(store
                .read(target, rel)?
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|t| Json::parse(&t).ok()))
        };
        let settings = parse(SETTINGS_FILE)?;
        let hooks = parse(HOOKS_FILE)?;
        let mcp = parse(MCP_FILE)?;
        let callback = |doc: &Json| match doc {
            Json::Object(events) => events.iter().any(|(_, list)| {
                list.as_array().is_some_and(|l| {
                    l.iter().any(|h| {
                        h.get("command").and_then(Json::as_str) == Some("ctxloom")
                            && h.get("args")
                                .and_then(Json::as_array)
                                .and_then(|a| a.first())
                                .and_then(Json::as_str)
                                == Some("hook")
                    })
                })
            }),
            _ => false,
        };
        Ok(Wiring {
            settings_exists: settings.is_some(),
            hooks_present: hooks.as_ref().is_some_and(callback),
            statusline: matches!(
                settings.as_ref().and_then(|s| s.get("statusline")),
                Some(Json::Bool(true))
            ),
            mcp_present: mcp
                .as_ref()
                .and_then(|d| d.get("mcpServers"))
                .is_some_and(|s| s.get("ctxloom").is_some()),
        })
    }

    fn codec(&self) -> &dyn HookCodec {
        &self.codec
    }
}

/// What a `BTreeMap` of hooks names, for the tests of other crates.
pub type Hooks = BTreeMap<String, Vec<Hook>>;

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
