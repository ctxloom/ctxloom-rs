//! What each write is asked, as data: the flags a command was given, with
//! `None` for a flag not named (it keeps what is stored) and `Some` empty for
//! a flag named empty (it clears).

/// `bundle edit`.
#[derive(Clone, Default, Debug)]
pub struct BundleEditRequest {
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub add_fragments: Vec<String>,
    pub remove_fragments: Vec<String>,
    pub add_prompts: Vec<String>,
    pub remove_prompts: Vec<String>,
    pub add_mcp: Vec<String>,
    pub remove_mcp: Vec<String>,
}

/// `profile create`.
#[derive(Clone, Default, Debug)]
pub struct ProfileCreateRequest {
    pub name: String,
    pub description: String,
    pub parents: Vec<String>,
    pub bundles: Vec<String>,
    pub llm: String,
    /// The local bundle the profile is written into; the project's own when
    /// empty.
    pub target: String,
}

/// `agent create` and `agent edit`.
#[derive(Clone, Default, Debug)]
pub struct AgentRequest {
    pub llm: Option<String>,
    pub profiles: Option<Vec<String>>,
    pub runtime: Option<String>,
    /// `kind=approach` pairs; an empty list clears them.
    pub surfaces: Option<Vec<(String, String)>>,
    /// `kind=root` pairs; an empty list clears them.
    pub roots: Option<Vec<(String, String)>>,
    pub permissions: Option<String>,
    pub engine_home: Option<String>,
    pub env_host: Option<bool>,
    pub env: Option<Vec<String>>,
}

/// `llm create` and `llm edit`.
#[derive(Clone, Default, Debug)]
pub struct LlmRequest {
    pub kind: Option<String>,
    pub model: Option<String>,
    pub permissions: Option<String>,
    /// `dotted.key=value` pairs, the value read as YAML; an empty value
    /// removes the key.
    pub set: Vec<String>,
}
