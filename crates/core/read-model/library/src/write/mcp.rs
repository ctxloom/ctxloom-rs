//! An MCP server a bundle ships, as its tree keeps it (Go's `surface_exec.go`):
//! `mcp/<n>.yaml` holds how it is reached, `mcp/.<n>.meta.yaml` what is said
//! about it; and the patch `mcp server set` applies.

use std::collections::BTreeMap;

use angzarr_yaml::Value;

use crate::model::McpServer;

/// What `mcp server set` was told: a field left `None` is not named and keeps
/// what is stored; one named empty clears it.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct McpPatch {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub url: Option<String>,
    pub headers: Option<BTreeMap<String, String>>,
    pub tags: Option<Vec<String>>,
    pub served_by: Option<String>,
    pub notes: Option<String>,
    pub installation: Option<String>,
}

/// `mcp/<name>.yaml`.
pub fn content_path(name: &str) -> String {
    format!("mcp/{name}.yaml")
}

/// `mcp/.<name>.meta.yaml`.
pub fn meta_path(name: &str) -> String {
    format!("mcp/.{name}.meta.yaml")
}

/// The server a bare `bundle edit --add-mcp <name>` adds: it launches `name`.
pub fn placeholder(name: &str) -> McpServer {
    McpServer {
        command: name.to_string(),
        ..McpServer::default()
    }
}

/// `server` with `patch` laid over it. A named empty list or map is stored as
/// none.
pub fn patched(server: &McpServer, patch: &McpPatch) -> McpServer {
    let mut out = server.clone();
    if let Some(v) = &patch.command {
        out.command.clone_from(v);
    }
    if let Some(v) = &patch.args {
        out.args.clone_from(v);
    }
    if let Some(v) = &patch.env {
        out.env.clone_from(v);
    }
    if let Some(v) = &patch.url {
        out.url.clone_from(v);
    }
    if let Some(v) = &patch.headers {
        out.headers.clone_from(v);
    }
    if let Some(v) = &patch.tags {
        out.tags.clone_from(v);
    }
    if let Some(v) = &patch.served_by {
        out.served_by.clone_from(v);
    }
    if let Some(v) = &patch.notes {
        out.notes.clone_from(v);
    }
    if let Some(v) = &patch.installation {
        out.installation.clone_from(v);
    }
    out
}

/// Why the server cannot be saved: it has not exactly one target (a command
/// with its arguments, a url, or the session's endpoint), or the target is
/// not one. In Go's words.
pub fn fault(server: &McpServer) -> Option<String> {
    crate::document::mcp_target_fault(server)
}

fn text(value: &str) -> Value {
    Value::Scalar(value.to_string())
}

fn list(items: &[String]) -> Value {
    Value::Seq(items.iter().map(|i| text(i)).collect())
}

fn map(entries: &std::collections::BTreeMap<String, String>) -> Value {
    Value::Map(entries.iter().map(|(k, v)| (k.clone(), text(v))).collect())
}

/// The content entries, in order, the ones the server has.
fn content_entries(server: &McpServer) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let mut put = |key: &str, value: Value, present: bool| {
        if present {
            out.push((key.to_string(), value));
        }
    };
    put("command", text(&server.command), !server.command.is_empty());
    put("args", list(&server.args), !server.args.is_empty());
    put("env", map(&server.env), !server.env.is_empty());
    put("url", text(&server.url), !server.url.is_empty());
    put("headers", map(&server.headers), !server.headers.is_empty());
    put(
        "served_by",
        text(&server.served_by),
        !server.served_by.is_empty(),
    );
    out
}

/// The sidecar entries, in order, the ones the server has.
fn meta_entries(server: &McpServer) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    if !server.tags.is_empty() {
        out.push(("tags".to_string(), list(&server.tags)));
    }
    if !server.notes.is_empty() {
        out.push(("notes".to_string(), text(&server.notes)));
    }
    if !server.installation.is_empty() {
        out.push(("installation".to_string(), text(&server.installation)));
    }
    out
}

/// The content file: `command`, `args`, `env`, `url`, `headers`, `served_by`,
/// in that order, `{}` when the server has none of them.
pub fn content_file(server: &McpServer) -> String {
    angzarr_yaml::write(&Value::Map(content_entries(server)), usize::MAX)
}

/// The sidecar: `tags`, `notes`, `installation`; none when there is nothing
/// to say.
pub fn meta_file(server: &McpServer) -> Option<String> {
    let entries = meta_entries(server);
    (!entries.is_empty()).then(|| angzarr_yaml::write(&Value::Map(entries), usize::MAX))
}

/// What an editor is given: every field the server has, as one document.
pub fn editor_text(server: &McpServer) -> String {
    let mut entries = content_entries(server);
    entries.extend(meta_entries(server));
    angzarr_yaml::write(&Value::Map(entries), usize::MAX)
}

const KEYS: [&str; 9] = [
    "command",
    "args",
    "env",
    "url",
    "headers",
    "served_by",
    "tags",
    "notes",
    "installation",
];

/// The server an editor left: the document of [`editor_text`], read strictly.
pub fn parse_edited(text: &str) -> Result<McpServer, String> {
    if text.trim().is_empty() {
        return Err("the edited MCP config is empty".to_string());
    }
    let value = angzarr_yaml::parse(text).map_err(|e| format!("invalid YAML: {e}"))?;
    let Value::Map(entries) = &value else {
        return Err("the edited MCP config is not a mapping".to_string());
    };
    if let Some((key, _)) = entries.iter().find(|(k, _)| !KEYS.contains(&k.as_str())) {
        return Err(format!(
            "unknown key `{key}` in the MCP config (valid keys: {})",
            KEYS.join(", ")
        ));
    }
    crate::document::mcp_server(&value, "mcp").map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "mcp.test.rs"]
mod tests;
