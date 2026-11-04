//! The items of two bundles compared, kind by kind, with what a hook or an MCP
//! server runs (Go's `diffBundleItems`).

use std::collections::BTreeMap;

use library::model::{Bundle, Hook, McpServer};

use crate::{Change, ExecDelta, ExecSpec, ItemChange};

/// The stand-in for a value that may be a secret: the first 8 hex characters
/// of its SHA-256 in angle brackets (`<a1b2c3d4>`). Equal values match and a
/// changed value reads as changed, without the value ever being shown.
pub fn fingerprint(value: &str) -> String {
    let digest = library::sha256::digest(value.as_bytes());
    let hex: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
    format!("<{hex}>")
}

/// The items that differ between `from` and `to`, in a fixed kind order (hook,
/// mcp, skill, command, fragment, profile), each kind by sorted name.
pub fn diff_items(from: &Bundle, to: &Bundle) -> Vec<ItemChange> {
    let mut out = diff_map("hook", &hooks(from), &hooks(to), Some(hook_exec));
    out.extend(diff_map("mcp", &from.mcp, &to.mcp, Some(mcp_exec)));
    out.extend(diff_map("skill", &from.skills, &to.skills, None));
    out.extend(diff_map("command", &from.commands, &to.commands, None));
    out.extend(diff_map("fragment", &from.fragments, &to.fragments, None));
    out.extend(diff_map("profile", &from.profiles, &to.profiles, None));
    out
}

/// A bundle's hooks by their identity, `<event>/<index>`.
fn hooks(bundle: &Bundle) -> BTreeMap<String, Hook> {
    bundle
        .hook_ids()
        .into_iter()
        .zip(bundle.hooks.iter().cloned())
        .collect()
}

/// The items of one kind that differ, from's names (sorted) then the names
/// only to has (sorted). `exec`, where the kind has one, gives what an item
/// runs.
fn diff_map<V: PartialEq>(
    kind: &'static str,
    from: &BTreeMap<String, V>,
    to: &BTreeMap<String, V>,
    exec: Option<fn(&V) -> Option<ExecSpec>>,
) -> Vec<ItemChange> {
    let names = from
        .keys()
        .chain(to.keys().filter(|name| !from.contains_key(*name)));
    let mut out = Vec::new();
    for name in names {
        let (before, after) = (from.get(name), to.get(name));
        let change = match (before, after) {
            (None, Some(_)) => Change::Added,
            (Some(_), None) => Change::Removed,
            (Some(b), Some(a)) if b != a => Change::Modified,
            _ => continue,
        };
        let exec = exec.and_then(|exec| {
            let delta = ExecDelta {
                before: before.and_then(exec),
                after: after.and_then(exec),
            };
            (delta.before.is_some() || delta.after.is_some()).then_some(delta)
        });
        out.push(ItemChange {
            kind,
            name: name.clone(),
            change,
            exec,
        });
    }
    out
}

/// What a hook runs; a hook that runs no command has none.
fn hook_exec(hook: &Hook) -> Option<ExecSpec> {
    (!hook.command.is_empty()).then(|| ExecSpec {
        command: hook.command.clone(),
        args: hook.args.clone(),
        ..ExecSpec::default()
    })
}

/// What an MCP server runs or dials. Env and header values are fingerprinted
/// here, at the source, so no rendering can carry a raw secret.
fn mcp_exec(server: &McpServer) -> Option<ExecSpec> {
    let fingerprinted = |values: &BTreeMap<String, String>| {
        values
            .iter()
            .map(|(name, value)| (name.clone(), fingerprint(value)))
            .collect()
    };
    Some(ExecSpec {
        command: server.command.clone(),
        args: server.args.clone(),
        env: fingerprinted(&server.env),
        url: server.url.clone(),
        headers: fingerprinted(&server.headers),
    })
}

#[cfg(test)]
#[path = "item.test.rs"]
mod tests;
