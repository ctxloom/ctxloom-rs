//! A bundle as the library holds it: the envelope's metadata and the items,
//! whatever form they were read from (a tree of files, or an inline document).
//!
//! Everything is a value: no path, no clock, no I/O. A bundle's place on disk
//! is the adapter's and the application's (the catalog's entry).

use std::collections::BTreeMap;

use angzarr_yaml::Value;

use crate::profile::Profile;

/// What a fragment and a command hold alike.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct ItemBody {
    pub tags: Vec<String>,
    /// Human-facing, never sent to a model.
    pub notes: String,
    pub installation: String,
    /// The authored body, front matter excluded.
    pub content: String,
    /// Distillation change-detection bookkeeping, carried verbatim.
    pub content_hash: String,
    /// The distilled rewrite; empty when there is none.
    pub distilled: String,
    pub distilled_by: String,
    pub no_distill: bool,
}

/// A fragment: context an assistant just has. `premise` (the file's
/// `description:`) is the situation it applies to; empty means always loaded.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Fragment {
    pub body: ItemBody,
    pub premise: String,
}

/// A command: a user-invoked slash template.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Command {
    pub body: ItemBody,
    pub description: String,
    /// Per-engine export blocks as authored, by engine name: carried, never
    /// read here (the named engine decodes its own).
    pub exports: BTreeMap<String, Value>,
}

/// An MCP server a bundle ships: a command to spawn, or a URL to reach.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct McpServer {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    /// `session-endpoint` when the running session serves it.
    pub served_by: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub installation: String,
}

/// One file of a skill package.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SkillFile {
    /// Relative to the package directory, slash-separated.
    pub path: String,
    pub bytes: Vec<u8>,
    /// Whether the package's sidecar declares the file executable (a mode bit
    /// is not portable, so the declaration is the whole of it).
    pub executable: bool,
}

/// An Agent Skill package.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Skill {
    /// The document form's `path:`; empty means `skills/<name>`.
    pub path: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub exports: BTreeMap<String, Value>,
    /// The package's files, by path (tree form only), sorted.
    pub files: Vec<SkillFile>,
}

/// A lifecycle hook a bundle ships.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Hook {
    pub event: String,
    /// The hook's name within its event (the tree's file name).
    pub name: String,
    /// Its place among its event's hooks; a hook declaring none sorts last.
    pub order: Option<i64>,
    pub tool: String,
    pub kind: String,
    pub command: String,
    pub args: Vec<String>,
    pub prompt: String,
    pub timeout: i64,
    pub is_async: bool,
    pub pre_tool_fallback: bool,
    pub tags: Vec<String>,
}

/// A bundle.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Bundle {
    /// The name the bundle declares for itself (`name:`); usually empty.
    pub name: String,
    pub version: String,
    pub tags: Vec<String>,
    pub author: String,
    pub description: String,
    pub notes: String,
    pub installation: String,
    /// The commands its content assumes exist (`requires.commands`).
    pub requires_commands: Vec<String>,
    pub fragments: BTreeMap<String, Fragment>,
    pub commands: BTreeMap<String, Command>,
    pub mcp: BTreeMap<String, McpServer>,
    pub skills: BTreeMap<String, Skill>,
    pub profiles: BTreeMap<String, Profile>,
    /// In event order (`pre_tool`, `post_tool`, `session_start`, `session_end`,
    /// `pre_shell`, `post_file_edit`, `turn_end`, `turn_start`), each event's
    /// hooks in their declared order.
    pub hooks: Vec<Hook>,
}

/// The lifecycle events a hook may be on, in the order they enumerate.
pub const HOOK_EVENTS: [&str; 8] = [
    "pre_tool",
    "post_tool",
    "session_start",
    "session_end",
    "pre_shell",
    "post_file_edit",
    "turn_end",
    "turn_start",
];

impl Bundle {
    /// The hooks' identities, `<event>/<index>` (the index among its event's
    /// hooks), in enumeration order: what `bundle show` lists.
    pub fn hook_ids(&self) -> Vec<String> {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        self.hooks
            .iter()
            .map(|hook| {
                let next = counts.entry(hook.event.as_str()).or_insert(0);
                let id = format!("{}/{next}", hook.event);
                *next += 1;
                id
            })
            .collect()
    }
}
