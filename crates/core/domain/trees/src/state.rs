//! The tree, folded from its events.

/// The trees domain's name: every tree stream's `Cover.domain`.
pub const TREE_DOMAIN: &str = "trees";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeState {
    pub owner_session: String,
    pub ephemeral: bool,
    pub concurrency: u32,
    pub depth: u32,
    /// Whoever holds the tree after `--session` (Q2); the owner until then.
    pub occupant: String,
    pub founded: bool,
}
