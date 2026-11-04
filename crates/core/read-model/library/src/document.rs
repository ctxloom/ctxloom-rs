//! A bundle read from one inline document (Go's `bundles.ParseBundle`): the
//! envelope and every item in a single YAML mapping. Companion loadouts are
//! this form, and so are the older bundle files; a tree's envelope shares the
//! envelope's reading.
//!
//! The envelope's `schema_version` names a generation, 0 to
//! [`CURRENT`]: an older document is migrated in memory (generation 0 to 1
//! renames `prompts:` to `commands:` and an item's `llm:` to `exports:`;
//! 1 to 2 moves a profile's references onto the canonical grammar; 2 to 3 and
//! 3 to 4 only widen what is read), a newer one is refused, and nothing is
//! written back.

use std::collections::BTreeMap;

use angzarr_yaml::Value;

use crate::error::BundleError;
use crate::model::{Bundle, Command, Fragment, Hook, ItemBody, McpServer, Skill, HOOK_EVENTS};
use crate::profile::{Located, Profile};
use crate::yamlx::{self, key_line, nearest};

/// The generation this build reads and writes.
pub const CURRENT: u32 = 4;

/// The generation at which a profile's stored references are canonical.
pub const PROFILE_REFS: u32 = 2;

/// The keys a bundle document may declare at its top level.
const TOP_KEYS: [&str; 15] = [
    "author",
    "commands",
    "description",
    "fragments",
    "hooks",
    "installation",
    "mcp",
    "name",
    "notes",
    "profiles",
    "requires",
    "schema_version",
    "skills",
    "tags",
    "version",
];
const FRAGMENT_KEYS: [&str; 9] = [
    "content",
    "content_hash",
    "distilled",
    "distilled_by",
    "installation",
    "no_distill",
    "notes",
    "premise",
    "tags",
];
const COMMAND_KEYS: [&str; 10] = [
    "content",
    "content_hash",
    "description",
    "distilled",
    "distilled_by",
    "exports",
    "installation",
    "no_distill",
    "notes",
    "tags",
];
const MCP_KEYS: [&str; 9] = [
    "args",
    "command",
    "env",
    "headers",
    "installation",
    "notes",
    "served_by",
    "tags",
    "url",
];
const SKILL_KEYS: [&str; 4] = ["exports", "notes", "path", "tags"];
const HOOKS_KEYS: [&str; 8] = [
    "post_file_edit",
    "post_tool",
    "pre_shell",
    "pre_tool",
    "session_end",
    "session_start",
    "turn_end",
    "turn_start",
];
const HOOK_KEYS: [&str; 10] = [
    "args",
    "async",
    "command",
    "order",
    "pre_tool_fallback",
    "prompt",
    "tags",
    "timeout",
    "tool",
    "type",
];
const REQUIRES_KEYS: [&str; 1] = ["commands"];

/// The neutral tool classes a hook may narrow to.
pub const TOOL_CLASSES: [&str; 3] = ["shell", "file_edit", "skill"];

/// Parse an inline bundle document. An unknown key is refused naming it and
/// the near one; an empty document, one with no version and no items, is
/// refused; an entry under `skills:` carrying `content:` (a command's shape)
/// is refused.
pub fn parse_document(text: &str) -> Result<Bundle, BundleError> {
    parse_with_generation(text).map(|(bundle, _)| bundle)
}

/// As [`parse_document`], with the generation the document declared (a tree's
/// items are at their envelope's).
pub fn parse_with_generation(text: &str) -> Result<(Bundle, u32), BundleError> {
    let mut root = crate::yamlx::parse(text).map_err(|cause| BundleError::Syntax { cause })?;
    let generation = read_generation(&root)?;
    migrate(&mut root, generation);
    check_vocabulary(&root, text)?;
    let mut bundle = decode(&root, text)?;
    if generation < PROFILE_REFS {
        for profile in bundle.profiles.values_mut() {
            profile.canonicalize_refs();
        }
    }
    if declares_nothing(&bundle) {
        return Err(BundleError::Empty {
            bytes: text.len() as u64,
        });
    }
    Ok((bundle, generation))
}

fn declares_nothing(bundle: &Bundle) -> bool {
    bundle.version.is_empty()
        && bundle.fragments.is_empty()
        && bundle.commands.is_empty()
        && bundle.skills.is_empty()
        && bundle.mcp.is_empty()
        && bundle.profiles.is_empty()
        && bundle.hooks.is_empty()
}

/// The generation the envelope declares: none is generation 0.
fn read_generation(root: &Value) -> Result<u32, BundleError> {
    let Some(stamp) = root.get("schema_version") else {
        return Ok(0);
    };
    let generation: u64 = match stamp {
        Value::Scalar(s) => s.trim().parse().map_err(|_| BundleError::NotInteger)?,
        _ => return Err(BundleError::NotInteger),
    };
    if generation > u64::from(CURRENT) {
        return Err(BundleError::Newer {
            generation,
            current: u64::from(CURRENT),
        });
    }
    Ok(generation as u32)
}

/// Rename `old` to `new` in a mapping, keeping its place; when `new` is there
/// already the legacy `old` is dropped (the current key wins).
fn rename_key(entries: &mut Vec<(String, Value)>, old: &str, new: &str) {
    let Some(at) = entries.iter().position(|(k, _)| k == old) else {
        return;
    };
    if entries.iter().any(|(k, _)| k == new) {
        entries.remove(at);
    } else {
        entries[at].0 = new.to_string();
    }
}

/// The in-memory migrations from `generation` up to [`CURRENT`] that edit the
/// document (the later steps only widen what is read).
fn migrate(root: &mut Value, generation: u32) {
    if generation >= 1 {
        return;
    }
    let Value::Map(entries) = root else { return };
    rename_key(entries, "prompts", "commands");
    for (key, items) in entries.iter_mut() {
        if key != "commands" && key != "skills" {
            continue;
        }
        if let Value::Map(items) = items {
            for (_, item) in items.iter_mut() {
                if let Value::Map(fields) = item {
                    rename_key(fields, "llm", "exports");
                }
            }
        }
    }
}

fn unknown(key: &str, path: &[&str], text: &str, site: &str, valid: &[&str]) -> BundleError {
    let mut full: Vec<&str> = path.to_vec();
    full.push(key);
    BundleError::UnknownKey {
        key: key.to_string(),
        line: key_line(text, &full),
        site: site.to_string(),
        suggestion: nearest(key, valid),
        valid: valid.join(", "),
    }
}

/// Refuse the first key the schema does not model, wherever it sits.
fn check_vocabulary(root: &Value, text: &str) -> Result<(), BundleError> {
    let entries = match root {
        Value::Null => return Ok(()),
        Value::Map(entries) => entries,
        other => {
            return Err(BundleError::BadValue {
                path: "(document)".to_string(),
                detail: format!(
                    "expected a mapping at the top level, found {}",
                    yamlx::kind(other)
                ),
            })
        }
    };
    for (key, value) in entries {
        if !TOP_KEYS.contains(&key.as_str()) {
            return Err(unknown(
                key,
                &[],
                text,
                "the top level of the bundle",
                &TOP_KEYS,
            ));
        }
        match key.as_str() {
            "fragments" => check_items(value, key, &FRAGMENT_KEYS, text)?,
            "commands" => check_items(value, key, &COMMAND_KEYS, text)?,
            "mcp" => check_items(value, key, &MCP_KEYS, text)?,
            "skills" => check_items(value, key, &SKILL_KEYS, text)?,
            "requires" => check_keys(value, &["requires"], "`requires`", &REQUIRES_KEYS, text)?,
            "hooks" => check_hooks(value, text)?,
            _ => {}
        }
    }
    Ok(())
}

/// The keys of `value` (a mapping) against `valid`.
fn check_keys(
    value: &Value,
    path: &[&str],
    site: &str,
    valid: &[&str],
    text: &str,
) -> Result<(), BundleError> {
    if let Value::Map(entries) = value {
        for (key, _) in entries {
            if !valid.contains(&key.as_str()) {
                return Err(unknown(key, path, text, site, valid));
            }
        }
    }
    Ok(())
}

/// Every item of the kind `kind` against its vocabulary.
fn check_items(items: &Value, kind: &str, valid: &[&str], text: &str) -> Result<(), BundleError> {
    if let Value::Map(entries) = items {
        let site = format!("`{kind}.<name>`");
        for (name, item) in entries {
            check_keys(item, &[kind, name], &site, valid, text)?;
        }
    }
    Ok(())
}

fn check_hooks(hooks: &Value, text: &str) -> Result<(), BundleError> {
    check_keys(hooks, &["hooks"], "`hooks`", &HOOKS_KEYS, text)?;
    if let Value::Map(events) = hooks {
        for (event, list) in events {
            if let Value::Seq(items) = list {
                for item in items {
                    check_keys(
                        item,
                        &["hooks", event, "[]"],
                        "`hooks.pre_tool[]`",
                        &HOOK_KEYS,
                        text,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn bad(path: impl Into<String>, detail: impl Into<String>) -> BundleError {
    BundleError::BadValue {
        path: path.into(),
        detail: detail.into(),
    }
}

/// `result` with its refusal located at `path`.
fn at<T>(result: Result<T, String>, path: &str) -> Result<T, BundleError> {
    result.map_err(|detail| bad(path, detail))
}

/// The mapping `value` is, or none for a null; anything else is refused.
fn mapping<'a>(value: &'a Value, path: &str) -> Result<&'a [(String, Value)], BundleError> {
    match value {
        Value::Null => Ok(&[]),
        Value::Map(entries) => Ok(entries),
        other => Err(bad(
            path,
            format!("expected a mapping, found {}", yamlx::kind(other)),
        )),
    }
}

/// The text field `key` of `item`.
fn field(item: &Value, key: &str, path: &str) -> Result<String, BundleError> {
    match item.get(key) {
        Some(v) => at(yamlx::string(v), &format!("{path}.{key}")),
        None => Ok(String::new()),
    }
}

fn list_field(item: &Value, key: &str, path: &str) -> Result<Vec<String>, BundleError> {
    match item.get(key) {
        Some(v) => at(yamlx::string_list(v), &format!("{path}.{key}")),
        None => Ok(Vec::new()),
    }
}

fn flag(item: &Value, key: &str, path: &str) -> Result<bool, BundleError> {
    match item.get(key) {
        Some(v) => at(yamlx::boolean(v), &format!("{path}.{key}")),
        None => Ok(false),
    }
}

/// The fields a fragment and a command hold alike.
pub(crate) fn item_body(item: &Value, path: &str) -> Result<ItemBody, BundleError> {
    Ok(ItemBody {
        tags: list_field(item, "tags", path)?,
        notes: field(item, "notes", path)?,
        installation: field(item, "installation", path)?,
        content: field(item, "content", path)?,
        content_hash: field(item, "content_hash", path)?,
        distilled: field(item, "distilled", path)?,
        distilled_by: field(item, "distilled_by", path)?,
        no_distill: flag(item, "no_distill", path)?,
    })
}

/// The per-engine blocks of `item` (`exports:`), each a mapping, as authored.
pub(crate) fn exports(item: &Value, path: &str) -> Result<BTreeMap<String, Value>, BundleError> {
    let Some(blocks) = item.get("exports") else {
        return Ok(BTreeMap::new());
    };
    exports_of(blocks, &format!("{path}.exports"))
}

/// As [`exports`], for the blocks value itself.
pub(crate) fn exports_of(
    blocks: &Value,
    path: &str,
) -> Result<BTreeMap<String, Value>, BundleError> {
    match blocks {
        Value::Null => Ok(BTreeMap::new()),
        Value::Map(entries) => entries
            .iter()
            .map(|(engine, block)| match block {
                Value::Map(_) | Value::Null => Ok((engine.clone(), block.clone())),
                other => Err(bad(
                    format!("{path}.{engine}"),
                    format!(
                        "exports: expected a mapping of engine name to block, got {}",
                        yamlx::kind(other)
                    ),
                )),
            })
            .collect(),
        other => Err(bad(
            path,
            format!(
                "exports: expected a mapping of engine name to block, got {}",
                yamlx::kind(other)
            ),
        )),
    }
}

/// An MCP server from its mapping, its target checked as Go's loader checks it.
pub(crate) fn mcp_server(item: &Value, path: &str) -> Result<McpServer, BundleError> {
    let map = |key: &str| -> Result<BTreeMap<String, String>, BundleError> {
        match item.get(key) {
            Some(v) => at(yamlx::string_map(v), &format!("{path}.{key}")),
            None => Ok(BTreeMap::new()),
        }
    };
    Ok(McpServer {
        command: field(item, "command", path)?,
        args: list_field(item, "args", path)?,
        env: map("env")?,
        url: field(item, "url", path)?,
        headers: map("headers")?,
        served_by: field(item, "served_by", path)?,
        tags: list_field(item, "tags", path)?,
        notes: field(item, "notes", path)?,
        installation: field(item, "installation", path)?,
    })
}

/// Why an MCP server's target is not one a writer can use, in Go's words.
fn mcp_target_fault(server: &McpServer) -> Option<String> {
    let targets = usize::from(!server.command.is_empty() || !server.args.is_empty())
        + usize::from(!server.url.is_empty())
        + usize::from(!server.served_by.is_empty());
    let fault = if targets == 0 {
        "mcp server: none of command, url or served_by is set".to_string()
    } else if targets > 1 {
        "mcp server: more than one of command (with args), url and served_by is set; a server is stdio (command), remote (url) or served by the session (served_by), not several".to_string()
    } else if !server.served_by.is_empty() {
        if server.served_by == "session-endpoint" {
            return None;
        }
        format!(
            "mcp server: served_by must be session-endpoint, got \"{}\"",
            server.served_by
        )
    } else if server.url.is_empty() {
        if server.command.is_empty() {
            "mcp server: none of command, url or served_by is set".to_string()
        } else {
            return None;
        }
    } else if server.url.starts_with("http://") || server.url.starts_with("https://") {
        return None;
    } else {
        format!(
            "mcp server: url scheme must be http or https: \"{}\"",
            server.url
        )
    };
    Some(fault)
}

/// A hook from its mapping.
pub(crate) fn hook(item: &Value, event: &str, name: &str, path: &str) -> Result<Hook, BundleError> {
    let tool = field(item, "tool", path)?;
    if !tool.is_empty() && !TOOL_CLASSES.contains(&tool.as_str()) {
        return Err(bad(
            path,
            format!(
                "tool \"{tool}\" is not a tool class; use one of {}",
                TOOL_CLASSES.join(", ")
            ),
        ));
    }
    Ok(Hook {
        event: event.to_string(),
        name: name.to_string(),
        order: match item.get("order") {
            Some(Value::Null) | None => None,
            Some(v) => Some(at(yamlx::integer(v), &format!("{path}.order"))?),
        },
        tool,
        kind: field(item, "type", path)?,
        command: field(item, "command", path)?,
        args: list_field(item, "args", path)?,
        prompt: field(item, "prompt", path)?,
        timeout: match item.get("timeout") {
            Some(v) => at(yamlx::integer(v), &format!("{path}.timeout"))?,
            None => 0,
        },
        is_async: flag(item, "async", path)?,
        pre_tool_fallback: flag(item, "pre_tool_fallback", path)?,
        tags: list_field(item, "tags", path)?,
    })
}

fn decode_hooks(hooks: &Value) -> Result<Vec<Hook>, BundleError> {
    let mut out = Vec::new();
    for event in HOOK_EVENTS {
        let Some(list) = hooks.get(event) else {
            continue;
        };
        match list {
            Value::Null => {}
            Value::Seq(items) => {
                for (i, item) in items.iter().enumerate() {
                    let path = format!("hooks.{event}[{i}]");
                    mapping(item, &path)?;
                    out.push(hook(item, event, &i.to_string(), &path)?);
                }
            }
            other => {
                return Err(bad(
                    format!("hooks.{event}"),
                    format!("expected a list, found {}", yamlx::kind(other)),
                ))
            }
        }
    }
    Ok(out)
}

/// Every name and item of the kind's mapping.
fn each_item<T>(
    items: Option<&Value>,
    kind: &str,
    mut read: impl FnMut(&str, &Value, &str) -> Result<T, BundleError>,
) -> Result<BTreeMap<String, T>, BundleError> {
    let mut out = BTreeMap::new();
    if let Some(items) = items {
        for (name, item) in mapping(items, kind)? {
            let path = format!("{kind}.{name}");
            mapping(item, &path)?;
            out.insert(name.clone(), read(name, item, &path)?);
        }
    }
    Ok(out)
}

fn decode(root: &Value, text: &str) -> Result<Bundle, BundleError> {
    let mut bundle = Bundle {
        name: field(root, "name", "")?,
        version: field(root, "version", "")?,
        tags: list_field(root, "tags", "")?,
        author: field(root, "author", "")?,
        description: field(root, "description", "")?,
        notes: field(root, "notes", "")?,
        installation: field(root, "installation", "")?,
        ..Bundle::default()
    };
    bundle.fragments = each_item(root.get("fragments"), "fragments", |_, item, path| {
        Ok(Fragment {
            body: item_body(item, path)?,
            premise: field(item, "premise", path)?,
        })
    })?;
    bundle.commands = each_item(root.get("commands"), "commands", |_, item, path| {
        Ok(Command {
            body: item_body(item, path)?,
            description: field(item, "description", path)?,
            exports: exports(item, path)?,
        })
    })?;
    bundle.mcp = each_item(root.get("mcp"), "mcp", |name, item, path| {
        let server = mcp_server(item, path)?;
        match mcp_target_fault(&server) {
            Some(fault) => Err(bad(format!("bundle mcp server \"{name}\""), fault)),
            None => Ok(server),
        }
    })?;
    bundle.skills = each_item(root.get("skills"), "skills", |_, item, path| {
        Ok(Skill {
            path: field(item, "path", path)?,
            tags: list_field(item, "tags", path)?,
            notes: field(item, "notes", path)?,
            exports: exports(item, path)?,
            files: Vec::new(),
        })
    })?;
    bundle.profiles = each_item(root.get("profiles"), "profiles", |name, item, _| {
        let at = Located {
            text,
            prefix: &["profiles", name],
        };
        let mut profile =
            Profile::from_value(item, false, &at).map_err(|cause| BundleError::Profile {
                name: name.to_string(),
                cause,
            })?;
        profile.name = name.to_string();
        Ok(profile)
    })?;
    if let Some(hooks) = root.get("hooks") {
        bundle.hooks = decode_hooks(hooks)?;
    }
    if let Some(requires) = root.get("requires") {
        bundle.requires_commands = list_field(requires, "commands", "requires")?;
        for command in &bundle.requires_commands {
            if !is_command_name(command) {
                return Err(bad(
                    "requires.commands",
                    format!("\"{command}\" is not a command name (letters, digits, '.', '_', '+', '-'; no path separator)"),
                ));
            }
        }
    }
    Ok(bundle)
}

/// Go's `commandNamePattern`: `^[A-Za-z0-9][A-Za-z0-9._+-]*$`.
fn is_command_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

#[cfg(test)]
#[path = "document.test.rs"]
mod tests;
