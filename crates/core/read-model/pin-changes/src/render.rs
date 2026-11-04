//! The disclosure as text (Go's `WritePinChanges`, `WriteNewPins`).

use std::collections::{BTreeMap, BTreeSet};

use crate::{Change, ExecDelta, ExecSpec, PinChange};

/// Introduces the first pins a pull made.
pub const NEW_PINS_HEADER: &str = "New pins, with everything each one brings in:";

/// Each pin change: a header naming the move, one line per item (`+` added,
/// `-` removed, `~` modified) with what hooks and MCP servers run, and one
/// line per file with the unified diff of each script.
pub fn pin_changes_text(changes: &[PinChange]) -> String {
    let mut out = String::new();
    for change in changes {
        let (from, from_sha) = if change.from_sha.is_empty() {
            ("first pin", "new")
        } else {
            (version_label(&change.from_version), short(&change.from_sha))
        };
        out.push_str(&format!(
            "{}  {from} -> {}  ({from_sha} -> {})\n",
            change.identity,
            version_label(&change.to_version),
            short(&change.to_sha)
        ));
        for item in &change.items {
            out.push_str(&format!(
                "  {} {} {}\n",
                mark(item.change),
                item.kind,
                item.name
            ));
            if let Some(delta) = &item.exec {
                exec_text(&mut out, delta);
            }
        }
        for file in &change.files {
            out.push_str(&format!("  {} file {}\n", mark(file.change), file.path));
            for line in file.diff.trim_end_matches('\n').lines() {
                out.push_str(&format!("      {line}\n"));
            }
        }
    }
    out
}

fn version_label(version: &str) -> &str {
    if version.is_empty() {
        "unversioned"
    } else {
        version
    }
}

/// A commit shortened to seven characters.
fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

fn mark(change: Change) -> char {
    match change {
        Change::Added => '+',
        Change::Removed => '-',
        Change::Modified => '~',
    }
}

/// Each field either side sets: once when the two agree (or only one side
/// exists), as before -> after when they differ. Env and headers one line per
/// name.
fn exec_text(out: &mut String, delta: &ExecDelta) {
    scalar_text(out, "command", delta, |spec| spec.command.clone());
    scalar_text(out, "args", delta, |spec| {
        let quoted: Vec<String> = spec.args.iter().map(|arg| format!("{arg:?}")).collect();
        quoted.join(" ")
    });
    values_text(out, "env", delta, |spec| &spec.env);
    scalar_text(out, "url", delta, |spec| spec.url.clone());
    values_text(out, "headers", delta, |spec| &spec.headers);
}

/// One text field of the spec.
fn scalar_text(out: &mut String, name: &str, delta: &ExecDelta, get: fn(&ExecSpec) -> String) {
    let before = delta.before.as_ref().map(get).unwrap_or_default();
    let after = delta.after.as_ref().map(get).unwrap_or_default();
    if before.is_empty() && after.is_empty() {
        return;
    }
    if delta.before.is_none() || delta.after.is_none() || before == after {
        let shown = if after.is_empty() { &before } else { &after };
        out.push_str(&format!("      {name}: {shown}\n"));
    } else {
        out.push_str(&format!(
            "      {name}: {} -> {}\n",
            none_if_empty(&before),
            none_if_empty(&after)
        ));
    }
}

fn none_if_empty(text: &str) -> &str {
    if text.is_empty() {
        "(none)"
    } else {
        text
    }
}

/// One map field (env, headers) name by name.
fn values_text(
    out: &mut String,
    name: &str,
    delta: &ExecDelta,
    get: fn(&ExecSpec) -> &BTreeMap<String, String>,
) {
    let empty = BTreeMap::new();
    let before = delta.before.as_ref().map_or(&empty, get);
    let after = delta.after.as_ref().map_or(&empty, get);
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    if names.is_empty() {
        return;
    }
    out.push_str(&format!("      {name}:\n"));
    let one_sided = delta.before.is_none() || delta.after.is_none();
    for key in names {
        let line = match (before.get(key), after.get(key)) {
            (Some(b), Some(a)) if a != b => format!("        ~ {key}: {b} -> {a}\n"),
            (None, Some(a)) if !one_sided => format!("        + {key}: {a}\n"),
            (Some(b), None) if !one_sided => format!("        - {key}: {b}\n"),
            (b, a) => format!("          {key}: {}\n", a.or(b).map_or("", String::as_str)),
        };
        out.push_str(&line);
    }
}

/// The first pins a pull made, under [`NEW_PINS_HEADER`]; nothing when there
/// are none.
pub fn new_pins_text(changes: &[PinChange]) -> String {
    if changes.is_empty() {
        return String::new();
    }
    format!("{NEW_PINS_HEADER}\n{}", pin_changes_text(changes))
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
