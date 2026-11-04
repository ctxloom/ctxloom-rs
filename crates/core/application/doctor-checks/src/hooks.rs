//! Reading the engine's settings files (JSON): which hook entries are
//! ctxloom's, which of them run a subcommand this ctxloom lacks, and which
//! MCP entry launches a server that does not exist.

use serde_json::Value;

/// The `ctxloom hook` subcommands this ctxloom has (plan 8.4; `stamp-plan` is
/// taskloom's). Any other `ctxloom hook <name>` is a leftover of an older
/// release: the rule is general, not a list of retired names.
pub const LIVE_HOOKS: [&str; 7] = [
    "session-start",
    "session-bind",
    "hud",
    "mail-drain",
    "mail-peek",
    "skill-mates",
    "permission",
];

/// A hook entry running a `ctxloom hook` subcommand that is gone.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Stale {
    /// The engine's event (`SessionStart`).
    pub event: String,
    /// The subcommand (`inject-context`).
    pub sub: String,
}

/// A command line's words: whitespace separates, quotes group and vanish.
fn words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let (mut word, mut quote, mut any) = (String::new(), None, false);
    for c in command.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                any = true;
            }
            (None, c) if c.is_whitespace() => {
                if any {
                    words.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            (None, c) => {
                word.push(c);
                any = true;
            }
        }
    }
    if any {
        words.push(word);
    }
    words
}

/// Whether `program` is ctxloom itself, by its file name.
fn is_ctxloom(program: &str) -> bool {
    let name = program.rsplit(['/', '\\']).next().unwrap_or_default();
    name == "ctxloom" || name == "ctxloom.exe"
}

/// `ctxloom hook <sub>`: the subcommand, when `command` runs one.
fn hook_sub(command: &str) -> Option<String> {
    let words = words(command);
    match words.as_slice() {
        [program, hook, sub, ..]
            if is_ctxloom(program) && hook == "hook" && !sub.starts_with('-') =>
        {
            Some(sub.clone())
        }
        _ => None,
    }
}

fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}

/// The `(event, command)` of every hook entry in a settings document.
fn entries(doc: &Value) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let Some(events) = doc.get("hooks").and_then(Value::as_object) else {
        return found;
    };
    for (event, groups) in events {
        for group in groups.as_array().into_iter().flatten() {
            for hook in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(command) = hook.get("command").and_then(Value::as_str) {
                    found.push((event.clone(), command.to_string()));
                }
            }
        }
    }
    found
}

fn stale_of(event: &str, command: &str) -> Option<Stale> {
    let sub = hook_sub(command)?;
    (!LIVE_HOOKS.contains(&sub.as_str())).then(|| Stale {
        event: event.to_string(),
        sub,
    })
}

/// Whether the settings register any ctxloom hook.
pub fn registered(text: &str) -> bool {
    parse(text).is_some_and(|doc| entries(&doc).iter().any(|(_, c)| hook_sub(c).is_some()))
}

/// The stale entries of a settings file.
pub fn stale(text: &str) -> Vec<Stale> {
    parse(text)
        .map(|doc| {
            entries(&doc)
                .iter()
                .filter_map(|(event, command)| stale_of(event, command))
                .collect()
        })
        .unwrap_or_default()
}

/// The settings with the stale entries gone and nothing else touched (a
/// group left with no hook goes, and an event left with no group), and which
/// went; none where there is nothing stale.
pub fn strip(text: &str) -> Option<(String, Vec<Stale>)> {
    let mut doc = parse(text)?;
    let mut gone = Vec::new();
    let events = doc.get_mut("hooks")?.as_object_mut()?;
    for (event, groups) in events.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        for group in groups.iter_mut() {
            let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            hooks.retain(|hook| {
                let command = hook
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match stale_of(event, command) {
                    Some(stale) => {
                        gone.push(stale);
                        false
                    }
                    None => true,
                }
            });
        }
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|hooks| !hooks.is_empty())
        });
    }
    events.retain(|_, groups| groups.as_array().is_none_or(|groups| !groups.is_empty()));
    if gone.is_empty() {
        return None;
    }
    let mut out = serde_json::to_string_pretty(&doc).ok()?;
    out.push('\n');
    Some((out, gone))
}

/// Whether a settings document's `mcpServers.ctxloom` launches a process
/// (`command`, no `url`) running `ctxloom mcp ...`.
pub fn launches_ctxloom_mcp(text: &str) -> bool {
    let Some(doc) = parse(text) else { return false };
    let Some(entry) = doc.get("mcpServers").and_then(|s| s.get("ctxloom")) else {
        return false;
    };
    if entry.get("url").is_some() {
        return false;
    }
    let Some(command) = entry.get("command").and_then(Value::as_str) else {
        return false;
    };
    let first_arg = entry
        .get("args")
        .and_then(Value::as_array)
        .and_then(|args| args.first())
        .and_then(Value::as_str);
    is_ctxloom(command) && first_arg == Some("mcp")
}
