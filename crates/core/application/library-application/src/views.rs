//! What each command answers, as data: the shapes are the JSON the commands
//! print (Go's `--format json`, field for field, in its order), and the text
//! forms are written from the same values.

use std::collections::BTreeMap;

use serde::Serialize;

fn no_text(s: &str) -> bool {
    s.is_empty()
}

fn no_items<T>(items: &[T]) -> bool {
    items.is_empty()
}

fn no_entries<K, V>(map: &BTreeMap<K, V>) -> bool {
    map.is_empty()
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// One row of `bundle list`.
#[derive(Clone, Debug, Serialize)]
pub struct BundleRow {
    pub name: String,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
    #[serde(skip_serializing_if = "no_text")]
    pub version: String,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    pub fragment_count: u64,
    pub command_count: u64,
    pub mcp_count: u64,
    pub profile_count: u64,
    pub held: bool,
}

/// What `bundle list` answers.
#[derive(Clone, Debug)]
pub struct BundleList {
    pub rows: Vec<BundleRow>,
    /// The project has no bundles directory at all.
    pub no_root: bool,
    /// The hint for an empty listing.
    pub hint: String,
}

/// A fragment as `bundle show` shows it.
#[derive(Clone, Debug, Serialize)]
pub struct ShownFragment {
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    pub preview: String,
    pub distilled: bool,
    pub no_distill: bool,
    #[serde(skip_serializing_if = "no_text")]
    pub premise: String,
}

/// A command as `bundle show` shows it.
#[derive(Clone, Debug, Serialize)]
pub struct ShownCommand {
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    pub preview: String,
    pub distilled: bool,
    pub no_distill: bool,
}

/// An MCP server as `bundle show` shows it.
#[derive(Clone, Debug, Serialize)]
pub struct ShownMcp {
    pub command: String,
    #[serde(skip_serializing_if = "no_items")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "no_entries")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "no_text")]
    pub notes: String,
    #[serde(skip_serializing_if = "no_text")]
    pub installation: String,
}

/// A skill as `bundle show` shows it.
#[derive(Clone, Debug, Serialize)]
pub struct ShownSkill {
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub notes: String,
}

/// What `bundle show` answers.
#[derive(Clone, Debug, Serialize)]
pub struct BundleShow {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
    #[serde(skip_serializing_if = "no_text")]
    pub version: String,
    #[serde(skip_serializing_if = "no_text")]
    pub author: String,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub notes: String,
    #[serde(skip_serializing_if = "no_text")]
    pub installation: String,
    #[serde(skip_serializing_if = "no_entries")]
    pub fragments: BTreeMap<String, ShownFragment>,
    #[serde(skip_serializing_if = "no_entries")]
    pub commands: BTreeMap<String, ShownCommand>,
    #[serde(skip_serializing_if = "no_entries")]
    pub mcp: BTreeMap<String, ShownMcp>,
    #[serde(skip_serializing_if = "no_entries")]
    pub skills: BTreeMap<String, ShownSkill>,
    #[serde(skip_serializing_if = "no_items")]
    pub profiles: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub hooks: Vec<String>,
}

/// What `bundle view` answers.
#[derive(Clone, Debug, Serialize)]
pub struct BundleView {
    pub bundle: String,
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
    #[serde(skip_serializing_if = "is_false")]
    pub distilled: bool,
    pub content: String,
    /// Whether the content is one item's (its blank runs collapse in text).
    #[serde(skip)]
    pub item: bool,
}

/// One row of `fragment list` and `command list`.
#[derive(Clone, Debug, Serialize)]
pub struct ItemRow {
    pub name: String,
    pub tags: Vec<String>,
    pub bundle: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub remote: String,
    pub bundle_label: String,
}

/// What `fragment list` and `command list` answer.
#[derive(Clone, Debug)]
pub struct ItemList {
    pub rows: Vec<ItemRow>,
    /// The kind, `fragment` or `command`.
    pub kind: String,
    /// The `--bundle` asked for, empty for none.
    pub bundle_filter: String,
    /// How many there are in all bundles.
    pub total: usize,
    pub hint: String,
}

/// What `fragment show` and `command show` answer.
#[derive(Clone, Debug)]
pub struct ItemShow {
    pub name: String,
    pub reference: String,
    pub content: String,
    pub distilled: bool,
}

/// One entry of `fragment premises`.
#[derive(Clone, Debug, Serialize)]
pub struct PremiseRow {
    pub name: String,
    pub premise: String,
}

/// What `fragment premises` answers.
#[derive(Clone, Debug, Serialize)]
pub struct Premises {
    pub instruction: String,
    pub fragments: Vec<PremiseRow>,
}

/// One row of `skill list`.
#[derive(Clone, Debug, Serialize)]
pub struct SkillRow {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    pub source: String,
    pub file_count: u64,
}

/// What `skill list` answers.
#[derive(Clone, Debug)]
pub struct SkillList {
    pub rows: Vec<SkillRow>,
    pub bundle_filter: String,
    pub total: usize,
}

/// One file of a skill package.
#[derive(Clone, Debug, Serialize)]
pub struct SkillFileRow {
    pub path: String,
    pub sha256: String,
    pub mode: String,
}

/// What `skill show` answers.
#[derive(Clone, Debug, Serialize)]
pub struct SkillShow {
    pub name: String,
    pub bundle: String,
    pub description: String,
    #[serde(skip_serializing_if = "no_text")]
    pub license: String,
    #[serde(skip_serializing_if = "no_text")]
    pub compatibility: String,
    #[serde(skip_serializing_if = "no_items")]
    pub allowed_tools: Vec<String>,
    pub body: String,
    pub files: Vec<SkillFileRow>,
}

/// One row of `profile list`.
#[derive(Clone, Debug, Serialize)]
pub struct ProfileRow {
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    #[serde(skip_serializing_if = "no_items")]
    pub parents: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub bundles: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub default: bool,
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
    pub is_remote: bool,
    #[serde(skip_serializing_if = "no_text")]
    pub bundle: String,
}

/// What `profile list` answers.
#[derive(Clone, Debug)]
pub struct ProfileList {
    pub rows: Vec<ProfileRow>,
    pub hint: String,
}

/// What `profile show` answers.
#[derive(Clone, Debug, Serialize)]
pub struct ProfileShow {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    #[serde(skip_serializing_if = "no_text")]
    pub llm: String,
    #[serde(skip_serializing_if = "no_items")]
    pub parents: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub bundles: Vec<String>,
    #[serde(skip_serializing_if = "no_entries")]
    pub variables: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "no_items")]
    pub exclude_fragments: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub exclude_mcp: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub path: String,
    #[serde(skip_serializing_if = "no_text")]
    pub bundle: String,
    pub default: bool,
}

/// An agent as its binding declares it.
#[derive(Clone, Debug, Serialize)]
pub struct AgentRow {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub llm: String,
    #[serde(skip_serializing_if = "no_items")]
    pub profiles: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub runtime: String,
    pub permissions: serde_json::Value,
    #[serde(skip_serializing_if = "no_text")]
    pub driving: String,
    #[serde(rename = "engine_home", skip_serializing_if = "no_text")]
    pub home_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_host: Option<bool>,
    #[serde(skip_serializing_if = "no_items")]
    pub env: Vec<String>,
    /// The permissions block on one line, for the listing.
    #[serde(skip)]
    pub permissions_line: String,
}

/// The engine and context an agent resolves to.
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedAgent {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub llm: String,
    pub profiles: Vec<String>,
    pub label: String,
    pub backend: String,
    #[serde(skip_serializing_if = "no_text")]
    pub model: String,
    #[serde(skip_serializing_if = "no_text")]
    pub context: String,
    #[serde(skip_serializing_if = "no_items")]
    pub fragments: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub runtime: String,
    pub permissions: serde_json::Value,
    #[serde(rename = "effectivePermissions", skip_serializing_if = "no_text")]
    pub effective_permissions: String,
    #[serde(skip_serializing_if = "no_text")]
    pub approver: String,
    #[serde(skip_serializing_if = "no_text")]
    pub driving: String,
    #[serde(rename = "engine_home", skip_serializing_if = "no_text")]
    pub home_mode: String,
}

/// A command an agent needs on its PATH and who asks for it.
#[derive(Clone, Debug, Serialize)]
pub struct Requirement {
    pub command: String,
    pub sources: Vec<RequirementSource>,
}

/// One source of a requirement.
#[derive(Clone, Debug, Serialize)]
pub struct RequirementSource {
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

/// What `agent show` answers.
#[derive(Clone, Debug, Serialize)]
pub struct AgentShow {
    pub definition: AgentRow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<ResolvedAgent>,
    #[serde(skip_serializing_if = "no_text")]
    pub resolution_error: String,
    #[serde(skip_serializing_if = "no_items")]
    pub requirements: Vec<Requirement>,
    #[serde(skip_serializing_if = "no_text")]
    pub requirements_error: String,
}

/// One row of `llm list`.
#[derive(Clone, Debug, Serialize)]
pub struct LlmRow {
    pub label: String,
    pub default: bool,
    pub authored: bool,
    #[serde(skip_serializing_if = "no_items")]
    pub runtimes: Vec<String>,
}

/// One row of `companion list`.
#[derive(Clone, Debug, Serialize)]
pub struct CompanionRow {
    pub name: String,
    /// The program, then its arguments.
    pub command: Vec<String>,
    /// Whether the program is there; it is not run to find out.
    pub exists: bool,
}

/// A registered MCP server.
#[derive(Clone, Debug, Serialize)]
pub struct McpRow {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub command: String,
    #[serde(skip_serializing_if = "no_items")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "no_entries")]
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "no_text")]
    pub url: String,
    #[serde(skip_serializing_if = "no_text")]
    pub served_by: String,
    pub source: String,
    #[serde(skip_serializing_if = "no_text")]
    pub notes: String,
    #[serde(skip_serializing_if = "no_text")]
    pub installation: String,
}

/// What `mcp server list` answers.
#[derive(Clone, Debug, Serialize)]
pub struct McpList {
    pub servers: Vec<McpListRow>,
}

/// One row of `mcp server list` (no environment, notes or installation).
#[derive(Clone, Debug, Serialize)]
pub struct McpListRow {
    pub name: String,
    #[serde(skip_serializing_if = "no_text")]
    pub command: String,
    #[serde(skip_serializing_if = "no_items")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub url: String,
    #[serde(skip_serializing_if = "no_text")]
    pub served_by: String,
    pub source: String,
}

/// What `mcp server show` answers.
#[derive(Clone, Debug, Serialize)]
pub struct McpShow {
    pub name: String,
    pub found: bool,
    pub entries: Vec<McpRow>,
}

/// One result of `search`.
#[derive(Clone, Debug, Serialize)]
pub struct SearchHit {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub source: String,
    #[serde(skip_serializing_if = "no_text")]
    pub r#match: String,
}

/// One bundle a remote publishes, as a search found it.
#[derive(Clone, Debug, Serialize)]
pub struct RemoteFound {
    pub remote: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "no_items")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "no_text")]
    pub description: String,
    #[serde(skip_serializing_if = "no_text")]
    pub author: String,
    pub pull_ref: String,
}

/// What `search` answers.
#[derive(Clone, Debug, Serialize)]
pub struct SearchResults {
    pub query: String,
    pub local: Vec<SearchHit>,
    pub remote: Vec<RemoteFound>,
    pub count: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pub hidden_local: u64,
}

/// A finding, on the wire.
#[derive(Clone, Debug, Serialize)]
pub struct FindingRow {
    pub kind: String,
    pub text: String,
    #[serde(skip_serializing_if = "no_text")]
    pub remedy: String,
    pub fatal: bool,
}

/// What `run --dry-run` answers: the launch a run would carry. The pieces
/// that print the engines' surfaces and isolation (`resolved`, `environment`,
/// `delivery`, `engine_home`) arrive with those slices.
#[derive(Clone, Debug, Serialize)]
pub struct DryRun {
    #[serde(skip_serializing_if = "no_text")]
    pub agent: String,
    #[serde(skip_serializing_if = "no_text")]
    pub workspace: String,
    #[serde(skip_serializing_if = "no_text")]
    pub runtime: String,
    pub llm: String,
    pub backend: String,
    pub profiles: Vec<String>,
    pub fragments: Vec<String>,
    pub context: String,
    pub tokens: u64,
    #[serde(skip_serializing_if = "no_text")]
    pub prompt: String,
    pub findings: Vec<FindingRow>,
    /// The workspace and runtime a launch settles on (shown beside an agent).
    #[serde(skip)]
    pub axes: (String, String),
}

/// What `context assemble` answers (Go's `assemble_context` result).
#[derive(Clone, Debug, Serialize)]
pub struct Assembled {
    pub profiles: Vec<String>,
    pub context: String,
    pub fragments_loaded: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub missing_fragments: Vec<String>,
    #[serde(skip_serializing_if = "no_items")]
    pub missing_tags: Vec<String>,
}

#[cfg(test)]
#[path = "views.test.rs"]
mod tests;
