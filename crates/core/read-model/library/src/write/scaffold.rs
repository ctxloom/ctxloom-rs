//! What `config create` leaves in a new project (Go's
//! `operations.InitializeProject`, `BuildInitialConfig`): the config, the
//! default remotes registry and the seeded default profile. The two texts are
//! Go's `resources/` at release/0.7 ca14d6046, byte for byte.

use angzarr_yaml::Value;

use super::section::{render_sorted, sorted};

/// `remotes.yaml` as a new project starts with it.
pub const DEFAULT_REMOTES: &str = include_str!("../../resources/default-remotes.yaml");

/// The `default` profile a new project is seeded with.
pub const SEED_PROFILE: &str = include_str!("../../resources/seed-profile.yaml");

/// The name of the seeded profile, and of the agent bound to it.
pub const SEED_NAME: &str = "default";

fn text(value: &str) -> Value {
    Value::Scalar(value.to_string())
}

fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// The registry's entries for `engine`, and the labels that play its two
/// roles: the shipped registry marks claude-code's primary (`opus`) and fast
/// (`haiku`) entries; an engine it does not mark is one entry of its own
/// serving both.
fn registry(engine: &str) -> (Value, String, String) {
    if engine == "claude-code" {
        let entry = |model: &str| map(vec![("model", text(model)), ("type", text("claude-code"))]);
        return (
            map(vec![
                ("claude-code", entry("opus")),
                ("claude-fast", entry("haiku")),
            ]),
            "claude-code".to_string(),
            "claude-fast".to_string(),
        );
    }
    (
        map(vec![(engine, map(vec![("type", text(engine))]))]),
        engine.to_string(),
        engine.to_string(),
    )
}

/// The config a new project starts as, as a value: keys sorted.
pub fn initial_sections(engine: &str) -> Value {
    let (configs, primary, fast) = registry(engine);
    sorted(&map(vec![
        (
            "agents",
            map(vec![(
                SEED_NAME,
                map(vec![
                    ("engine_home", text("session")),
                    ("llm", text(&primary)),
                    ("profiles", Value::Seq(vec![text(SEED_NAME)])),
                    ("runtime", text("host")),
                ]),
            )]),
        ),
        ("auth", text("token")),
        ("config", map(vec![("use_distilled", text("true"))])),
        ("default_agent", text(SEED_NAME)),
        (
            "llm",
            map(vec![
                ("configs", configs),
                (
                    "defaults",
                    map(vec![("fast", text(&fast)), ("primary", text(&primary))]),
                ),
            ]),
        ),
        ("schema_version", text("8")),
    ]))
}

/// The config a new project starts as, for `engine` (a name the caller has
/// checked): the registry's primary and fast entries of that engine, or a
/// single entry serving both roles; the default agent bound to the seeded
/// profile. Keys sorted, as a fresh document is.
pub fn initial_config(engine: &str) -> String {
    render_sorted(&initial_sections(engine))
}

#[cfg(test)]
#[path = "scaffold.test.rs"]
mod tests;
