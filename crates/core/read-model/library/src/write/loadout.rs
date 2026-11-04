//! ctxloom's own loadout (Go's `cmd/ctxloom/loadout.yaml`): what ctxloom
//! delivers into an engine on its own behalf, printed by `ctxloom loadout
//! --format yaml`. ctxloom is its own companion.
//!
//! The document's prose is the owner's (D24) and names MCP tools whose cut
//! list is open (D8): until the owner rules, this carries its structure and
//! none of the text (the description, the server's notes, the `isolation-axes`
//! fragment).

/// The loadout document.
pub const DOCUMENT: &str = "run:\n  version: 1.0.0\n  tags:\n    - core\n    - mcp\n    - isolation\n  author: ctxloom\n  mcp:\n    ctxloom:\n      served_by: session-endpoint\n";

#[cfg(test)]
#[path = "loadout.test.rs"]
mod tests;
