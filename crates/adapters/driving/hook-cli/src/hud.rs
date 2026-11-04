//! The statusline hook (`ctxloom hook hud`): the active agent's statusline
//! JSON on stdin, one formatted line out. Rendering only, and it records
//! nothing: the context-occupancy sample Go also wrote here is out of the
//! port (D5, 2026-11-02). Go: `hook_hud.go` (release/0.7).

use serde::Deserialize;

/// ANSI colors.
pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[36m";
pub const YELLOW: &str = "\x1b[33m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";

/// The common statusline JSON an agent pipes to the hook. Claude Code emits
/// exactly this shape; other agents map onto it. `model` is kept raw: an
/// object (`display_name`, `name` or `id`) or a bare string.
#[derive(Deserialize, Default, Debug)]
pub struct StatusPayload {
    #[serde(default)]
    pub model: serde_json::Value,
    #[serde(default)]
    pub context_window: ContextWindow,
    #[serde(default)]
    pub cost: Cost,
    #[serde(default)]
    pub worktree: Worktree,
}

/// `used_percentage` is optional because the engine sends JSON null, with
/// nothing used yet, until a session has accumulated usage; 0 would read as
/// "the context is empty" when it means "nobody has looked".
#[derive(Deserialize, Default, Debug)]
pub struct ContextWindow {
    pub used_percentage: Option<f64>,
}

#[derive(Deserialize, Default, Debug)]
pub struct Cost {
    #[serde(default)]
    pub total_cost_usd: f64,
}

#[derive(Deserialize, Default, Debug)]
pub struct Worktree {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub branch: String,
}

impl StatusPayload {
    /// The label of the model: `display_name`, else `name`, else `id` of an
    /// object, or a bare string; empty when there is none.
    pub fn model_name(&self) -> String {
        if let Some(object) = self.model.as_object() {
            for key in ["display_name", "name", "id"] {
                if let Some(label) = object.get(key).and_then(|v| v.as_str()) {
                    if !label.is_empty() {
                        return label.to_string();
                    }
                }
            }
        }
        self.model.as_str().unwrap_or_default().to_string()
    }
}

/// The color of a context bar at `pct` percent: red from 80, yellow from 60.
pub fn bar_color(pct: f64) -> &'static str {
    if pct >= 80.0 {
        RED
    } else if pct >= 60.0 {
        YELLOW
    } else {
        GREEN
    }
}

/// How many cells the context bar has.
const BAR_WIDTH: usize = 8;

/// An eight-cell bar filled to `pct` percent, clamped.
pub fn context_bar(pct: f64) -> String {
    // Truncated, as Go's int(): a bar fills a cell only when it is full.
    let filled = ((pct / 100.0 * BAR_WIDTH as f64) as usize).min(BAR_WIDTH);
    "\u{2588}".repeat(filled) + &"\u{2591}".repeat(BAR_WIDTH - filled)
}

/// The statusline for `payload`: the model, the context bar, the cost, the
/// session's `harp`, the `unread` mail, the worktree; `ctxloom` when there is
/// nothing to show, so a sparse payload never formats to zero bytes.
pub fn render(payload: &StatusPayload, harp: &str, unread: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    let model = payload.model_name();
    if !model.is_empty() {
        parts.push(format!("{CYAN}{model}{RESET}"));
    }
    // A null used_percentage (no usage yet) renders nothing, as a zero does.
    if let Some(pct) = payload.context_window.used_percentage.filter(|p| *p > 0.0) {
        parts.push(format!(
            "{}{} {pct:.0}%{RESET}",
            bar_color(pct),
            context_bar(pct)
        ));
    }
    if payload.cost.total_cost_usd > 0.0 {
        parts.push(format!("{DIM}${:.2}{RESET}", payload.cost.total_cost_usd));
    }
    if !harp.is_empty() {
        parts.push(format!("{DIM}\u{2301} {harp}{RESET}"));
    }
    if unread > 0 {
        parts.push(format!("{YELLOW}\u{2709} {unread}{RESET}"));
    }
    if !payload.worktree.name.is_empty() {
        let branch = match payload.worktree.branch.as_str() {
            "" => &payload.worktree.name,
            branch => branch,
        };
        parts.push(format!("{DIM}\u{2387} {branch}{RESET}"));
    }
    if parts.is_empty() {
        return "ctxloom".to_string();
    }
    parts.join(" \u{2502} ")
}

/// The hook: `input` is whatever was read from stdin. A payload that is not
/// the statusline JSON still gets the `ctxloom` sentinel, never an error: the
/// statusline must not break the engine's screen.
pub fn hud(input: &str, harp: &str, unread: u32) -> String {
    match serde_json::from_str::<StatusPayload>(input) {
        Ok(payload) => render(&payload, harp, unread),
        Err(_) => "ctxloom".to_string(),
    }
}

#[cfg(test)]
#[path = "hud.test.rs"]
mod tests;
