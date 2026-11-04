//! The owner's voice (D24): text ctxloom delivers on its own behalf, copied
//! byte for byte from Go ctxloom's `cmd/ctxloom/loadout.yaml` at release/0.7
//! f36e8dfb2 (`run.fragments.isolation-axes`, its `content` and its
//! `distilled`). Nothing here is edited; any change is the owner's.
//!
//! The text names `agent_run`, an MCP tool the D8 cut list has not ruled on.
//! It is delivered because a project whose profiles carry no content of their
//! own would otherwise assemble to nothing, and Go refuses to materialize
//! nothing; if the cut list takes the tool, the owner rewrites this file.
//! (`discover` and `check-triggers` name `search_library` and
//! `evaluate_triggers`, which D24 holds back; they are not here.)

/// ctxloom's always-on isolation guidance, in full.
pub const ISOLATION_AXES: &str = include_str!("../resources/isolation-axes.md");

/// The same, distilled (what the assembly takes while `use_distilled` is on).
pub const ISOLATION_AXES_DISTILLED: &str = include_str!("../resources/isolation-axes.distilled.md");

/// The separator between the fragments of an assembled context.
const SEPARATOR: &str = "\n\n---\n\n";

/// `context` with ctxloom's own fragment after it, always last (the
/// self-companion's section).
pub fn with_own_fragment(context: &str, use_distilled: bool) -> String {
    let own = if use_distilled {
        ISOLATION_AXES_DISTILLED
    } else {
        ISOLATION_AXES
    };
    if context.trim().is_empty() {
        own.to_string()
    } else {
        format!("{context}{SEPARATOR}{own}")
    }
}

#[cfg(test)]
#[path = "owner_voice.test.rs"]
mod tests;
