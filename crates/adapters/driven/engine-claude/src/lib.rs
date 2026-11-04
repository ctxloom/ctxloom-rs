//! Claude Code's surface half (S9; Go's `internal/engines/claude`
//! `definition.go`, `surfaces.go`, `commandfiles.go`, `hookcodec.go`).
//!
//! At a directory's root claude reads `CLAUDE.md` (context), `.mcp.json`
//! (MCP servers), `.claude/settings.json` (settings and hooks),
//! `.claude/commands/` and `.claude/skills/`. This crate says what ctxloom
//! puts in each as claims; it touches no file.
//!
//! plan supersedes Go (8.4): ctxloom registers no MCP server of its own at
//! rest (its entry is the companion's dynamic declaration, injected into a
//! session), so `.mcp.json` carries only the servers the profiles' bundles
//! ship, and no file is conjured for none.

mod codec;
mod commands;
mod hooks;
mod skills;

use surface_ports::{
    Claim, EngineSurfaces, HookCodec, Inputs, Json, JsonOp, Kind, McpServer, Planned, SurfaceError,
    SurfaceStore, Wiring,
};

pub use codec::Codec;

/// The engine's name.
pub const NAME: &str = "claude-code";
/// The context file claude reads.
pub const CONTEXT_FILE: &str = "CLAUDE.md";
/// The MCP registry claude reads.
pub const MCP_FILE: &str = ".mcp.json";
/// The settings file claude reads, hooks included.
pub const SETTINGS_FILE: &str = ".claude/settings.json";
/// The statusline command ctxloom installs: the bare executable, which
/// resolves on `PATH` wherever the file is read.
pub const STATUSLINE: &str = "ctxloom hook hud";

/// Claude Code.
#[derive(Default)]
pub struct Claude {
    codec: Codec,
}

impl Claude {
    pub fn new() -> Claude {
        Claude::default()
    }
}

impl EngineSurfaces for Claude {
    fn name(&self) -> &str {
        NAME
    }

    fn carries(&self, _kind: Kind) -> bool {
        true
    }

    fn plan(&self, kind: Kind, inputs: &Inputs) -> Result<Planned, SurfaceError> {
        let mut planned = Planned::default();
        match kind {
            Kind::Context => planned.claims.push(Claim::Section {
                path: inputs
                    .context_file
                    .clone()
                    .unwrap_or_else(|| CONTEXT_FILE.to_string()),
                text: inputs.context.clone(),
            }),
            Kind::Mcp => {
                let ops: Vec<JsonOp> = inputs
                    .mcp
                    .iter()
                    .map(|server| JsonOp::Set {
                        path: vec!["mcpServers".to_string(), server.name.clone()],
                        value: mcp_entry(server),
                    })
                    .collect();
                if !ops.is_empty() {
                    planned.claims.push(Claim::Json {
                        path: MCP_FILE.to_string(),
                        ops,
                    });
                }
            }
            Kind::Settings => {
                let mut ops = Vec::new();
                if inputs.statusline {
                    ops.push(JsonOp::SetCommand {
                        path: vec!["statusLine".to_string()],
                        value: Json::object([
                            ("type", Json::text("command")),
                            ("command", Json::text(STATUSLINE)),
                        ]),
                    });
                }
                let mut seen: Vec<&str> = Vec::new();
                for tool in inputs.deny_tools.iter().filter(|t| !t.is_empty()) {
                    if !seen.contains(&tool.as_str()) {
                        seen.push(tool);
                        ops.push(JsonOp::Push {
                            path: vec!["permissions".to_string(), "deny".to_string()],
                            value: Json::text(tool.as_str()),
                        });
                    }
                }
                if !ops.is_empty() {
                    planned.claims.push(Claim::Json {
                        path: SETTINGS_FILE.to_string(),
                        ops,
                    });
                }
            }
            Kind::Hooks => {
                let ops = hooks::ops(&inputs.hooks)?;
                if !ops.is_empty() {
                    planned.claims.push(Claim::Json {
                        path: SETTINGS_FILE.to_string(),
                        ops,
                    });
                }
            }
            Kind::Commands => commands::plan(&inputs.commands, &mut planned),
            Kind::Skills => skills::plan(&inputs.skills, &mut planned),
        }
        Ok(planned)
    }

    fn wiring(&self, store: &dyn SurfaceStore, target: &str) -> Result<Wiring, SurfaceError> {
        let settings = store.read(target, SETTINGS_FILE)?;
        let doc = settings
            .as_deref()
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(|t| Json::parse(t).ok());
        let mcp = store
            .read(target, MCP_FILE)?
            .as_deref()
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(|t| Json::parse(t).ok());
        Ok(Wiring {
            settings_exists: settings.is_some(),
            hooks_present: doc.as_ref().is_some_and(hooks::has_callback),
            statusline: doc
                .as_ref()
                .and_then(|d| d.get("statusLine"))
                .and_then(|s| s.get("command"))
                .and_then(Json::as_str)
                == Some(STATUSLINE),
            mcp_present: mcp
                .as_ref()
                .and_then(|d| d.get("mcpServers"))
                .is_some_and(|servers| servers.get("ctxloom").is_some()),
        })
    }

    fn codec(&self) -> &dyn HookCodec {
        &self.codec
    }
}

/// One server as `.mcp.json` spells it: a command to spawn, or a URL to
/// reach.
fn mcp_entry(server: &McpServer) -> Json {
    let map = |m: &std::collections::BTreeMap<String, String>| {
        Json::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), Json::text(v.as_str())))
                .collect(),
        )
    };
    if !server.url.is_empty() {
        let mut members = vec![
            ("type", Json::text("http")),
            ("url", Json::text(server.url.as_str())),
        ];
        if !server.headers.is_empty() {
            members.push(("headers", map(&server.headers)));
        }
        return Json::object(members);
    }
    let mut members = vec![("command", Json::text(server.command.as_str()))];
    if !server.args.is_empty() {
        members.push(("args", Json::strings(&server.args)));
    }
    if !server.env.is_empty() {
        members.push(("env", map(&server.env)));
    }
    Json::object(members)
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
