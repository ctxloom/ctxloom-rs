//! What each write answers, as data: the shapes are the JSON the commands
//! print (Go's `--format json`, field for field, in its order), and the text
//! forms are written from the same values. A command Go answers in text only
//! (its format debt) has data for the text and no JSON shape.

use serde::Serialize;

fn no_text(s: &str) -> bool {
    s.is_empty()
}

fn no_items<T>(items: &[T]) -> bool {
    items.is_empty()
}

/// A remove that did not apply: what would go, and the command that applies
/// it. Nothing was removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Preview {
    /// Always `preview`.
    pub status: String,
    /// Always false: nothing was applied.
    pub applied: bool,
    pub target: String,
    #[serde(skip_serializing_if = "no_items")]
    pub detail: Vec<String>,
    pub apply: String,
}

impl Preview {
    /// The preview of removing `target`, naming `apply` as the command that does.
    pub fn of(target: String, detail: Vec<String>, apply: String) -> Preview {
        Preview {
            status: "preview".to_string(),
            applied: false,
            target,
            detail,
            apply,
        }
    }
}

/// A remove: the preview, or the removal done.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum Removal<T> {
    Preview(Preview),
    Done(T),
}

/// `bundle create`.
#[derive(Clone, Debug, Serialize)]
pub struct BundleCreated {
    pub status: String,
    pub name: String,
    pub path: String,
}

/// `bundle edit`: `updated`, or `no_changes`.
#[derive(Clone, Debug, Serialize)]
pub struct BundleEdited {
    pub status: String,
    pub name: String,
    #[serde(skip_serializing_if = "no_items")]
    pub changes: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
}

/// `bundle remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct BundleRemoved {
    pub status: String,
    pub name: String,
    pub path: String,
}

/// `bundle export`.
#[derive(Clone, Debug, Serialize)]
pub struct BundleExported {
    pub status: String,
    pub name: String,
    pub source: String,
    pub dest: String,
}

/// `bundle import`.
#[derive(Clone, Debug, Serialize)]
pub struct BundleImported {
    pub status: String,
    pub source: String,
    pub dest: String,
    pub version: String,
    pub fragments: u64,
    pub commands: u64,
    pub mcp: u64,
}

/// `fragment create` and `command create`.
#[derive(Clone, Debug, Serialize)]
pub struct ItemCreated {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub path: String,
    /// `fragment` or `command`, for the text.
    #[serde(skip)]
    pub kind: String,
}

/// `fragment remove --yes` and `command remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct ItemRemoved {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub path: String,
    #[serde(skip)]
    pub kind: String,
}

/// `fragment edit` and `command edit`: `updated`, or `unchanged`.
#[derive(Clone, Debug, Serialize)]
pub struct ItemEdited {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub path: String,
    /// Whether the distilled form was refreshed.
    pub distilled: bool,
    #[serde(skip)]
    pub kind: String,
    /// The reference the user typed, for the advice that names it.
    #[serde(skip)]
    pub reference: String,
    /// `--no-distill` skipped a distillation the item would have had.
    #[serde(skip)]
    pub distill_skipped: bool,
}

/// `skill create`.
#[derive(Clone, Debug, Serialize)]
pub struct SkillCreated {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub dir: String,
}

/// `skill remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct SkillRemoved {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub dir: String,
}

/// `skill export`.
#[derive(Clone, Debug, Serialize)]
pub struct SkillExported {
    pub name: String,
    pub zip_path: String,
    pub bytes: u64,
}

/// `skill import`.
#[derive(Clone, Debug, Serialize)]
pub struct SkillImported {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub dir: String,
    pub file_count: u64,
}

/// `mcp server set`: `created`, or `updated`.
#[derive(Clone, Debug, Serialize)]
pub struct McpSet {
    pub status: String,
    pub bundle: String,
    pub name: String,
    pub path: String,
}

/// `mcp server edit`: what it said (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpEdited {
    Unchanged,
    Updated { name: String, bundle: String },
}

/// `profile create` (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileCreated {
    pub name: String,
    pub parents: Vec<String>,
    pub bundles: Vec<String>,
    pub path: String,
}

/// `profile modify` (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileModified {
    pub name: String,
    pub changes: Vec<String>,
}

/// `profile edit` (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileEdited {
    Unchanged,
    Updated { path: String },
}

/// `profile export` (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileExported {
    pub source: String,
    pub dest: String,
}

/// `profile import` (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileImported {
    pub source: String,
    pub dest: String,
}

/// `profile remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct ProfileRemoved {
    pub status: String,
    pub profile: String,
}

/// `agent default`: what it said (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultAgent {
    /// No argument: the agent a bare run binds, or none.
    Current(String),
    /// An argument: the agent now bound; whether it is defined yet.
    Set { name: String, defined: bool },
}

/// `agent remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct AgentRemoved {
    pub status: String,
    pub name: String,
}

/// An llm entry as a write leaves it.
#[derive(Clone, Debug, Serialize)]
pub struct LlmWritten {
    pub label: String,
    #[serde(rename = "type", skip_serializing_if = "no_text")]
    pub kind: String,
    #[serde(skip_serializing_if = "no_text")]
    pub model: String,
    pub permissions: serde_json::Value,
    /// The permissions block on one line, for the text.
    #[serde(skip)]
    pub permissions_line: String,
    #[serde(skip)]
    pub edited: bool,
}

/// `llm default` with no name.
#[derive(Clone, Debug, Serialize)]
pub struct LlmDefaultShown {
    pub default: String,
}

/// `llm default <name>`: `set`, or `unchanged`.
#[derive(Clone, Debug, Serialize)]
pub struct LlmDefaultSet {
    pub status: String,
    pub name: String,
}

/// What `llm default` answers.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum LlmDefault {
    Shown(LlmDefaultShown),
    Set(LlmDefaultSet),
}

/// `llm remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct LlmRemoved {
    pub status: String,
    pub label: String,
}

/// `companion add`.
#[derive(Clone, Debug, Serialize)]
pub struct CompanionAdded {
    pub name: String,
    /// The program, then its arguments, as registered.
    pub command: Vec<String>,
    /// Whether the registration is new or changed a command; false when it
    /// was already registered exactly so.
    pub added: bool,
}

/// `companion remove --yes`.
#[derive(Clone, Debug, Serialize)]
pub struct CompanionRemoved {
    pub name: String,
    pub removed: bool,
}

/// `config create` (text only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigCreated {
    pub path: String,
}
