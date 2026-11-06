//! The ports an engine's native files and hook wire are reached through
//! (S9, the surface half of the engine port, plan 2.2).
//!
//! An engine (an adapter: `engine-claude`, `engine-mock`) knows its own
//! dialect: where its context, MCP registry, settings, hooks, commands and
//! skills live and how each is spelled. It says so as **claims** (what it
//! puts in which file), without touching a file. A store (`surface-files`)
//! delivers the claims under a directory and keeps the ownership record that
//! gives them back byte for byte: [`SurfaceStore`]. The application layer
//! decides which engine's claims are delivered, for which kinds, and when.
//!
//! The session-home half of the port (a disposable `CLAUDE_CONFIG_DIR` per
//! session) is the run slices' (S6): every claim here is at a directory's
//! root.

mod error;
mod json;

use std::collections::BTreeMap;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{SurfaceError, EN_US};
pub use json::Json;

/// A kind of surface an engine may carry, in the order a plan delivers them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    Context,
    Mcp,
    Settings,
    Hooks,
    Commands,
    Skills,
}

impl Kind {
    /// Every kind, in delivery order.
    pub const ALL: [Kind; 6] = [
        Kind::Context,
        Kind::Mcp,
        Kind::Settings,
        Kind::Hooks,
        Kind::Commands,
        Kind::Skills,
    ];

    /// The name `--surface` and the reports use.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Context => "context",
            Kind::Mcp => "mcp",
            Kind::Settings => "settings",
            Kind::Hooks => "hooks",
            Kind::Commands => "commands",
            Kind::Skills => "skills",
        }
    }

    /// The kind a name spells.
    pub fn parse(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// An MCP server to register.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

/// A lifecycle hook, in a unified event.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Hook {
    /// `command` (the default), `prompt`, ...
    pub kind: String,
    pub command: String,
    pub args: Vec<String>,
    pub prompt: String,
    pub timeout: i64,
    pub is_async: bool,
    /// The tool class the hook is narrowed to (`shell`, `file_edit`,
    /// `skill`); empty for every tool.
    pub tool: String,
    /// The engine's own matcher, which wins over a class.
    pub matcher: String,
}

impl Hook {
    /// Whether this is one of ctxloom's own callbacks: the bare `ctxloom`
    /// executable running `hook <verb>`.
    pub fn is_callback(&self) -> bool {
        self.command == "ctxloom" && self.args.first().is_some_and(|a| a == "hook")
    }

    /// The members an engine's native hook file shares: `type` (defaulting
    /// to `command`), then `command`, `args`, `prompt` and `timeout` where
    /// set. A ctxloom callback's args end `--engine <engine>`. The engine adds
    /// what only it spells (`async`, `tool`, `matcher`).
    pub fn json_members(&self, engine: &str) -> Vec<(&'static str, Json)> {
        let mut args = self.args.clone();
        if self.is_callback() && !args.iter().any(|a| a == "--engine") {
            args.push("--engine".to_string());
            args.push(engine.to_string());
        }
        let kind = if self.kind.is_empty() {
            "command"
        } else {
            &self.kind
        };
        let mut members = vec![("type", Json::text(kind))];
        if !self.command.is_empty() {
            members.push(("command", Json::text(self.command.as_str())));
        }
        if !args.is_empty() {
            members.push(("args", Json::strings(&args)));
        }
        if !self.prompt.is_empty() {
            members.push(("prompt", Json::text(self.prompt.as_str())));
        }
        if self.timeout > 0 {
            members.push(("timeout", Json::Number(self.timeout.to_string())));
        }
        members
    }
}

/// A command (a slash command's template).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Command {
    /// `<bundle>/<name>`.
    pub name: String,
    pub description: String,
    pub body: String,
}

/// One file of a skill package.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SkillFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

/// An Agent Skill package.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Skill {
    /// The name the package is written under (its `SKILL.md`'s `name`).
    pub name: String,
    pub description: String,
    pub files: Vec<SkillFile>,
}

/// What an engine is asked to deliver.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Inputs {
    /// The assembled context.
    pub context: String,
    /// A file inside the target the context goes to instead of the engine's
    /// own (`--surface context=file:<dest>`).
    pub context_file: Option<String>,
    pub mcp: Vec<McpServer>,
    /// Unified events (`session_start`, ...) to their hooks, in firing order.
    pub hooks: BTreeMap<String, Vec<Hook>>,
    pub deny_tools: Vec<String>,
    /// Whether ctxloom manages the HUD statusline.
    pub statusline: bool,
    pub commands: Vec<Command>,
    pub skills: Vec<Skill>,
}

/// What a claim on a JSON document does.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum JsonOp {
    /// The value at `path` is this (replacing what is there, creating the
    /// containers on the way).
    Set { path: Vec<String>, value: Json },
    /// As [`JsonOp::Set`] for a command entry (`{"type": "command", "command":
    /// "..."}`) that is ctxloom's only if absent or equal: one with another
    /// command is the user's and stays (said in the delivery's `left`); one
    /// whose command cannot be read is a refusal.
    SetCommand { path: Vec<String>, value: Json },
    /// The value is an element of the array at `path`; one equal to it already
    /// there is the user's, found rather than taken.
    Push { path: Vec<String>, value: Json },
    /// The hook is an entry of the group of `event` whose `matcher` is this
    /// (the group made if there is none), in the shape claude keeps hooks in:
    /// `hooks.<event>[{matcher, hooks: [...]}]`.
    HookEntry {
        event: String,
        matcher: Option<String>,
        value: Json,
    },
}

/// What an engine says it puts in a file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Claim {
    /// Text appended after what the file holds.
    Section { path: String, text: String },
    /// Entries in a JSON document.
    Json { path: String, ops: Vec<JsonOp> },
    /// A file ctxloom owns whole.
    Whole {
        path: String,
        bytes: Vec<u8>,
        executable: bool,
    },
}

impl Claim {
    /// The file the claim is on, below the target.
    pub fn path(&self) -> &str {
        match self {
            Claim::Section { path, .. } | Claim::Json { path, .. } | Claim::Whole { path, .. } => {
                path
            }
        }
    }
}

/// What an engine plans for a kind.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Planned {
    pub claims: Vec<Claim>,
    /// Each command or skill the engine would not carry, and why.
    pub skipped: Vec<String>,
}

/// What a delivery did.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Delivered {
    /// The files the claims are in, below the target.
    pub files: Vec<String>,
    /// Values left standing because they are not ctxloom's, each said.
    pub left: Vec<String>,
    /// Whether any file differs from what it held (with `dry`, whether any
    /// would).
    pub changed: bool,
}

/// What a release did.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Released {
    /// The files given back (their earlier bytes restored, or removed where
    /// ctxloom made them), below the target.
    pub files: Vec<String>,
    /// Whether any file differs from what it held (with `dry`, whether any
    /// would).
    pub changed: bool,
}

/// Where claims are delivered and given back.
pub trait SurfaceStore {
    /// Deliver `claims` as `engine`'s `kind` under `target`, replacing what
    /// that engine's kind claimed before. With `dry` nothing is written.
    fn deliver(
        &self,
        target: &str,
        engine: &str,
        kind: Kind,
        claims: &[Claim],
        dry: bool,
    ) -> Result<Delivered, SurfaceError>;

    /// Give back everything `engine`'s `kind` claimed under `target`. With
    /// `dry` nothing is written.
    fn release(
        &self,
        target: &str,
        engine: &str,
        kind: Kind,
        dry: bool,
    ) -> Result<Released, SurfaceError>;

    /// The kinds `engine` has claims of under `target`.
    fn claimed(&self, target: &str, engine: &str) -> Result<Vec<Kind>, SurfaceError>;

    /// The bytes of the file `rel` below `target`; none where it is not there.
    fn read(&self, target: &str, rel: &str) -> Result<Option<Vec<u8>>, SurfaceError>;

    /// The bytes of the file at the absolute `path` (a transcript the engine
    /// named); none where it is not there.
    fn read_absolute(&self, path: &str) -> Result<Option<Vec<u8>>, SurfaceError>;

    /// What is at `target`.
    fn inspect(&self, target: &str) -> Result<Inspected, SurfaceError>;
}

/// Foreign config files `util config-write` merges into, and the record of
/// what it applied to each (so the next write can take it back first).
pub trait ForeignFiles {
    /// The bytes of the file at the absolute `path`; none where it is not
    /// there.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, SurfaceError>;

    /// The file written whole, its directories made; an existing file keeps
    /// its mode.
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), SurfaceError>;

    /// The record of the last write to `path`, if there was one.
    fn record(&self, path: &str) -> Result<Option<String>, SurfaceError>;

    /// The record of the write just made to `path`; where it was kept.
    fn write_record(&self, path: &str, text: &str) -> Result<String, SurfaceError>;
}

/// What is at a target path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Inspected {
    Missing,
    Directory,
    /// A file, or anything that is not a directory.
    Other,
}

/// What an engine's settings show of ctxloom's wiring.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Wiring {
    /// The engine's settings file is there.
    pub settings_exists: bool,
    /// A ctxloom callback is registered in the hooks.
    pub hooks_present: bool,
    /// ctxloom's statusline is the engine's.
    pub statusline: bool,
    /// ctxloom's own MCP server is registered.
    pub mcp_present: bool,
}

/// The neutral shape of one hook event, as the engine's codec reads it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct HookEvent {
    /// The unified event.
    pub event: String,
    pub native_session: String,
    pub transcript: String,
    pub source: String,
    pub prompt: String,
    pub tool: String,
    /// The skill the tool call ran, if it ran one.
    pub skill: String,
    /// The file the tool call edited, if it edited one.
    pub path: String,
}

/// What a hook answers, in the neutral shape.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct HookResponse {
    /// Model-visible context.
    pub context: String,
    /// A message for the user.
    pub notice: String,
    pub block: bool,
    pub reason: String,
}

/// An engine's hook wire: what it writes to a hook's stdin and reads back
/// from its stdout.
pub trait HookCodec {
    /// Read one payload; `event` is the unified event the hook was
    /// registered under, used when the payload names none.
    fn decode(&self, event: &str, payload: &str) -> Result<HookEvent, SurfaceError>;

    /// The stdout (one JSON line) answering a hook registered under `event`.
    fn encode(&self, event: &str, response: &HookResponse) -> Result<String, SurfaceError>;

    /// The skills a transcript's main thread has invoked.
    fn invoked_skills(&self, transcript: &str) -> Vec<String>;
}

/// An engine's surface half.
pub trait EngineSurfaces {
    /// The engine's name.
    fn name(&self) -> &str;

    /// Whether the engine carries `kind` at all.
    fn carries(&self, kind: Kind) -> bool;

    /// Whether a launched session carries the engine's context itself (on
    /// its command line), so a copy at rest would double it.
    fn context_rides_launch(&self) -> bool {
        false
    }

    /// The unified hook events the engine has no native form for, each with
    /// the reason, in event order. The engine carries hooks generally and
    /// leaves these out of its hook file; a report names each one a profile
    /// configures (Go's `Definition.HookLosses`).
    fn hook_losses(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// The claims `kind` makes at a directory's root for `inputs`.
    fn plan(&self, kind: Kind, inputs: &Inputs) -> Result<Planned, SurfaceError>;

    /// What the engine's files, read through `store` under `target`, show of
    /// ctxloom's wiring.
    fn wiring(&self, store: &dyn SurfaceStore, target: &str) -> Result<Wiring, SurfaceError>;

    /// The engine's hook wire.
    fn codec(&self) -> &dyn HookCodec;
}

/// The engines a binary was composed with.
#[derive(Default)]
pub struct Registry {
    engines: Vec<Box<dyn EngineSurfaces>>,
}

impl Registry {
    /// A registry of these engines, the first the default.
    pub fn new(engines: Vec<Box<dyn EngineSurfaces>>) -> Registry {
        Registry { engines }
    }

    /// The engine called `name`.
    pub fn get(&self, name: &str) -> Option<&dyn EngineSurfaces> {
        self.engines
            .iter()
            .find(|e| e.name() == name)
            .map(|e| e.as_ref())
    }

    /// The engines' names, in registration order.
    pub fn names(&self) -> Vec<String> {
        self.engines.iter().map(|e| e.name().to_string()).collect()
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
