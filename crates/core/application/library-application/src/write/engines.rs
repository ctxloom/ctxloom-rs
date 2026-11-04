//! What a write needs to know of an engine (Go's engine registry, the part a
//! binding and a label are checked against): its permission modes. Which
//! surfaces and roots an engine offers is each engine's own answer and arrives
//! with the engines (S9); until then a surface preference is checked for its
//! shape and, for claude-code, against its approaches.

use crate::agents::ENGINES;

/// The roots a surface can be placed in.
pub(crate) const ROOTS: [&str; 3] = ["session-home", "project-root", "work-dir"];

/// The kinds of surface a binding has a preference for.
pub(crate) const SURFACE_KINDS: [&str; 6] =
    ["context", "mcp", "settings", "hooks", "commands", "skills"];

/// Whether `name` is an engine this build knows.
pub(crate) fn known(name: &str) -> bool {
    ENGINES.contains(&name)
}

/// The engine's words for its own kind of permission document.
fn family(engine: &str) -> Option<&'static str> {
    match engine {
        "claude-code" => Some("claude"),
        "opencode" => Some("opencode"),
        "mock" | "mock-launch" | "mock-lossy" | "mock-noskills" => Some("mock"),
        _ => None,
    }
}

/// The permission modes of `engine`, in its order.
pub(crate) fn postures(engine: &str) -> &'static [&'static str] {
    match family(engine) {
        Some("claude") => &["default", "acceptEdits", "plan", "bypassPermissions"],
        Some("opencode") => &["build", "plan", "auto"],
        Some("mock") => &["default", "plan", "bypass"],
        _ => &[],
    }
}

/// Claude's retired spellings of a mode, and what each is now.
fn replaced(mode: &str) -> Option<&'static str> {
    match mode.trim().to_lowercase().as_str() {
        "dontask" => Some("approver: none"),
        "auto" => Some("approver: reviewer"),
        "bypass" => Some("mode: bypassPermissions"),
        _ => None,
    }
}

/// `mode` as `engine` takes it (claude reads a mode without regard to case,
/// and says what a retired spelling is now), or why it does not.
pub(crate) fn mode(engine: &str, mode: &str) -> Result<String, String> {
    let Some(family) = family(engine) else {
        return Err(format!(
            "{engine} takes no permission mode (it is not an engine this build knows)"
        ));
    };
    let known = postures(engine);
    if family == "claude" {
        if let Some(found) = known.iter().find(|p| p.eq_ignore_ascii_case(mode.trim())) {
            return Ok((*found).to_string());
        }
        if let Some(now) = replaced(mode) {
            return Err(format!(
                "claude permissions: mode \"{mode}\" is declared as {now}"
            ));
        }
        return Err(format!(
            "claude permissions: mode \"{mode}\" is not a claude mode (known: {})",
            known.join("|")
        ));
    }
    if known.contains(&mode) {
        return Ok(mode.to_string());
    }
    Err(format!(
        "{family} permissions: mode {mode} (known: {})",
        known.join("|")
    ))
}

/// The approach `engine` offers for the surface `kind`: none known for an
/// engine whose declaration this build does not carry yet.
pub(crate) fn approaches(engine: &str, kind: &str) -> Option<&'static [&'static str]> {
    if engine != "claude-code" {
        return None;
    }
    match kind {
        "context" => Some(&["system-prompt"]),
        "mcp" => Some(&["mcp-config"]),
        "settings" => Some(&["settings"]),
        "hooks" => Some(&["settings-hooks"]),
        "commands" => Some(&["commands-dir"]),
        "skills" => Some(&["skills-dir"]),
        _ => None,
    }
}

#[cfg(test)]
#[path = "engines.test.rs"]
mod tests;
