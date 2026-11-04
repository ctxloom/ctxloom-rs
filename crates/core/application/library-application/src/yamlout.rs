//! A profile, an MCP server and a skill as the YAML Go marshals them: fields
//! in the order of its record, an empty one left out.

use angzarr_yaml::Value;
use library::config::typed::{to_yaml, Typed};
use library::model::{McpServer, Skill};
use library::profile::Profile;

fn text(s: &str) -> Typed {
    Typed::Str(s.to_string())
}

fn list(items: &[String]) -> Typed {
    Typed::Seq(items.iter().map(|s| text(s)).collect())
}

fn strings(map: &std::collections::BTreeMap<String, String>) -> Typed {
    Typed::Map(map.iter().map(|(k, v)| (k.clone(), text(v))).collect())
}

/// A parsed value as it was written, scalars read as what they look like.
fn value(v: &Value) -> Typed {
    match v {
        Value::Null => Typed::Null,
        Value::Scalar(s) => match s.as_str() {
            "true" => Typed::Bool(true),
            "false" => Typed::Bool(false),
            other => other
                .parse()
                .map_or_else(|_| Typed::Str(other.to_string()), Typed::Int),
        },
        Value::Seq(items) => Typed::Seq(items.iter().map(value).collect()),
        Value::Map(entries) => {
            let mut sorted: Vec<&(String, Value)> = entries.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            Typed::Map(
                sorted
                    .into_iter()
                    .map(|(k, v)| (k.clone(), value(v)))
                    .collect(),
            )
        }
    }
}

/// Only what is set, in order.
struct Fields(Vec<(String, Typed)>);

impl Fields {
    fn new() -> Fields {
        Fields(Vec::new())
    }

    fn text(&mut self, key: &str, v: &str) {
        if !v.is_empty() {
            self.0.push((key.to_string(), text(v)));
        }
    }

    fn list(&mut self, key: &str, v: &[String]) {
        if !v.is_empty() {
            self.0.push((key.to_string(), list(v)));
        }
    }

    fn map(&mut self, key: &str, v: &std::collections::BTreeMap<String, String>) {
        if !v.is_empty() {
            self.0.push((key.to_string(), strings(v)));
        }
    }

    fn put(&mut self, key: &str, v: Typed) {
        self.0.push((key.to_string(), v));
    }

    fn finish(self) -> String {
        to_yaml(&Typed::Map(self.0))
    }
}

/// An MCP server.
pub fn mcp(server: &McpServer) -> String {
    let mut f = Fields::new();
    f.text("command", &server.command);
    f.list("args", &server.args);
    f.map("env", &server.env);
    f.text("url", &server.url);
    f.map("headers", &server.headers);
    f.text("served_by", &server.served_by);
    f.list("tags", &server.tags);
    f.text("notes", &server.notes);
    f.text("installation", &server.installation);
    f.finish()
}

/// A skill's entry in its bundle.
pub fn skill(skill: &Skill) -> String {
    let mut f = Fields::new();
    f.text("path", &skill.path);
    f.list("tags", &skill.tags);
    f.text("notes", &skill.notes);
    if !skill.exports.is_empty() {
        f.put(
            "exports",
            Typed::Map(
                skill
                    .exports
                    .iter()
                    .map(|(engine, block)| (engine.clone(), value(block)))
                    .collect(),
            ),
        );
    }
    f.finish()
}

/// A profile.
pub fn profile(profile: &Profile) -> String {
    let mut f = Fields::new();
    f.text("description", &profile.description);
    f.list("parents", &profile.parents);
    f.list("tags", &profile.tags);
    f.list("select_tags", &profile.select_tags);
    f.text("llm", &profile.llm);
    f.list("bundles", &profile.bundles);
    f.list("commands", &profile.commands);
    f.list("skills", &profile.skills);
    if !profile.fragments.is_empty() {
        f.put(
            "fragments",
            Typed::Seq(
                profile
                    .fragments
                    .iter()
                    .map(|r| {
                        if r.priority == 0 {
                            text(&r.name)
                        } else {
                            Typed::Map(vec![
                                ("name".to_string(), text(&r.name)),
                                ("priority".to_string(), Typed::Int(r.priority)),
                            ])
                        }
                    })
                    .collect(),
            ),
        );
    }
    f.list("bundle_items", &profile.bundle_items);
    if profile.hooks.has_any() {
        f.put("hooks", hooks(&profile.hooks));
    }
    f.map("variables", &profile.variables);
    f.list("exclude_fragments", &profile.exclude_fragments);
    f.list("exclude_mcp", &profile.exclude_mcp);
    f.list("deny_tools", &profile.deny_tools);
    f.finish()
}

fn hooks(config: &library::profile::HooksConfig) -> Typed {
    let events = |map: &std::collections::BTreeMap<String, Vec<Value>>| {
        Typed::Map(
            map.iter()
                .filter(|(_, hooks)| !hooks.is_empty())
                .map(|(event, hooks)| {
                    (event.clone(), Typed::Seq(hooks.iter().map(value).collect()))
                })
                .collect(),
        )
    };
    let mut out = Vec::new();
    if config.unified.values().any(|h| !h.is_empty()) {
        out.push(("unified".to_string(), events(&config.unified)));
    }
    if config
        .ext
        .values()
        .flat_map(|m| m.values())
        .any(|h| !h.is_empty())
    {
        out.push((
            "ext".to_string(),
            Typed::Map(
                config
                    .ext
                    .iter()
                    .map(|(engine, map)| (engine.clone(), events(map)))
                    .collect(),
            ),
        ));
    }
    Typed::Map(out)
}

#[cfg(test)]
#[path = "yamlout.test.rs"]
mod tests;
