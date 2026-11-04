//! What a config may say (Go's `resources/schema/input/config-schema.json`):
//! its keys, and the shape of each value. A violation is worded as Go words
//! it, so the same file is refused in the same terms.

use std::sync::OnceLock;

use angzarr_yaml::Value;

use crate::yamlx::nearest;

/// One way the document breaks the schema.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Violation {
    /// A key the schema does not describe: its dotted path, where it would
    /// have been legal (`the top level` or a path), and the keys that are.
    UnknownKey {
        path: String,
        site: String,
        known: Vec<String>,
    },
    /// A value of the wrong type: the key, what it holds (`string`, `number`,
    /// `boolean`, `array`, `object`) and what was expected.
    Holds {
        path: String,
        holds: String,
        expected: String,
    },
    /// A value the schema refuses: the key and why.
    Breaks { path: String, detail: String },
}

impl Violation {
    /// The violation as Go words it, naming `file`.
    pub fn text(&self, file: &str) -> String {
        match self {
            Violation::UnknownKey { path, site, known } => {
                let key = path.rsplit('.').next().unwrap_or(path);
                let refs: Vec<&str> = known.iter().map(String::as_str).collect();
                let mut out = format!("unknown key `{path}` in {file}: ctxloom does not know it");
                let near = nearest(key, &refs);
                if !near.is_empty() {
                    out.push_str(&format!(" — did you mean `{near}`?"));
                }
                if !known.is_empty() {
                    out.push_str(&format!(" (known keys at {site}: {})", known.join(", ")));
                }
                out
            }
            Violation::Holds {
                path,
                holds,
                expected,
            } => format!(
                "{file}: `{}` holds a {holds}, expected {expected}",
                shown(path)
            ),
            Violation::Breaks { path, detail } => {
                format!("{file}: `{}` breaks the schema: {detail}", shown(path))
            }
        }
    }

    /// Whether the value is of the wrong shape for its key, so that reading
    /// the config into its model fails (rather than reading on).
    pub fn is_shape(&self) -> bool {
        matches!(self, Violation::Holds { .. })
    }
}

/// A violation's key as it is shown: the document root has no dotted path.
fn shown(path: &str) -> &str {
    if path.is_empty() {
        "(top level)"
    } else {
        path
    }
}

/// What a string must look like.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pat {
    Duration,
    Approval,
    Age,
    ListWindow,
    PrefixKey,
    MemoryLimit,
    PidsLimit,
    Companion,
    NoEquals,
    Url,
}

impl Pat {
    /// The pattern as the schema writes it.
    fn source(self) -> &'static str {
        match self {
            Pat::Duration => "^[0-9]+(\\.[0-9]+)?(ns|us|\u{b5}s|ms|s|m|h)([0-9]+(\\.[0-9]+)?(ns|us|\u{b5}s|ms|s|m|h))*$",
            Pat::Approval => "^([0-9]+(\\.[0-9]+)?(ns|us|ms|s|m|h))+$",
            Pat::Age => "^[0-9]+(d|w|h|m|s)$",
            Pat::ListWindow => "^(all|0|[0-9]+(d|w|h|m|s))$",
            Pat::PrefixKey => "^ctrl[-+].$",
            Pat::MemoryLimit => "^\\s*([0-9]+(\\.[0-9]+)?\\s*([kKmMgGtT]([iI]?[bB])?|[bB])?|[uU][nN][lL][iI][mM][iI][tT][eE][dD])\\s*$",
            Pat::PidsLimit => "^\\s*([0-9]+|[uU][nN][lL][iI][mM][iI][tT][eE][dD])\\s*$",
            Pat::Companion => "^[A-Za-z0-9][A-Za-z0-9._-]*$",
            Pat::NoEquals => "^[^=]+$",
            Pat::Url => "^https?://",
        }
    }

    fn matches(self, s: &str) -> bool {
        match self {
            Pat::Duration => durations(s, &["ns", "us", "\u{b5}s", "ms", "s", "m", "h"]),
            Pat::Approval => durations(s, &["ns", "us", "ms", "s", "m", "h"]),
            Pat::Age => age(s),
            Pat::ListWindow => s == "all" || s == "0" || age(s),
            Pat::PrefixKey => {
                let mut chars = s.chars();
                s.starts_with("ctrl")
                    && matches!(s.chars().nth(4), Some('-' | '+'))
                    && s.chars().count() == 6
                    && chars.nth(5).is_some_and(|c| c != '\n')
            }
            Pat::MemoryLimit => memory_limit(s.trim_matches(char::is_whitespace)),
            Pat::PidsLimit => {
                let t = s.trim_matches(char::is_whitespace);
                t.eq_ignore_ascii_case("unlimited")
                    || (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit()))
            }
            Pat::Companion => {
                let mut chars = s.chars();
                chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
                    && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            }
            Pat::NoEquals => !s.is_empty() && !s.contains('=') && !s.contains('\n'),
            Pat::Url => s.starts_with("http://") || s.starts_with("https://"),
        }
    }
}

/// `[0-9]+(d|w|h|m|s)`.
fn age(s: &str) -> bool {
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && matches!(&s[digits..], "d" | "w" | "h" | "m" | "s")
}

/// One or more `<number><unit>` runs.
fn durations(s: &str, units: &[&str]) -> bool {
    if s.is_empty() {
        return false;
    }
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return false;
    }
    let mut rest = &s[digits..];
    if let Some(after_dot) = rest.strip_prefix('.') {
        let fraction = after_dot.chars().take_while(char::is_ascii_digit).count();
        if fraction == 0 {
            return false;
        }
        rest = &after_dot[fraction..];
    }
    units.iter().any(|unit| {
        rest.strip_prefix(unit)
            .is_some_and(|after| after.is_empty() || durations(after, units))
    })
}

/// `[0-9]+(\.[0-9]+)?\s*([kKmMgGtT]([iI]?[bB])?|[bB])?` or `unlimited`.
fn memory_limit(s: &str) -> bool {
    if s.eq_ignore_ascii_case("unlimited") {
        return true;
    }
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return false;
    }
    let mut rest = &s[digits..];
    if let Some(after_dot) = rest.strip_prefix('.') {
        let fraction = after_dot.chars().take_while(char::is_ascii_digit).count();
        if fraction == 0 {
            return false;
        }
        rest = &after_dot[fraction..];
    }
    let rest = rest.trim_start();
    let mut chars = rest.chars();
    match chars.next() {
        None => true,
        Some('b' | 'B') => chars.next().is_none(),
        Some('k' | 'K' | 'm' | 'M' | 'g' | 'G' | 't' | 'T') => {
            let tail: String = chars.collect();
            matches!(tail.to_lowercase().as_str(), "" | "b" | "ib")
        }
        Some(_) => false,
    }
}

/// How a mapping treats keys its properties do not list.
#[derive(Clone, Debug)]
enum Extra {
    Deny,
    Any,
    Each(Box<Node>),
}

/// The schema, as a tree of what each position holds.
#[derive(Clone, Debug)]
enum Node {
    Str {
        choices: &'static [&'static str],
        pattern: Option<Pat>,
        min_len: bool,
    },
    Int {
        min: Option<i64>,
    },
    Bool,
    Array {
        items: Box<Node>,
        unique: bool,
    },
    Object {
        props: Vec<(&'static str, Node)>,
        extra: Extra,
    },
    /// Any one of the shapes (the limits: a size text or a number).
    AnyOf(Vec<Node>),
    /// An `llm.configs.<label>` entry: one shape per backend `type`.
    Llm,
    /// Whatever there is.
    Free,
}

fn string() -> Node {
    Node::Str {
        choices: &[],
        pattern: None,
        min_len: false,
    }
}

fn non_empty() -> Node {
    Node::Str {
        choices: &[],
        pattern: None,
        min_len: true,
    }
}

fn pattern(p: Pat) -> Node {
    Node::Str {
        choices: &[],
        pattern: Some(p),
        min_len: false,
    }
}

fn choice(choices: &'static [&'static str]) -> Node {
    Node::Str {
        choices,
        pattern: None,
        min_len: false,
    }
}

fn int(min: Option<i64>) -> Node {
    Node::Int { min }
}

fn list(items: Node) -> Node {
    Node::Array {
        items: Box::new(items),
        unique: false,
    }
}

fn object(props: Vec<(&'static str, Node)>) -> Node {
    Node::Object {
        props,
        extra: Extra::Deny,
    }
}

fn map_of(each: Node) -> Node {
    Node::Object {
        props: Vec::new(),
        extra: Extra::Each(Box::new(each)),
    }
}

const RUNTIMES: &[&str] = &["host", "container-rootless", "container-rootful"];
const ROLES: &[&str] = &["primary", "fast"];

fn permission_props() -> Vec<(&'static str, Node)> {
    vec![
        ("approver", choice(&["human", "none", "reviewer"])),
        ("approval_timeout", pattern(Pat::Approval)),
        ("sandbox", choice(&["read-only", "workspace-write", "full"])),
        ("network", Node::Bool),
    ]
}

/// The common properties of an `llm.configs` entry, then its own.
fn llm_shape(own: Vec<(&'static str, Node)>) -> Vec<(&'static str, Node)> {
    let mut props = vec![
        ("role", choice(ROLES)),
        (
            "permissions",
            Node::Object {
                props: permission_props(),
                extra: Extra::Any,
            },
        ),
        ("model", string()),
        ("context_window", int(Some(1))),
    ];
    props.extend(own);
    props
}

/// The backend `type`s an `llm.configs` entry may declare.
const LLM_TYPES: [&str; 6] = [
    "claude-code",
    "mock",
    "mock-lossy",
    "mock-launch",
    "mock-noskills",
    "opencode",
];

/// The properties of the entry of backend `kind`, `type` first.
fn llm_props(kind: &str) -> Vec<(&'static str, Node)> {
    let mut props = vec![("type", choice(&[]))];
    let own = match kind {
        "claude-code" => vec![("binary_path", string()), ("args", list(string()))],
        "opencode" => vec![
            ("binary_path", string()),
            ("args", list(string())),
            (
                "provider",
                object(vec![
                    ("base_url", pattern(Pat::Url)),
                    ("output_limit", int(Some(1))),
                    ("header_timeout_ms", int(Some(0))),
                    ("chunk_timeout_ms", int(Some(1))),
                ]),
            ),
            ("model_options", Node::Free),
            ("snapshot", Node::Bool),
            ("title_generation", Node::Bool),
            ("share", choice(&["manual", "auto", "disabled"])),
        ],
        _ => vec![("mock_control", map_of(string()))],
    };
    props.extend(llm_shape(own));
    props
}

/// The schema of a config (`resources/schema/input/config-schema.json`).
fn root() -> Node {
    let limit = |p: Pat| Node::AnyOf(vec![pattern(p), int(Some(0))]);
    Node::Object {
        props: vec![
            ("schema_version", int(None)),
            (
                "llm",
                object(vec![
                    ("configs", map_of(Node::Llm)),
                    (
                        "defaults",
                        object(vec![("primary", string()), ("fast", string())]),
                    ),
                ]),
            ),
            (
                "editor",
                object(vec![("command", string()), ("args", list(string()))]),
            ),
            (
                "config",
                object(vec![
                    ("use_distilled", Node::Bool),
                    ("silence_unsupported", Node::Bool),
                    ("statusline", Node::Bool),
                ]),
            ),
            ("sync", object(vec![("auto_sync", Node::Bool)])),
            ("workspace", choice(&["none", "worktree"])),
            ("runtime", choice(RUNTIMES)),
            ("permissions", object(permission_props())),
            (
                "delegation",
                object(vec![
                    ("concurrency", int(Some(1))),
                    ("depth", int(Some(1))),
                    ("idle_timeout", pattern(Pat::Duration)),
                ]),
            ),
            (
                "shell_timeout",
                object(vec![
                    ("default", pattern(Pat::Duration)),
                    ("max", pattern(Pat::Duration)),
                ]),
            ),
            ("isolation_images", map_of(non_empty())),
            ("isolation_base", non_empty()),
            ("isolation_devcontainer_service", non_empty()),
            ("isolation_memory_limit", limit(Pat::MemoryLimit)),
            ("isolation_pids_limit", limit(Pat::PidsLimit)),
            ("output_dir", non_empty()),
            ("isolation_engines", list(non_empty())),
            (
                "companions",
                Node::Array {
                    items: Box::new(Node::Str {
                        choices: &[],
                        pattern: Some(Pat::Companion),
                        min_len: true,
                    }),
                    unique: true,
                },
            ),
            ("agents", map_of(binding())),
            ("default_agent", string()),
            (
                "ui",
                object(vec![
                    ("prefix_key", pattern(Pat::PrefixKey)),
                    ("list_window", pattern(Pat::ListWindow)),
                ]),
            ),
            ("session_reap_age", pattern(Pat::Age)),
            ("session_purge_age", pattern(Pat::Age)),
            ("session_transcript_retention", pattern(Pat::Age)),
            // plan supersedes Go: D28, json and text only
            ("non_tty_format", choice(&["json", "text"])),
            ("auth", choice(&["login", "token"])),
        ],
        extra: Extra::Deny,
    }
}

/// An `agents.<name>` binding.
fn binding() -> Node {
    Node::Object {
        props: vec![
            ("llm", string()),
            ("profiles", list(non_empty())),
            ("runtime", choice(RUNTIMES)),
            (
                "permissions",
                Node::Object {
                    props: permission_props(),
                    extra: Extra::Each(Box::new(Node::Object {
                        props: Vec::new(),
                        extra: Extra::Any,
                    })),
                },
            ),
            (
                "may_delegate",
                Node::Array {
                    items: Box::new(non_empty()),
                    unique: true,
                },
            ),
            ("driving", choice(&["conversational", "oneshot"])),
            ("surfaces", map_of(string())),
            ("roots", map_of(string())),
            ("engine_home", choice(&["host", "session"])),
            ("env_host", Node::Bool),
            (
                "env",
                list(Node::Str {
                    choices: &[],
                    pattern: Some(Pat::NoEquals),
                    min_len: false,
                }),
            ),
        ],
        extra: Extra::Deny,
    }
}

fn schema() -> &'static Node {
    static ROOT: OnceLock<Node> = OnceLock::new();
    ROOT.get_or_init(root)
}

/// The type a scalar's text reads as, as the schema's validator names it.
fn type_of_scalar(text: &str) -> &'static str {
    if text.parse::<i64>().is_ok() || text.parse::<f64>().is_ok_and(f64::is_finite) {
        "number"
    } else if text == "true" || text == "false" {
        "boolean"
    } else {
        "string"
    }
}

/// What a value is, as the schema's validator names it.
fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Scalar(text) => type_of_scalar(text),
        Value::Seq(_) => "array",
        Value::Map(_) => "object",
    }
}

fn dotted(path: &[String]) -> String {
    path.join(".")
}

fn push(path: &[String], key: &str) -> Vec<String> {
    let mut next = path.to_vec();
    next.push(key.to_string());
    next
}

/// The keys any shape of `node` lists, sorted.
fn keys_of(node: &Node) -> Vec<String> {
    let mut keys: Vec<String> = match node {
        Node::Object { props, .. } => props.iter().map(|(k, _)| (*k).to_string()).collect(),
        Node::Llm => LLM_TYPES
            .iter()
            .flat_map(|kind| llm_props(kind))
            .map(|(k, _)| k.to_string())
            .collect(),
        _ => Vec::new(),
    };
    keys.sort();
    keys.dedup();
    keys
}

/// The node under `key` of `node`, if the schema has one: a property, or the
/// shape every label of a map takes.
fn child<'a>(node: &'a Node, key: &str) -> Option<std::borrow::Cow<'a, Node>> {
    match node {
        Node::Object { props, extra } => props
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, n)| std::borrow::Cow::Borrowed(n))
            .or_else(|| match extra {
                Extra::Each(each) => Some(std::borrow::Cow::Borrowed(each.as_ref())),
                Extra::Any => Some(std::borrow::Cow::Owned(Node::Free)),
                Extra::Deny => None,
            }),
        Node::Llm => LLM_TYPES.iter().find_map(|kind| {
            llm_props(kind)
                .into_iter()
                .find(|(k, _)| *k == key)
                .map(|(_, n)| std::borrow::Cow::Owned(n))
        }),
        Node::Array { items, .. } if key.parse::<usize>().is_ok() => {
            Some(std::borrow::Cow::Borrowed(items.as_ref()))
        }
        Node::Free => Some(std::borrow::Cow::Owned(Node::Free)),
        _ => None,
    }
}

/// The node at `path`, if the schema describes it.
fn node_at(path: &[String]) -> Option<std::borrow::Cow<'static, Node>> {
    let mut here = std::borrow::Cow::Borrowed(schema());
    for key in path {
        here = match here {
            std::borrow::Cow::Borrowed(node) => match child(node, key)? {
                std::borrow::Cow::Borrowed(b) => std::borrow::Cow::Borrowed(b),
                std::borrow::Cow::Owned(o) => std::borrow::Cow::Owned(o),
            },
            std::borrow::Cow::Owned(node) => {
                std::borrow::Cow::Owned(child(&node, key)?.into_owned())
            }
        };
    }
    Some(here)
}

/// Every violation of `doc`.
pub fn validate(doc: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    check(schema(), doc, &mut Vec::new(), &mut out);
    out
}

/// `value` against `node`, its place `path`.
fn check(node: &Node, value: &Value, path: &mut Vec<String>, out: &mut Vec<Violation>) {
    match node {
        Node::Free => {}
        Node::Str {
            choices,
            pattern,
            min_len,
        } => check_str(value, choices, *pattern, *min_len, path, out),
        Node::Int { min } => match value {
            Value::Scalar(text) if text.parse::<i64>().is_ok() => {
                let n: i64 = text.parse().unwrap_or(0);
                if let Some(min) = min.filter(|min| n < *min) {
                    out.push(Violation::Breaks {
                        path: dotted(path),
                        detail: format!("must be >= {min} but found {n}"),
                    });
                }
            }
            other => out.push(holds(path, other, "integer")),
        },
        Node::Bool => match value {
            Value::Scalar(text) if text == "true" || text == "false" => {}
            other => out.push(holds(path, other, "boolean")),
        },
        Node::Array { items, unique } => check_array(items, *unique, value, path, out),
        Node::Object { props, extra } => check_object(node, props, extra, value, path, out),
        Node::AnyOf(alts) => {
            let ok = alts.iter().any(|alt| {
                let mut found = Vec::new();
                check(alt, value, &mut path.clone(), &mut found);
                found.is_empty()
            });
            if !ok {
                out.push(none_of_the_shapes(path));
            }
        }
        Node::Llm => check_llm(value, path, out),
    }
}

fn holds(path: &[String], value: &Value, expected: &str) -> Violation {
    Violation::Holds {
        path: dotted(path),
        holds: type_of(value).to_string(),
        expected: expected.to_string(),
    }
}

fn none_of_the_shapes(path: &[String]) -> Violation {
    Violation::Breaks {
        path: dotted(path),
        detail: "matches none of the shapes the schema allows here".to_string(),
    }
}

fn check_str(
    value: &Value,
    choices: &[&str],
    pattern: Option<Pat>,
    min_len: bool,
    path: &[String],
    out: &mut Vec<Violation>,
) {
    let Value::Scalar(text) = value else {
        out.push(holds(path, value, "string"));
        return;
    };
    if !choices.is_empty() && !choices.contains(&text.as_str()) {
        let list: Vec<String> = choices.iter().map(|c| format!("{c:?}")).collect();
        out.push(Violation::Breaks {
            path: dotted(path),
            detail: format!("value must be one of {}", list.join(", ")),
        });
    } else if let Some(p) = pattern.filter(|p| !p.matches(text)) {
        out.push(Violation::Breaks {
            path: dotted(path),
            detail: format!("does not match pattern '{}'", p.source()),
        });
    } else if min_len && text.is_empty() {
        out.push(Violation::Breaks {
            path: dotted(path),
            detail: "length must be >= 1, but got 0".to_string(),
        });
    }
}

fn check_array(
    items: &Node,
    unique: bool,
    value: &Value,
    path: &mut Vec<String>,
    out: &mut Vec<Violation>,
) {
    let Value::Seq(entries) = value else {
        out.push(holds(path, value, "array"));
        return;
    };
    for (i, entry) in entries.iter().enumerate() {
        path.push(i.to_string());
        check(items, entry, path, out);
        path.pop();
    }
    if unique {
        for (j, entry) in entries.iter().enumerate() {
            if let Some(i) = entries[..j].iter().position(|earlier| earlier == entry) {
                out.push(Violation::Breaks {
                    path: dotted(path),
                    detail: format!("items at index {i} and {j} are equal"),
                });
                break;
            }
        }
    }
}

fn check_object(
    node: &Node,
    props: &[(&'static str, Node)],
    extra: &Extra,
    value: &Value,
    path: &mut Vec<String>,
    out: &mut Vec<Violation>,
) {
    let Value::Map(entries) = value else {
        out.push(holds(path, value, "object"));
        return;
    };
    for (key, entry) in entries {
        let here = props.iter().find(|(k, _)| k == key).map(|(_, n)| n);
        path.push(key.clone());
        match (here, extra) {
            (Some(prop), _) => check(prop, entry, path, out),
            (None, Extra::Each(each)) => check(each, entry, path, out),
            (None, Extra::Any) => {}
            (None, Extra::Deny) => {
                path.pop();
                out.push(Violation::UnknownKey {
                    path: dotted(&push(path, key)),
                    site: site(path),
                    known: keys_of(node),
                });
                path.push(key.clone());
            }
        }
        path.pop();
    }
}

/// Where a key would have been legal, as the message names it.
fn site(path: &[String]) -> String {
    if path.is_empty() {
        "the top level".to_string()
    } else {
        format!("`{}`", dotted(path))
    }
}

/// An `llm.configs.<label>` entry: the shape its `type` names, or none.
fn check_llm(value: &Value, path: &mut Vec<String>, out: &mut Vec<Violation>) {
    let Value::Map(entries) = value else {
        out.push(holds(path, value, "object"));
        return;
    };
    let kind = value.get("type").and_then(Value::as_str);
    let Some(kind) = kind.filter(|k| LLM_TYPES.contains(k)) else {
        // No shape is named: a key no shape has is still unknown; otherwise
        // the entry is the defect.
        let before = out.len();
        for (key, _) in entries {
            if !keys_of(&Node::Llm).contains(key) {
                out.push(Violation::UnknownKey {
                    path: dotted(&push(path, key)),
                    site: site(path),
                    known: keys_of(&Node::Llm),
                });
            }
        }
        if out.len() == before {
            out.push(none_of_the_shapes(path));
        }
        return;
    };
    let props = llm_props(kind);
    let mut elsewhere = false;
    let before = out.len();
    for (key, entry) in entries {
        match props.iter().find(|(k, _)| k == key) {
            Some((_, prop)) => {
                path.push(key.clone());
                check(prop, entry, path, out);
                path.pop();
            }
            None if keys_of(&Node::Llm).contains(key) => elsewhere = true,
            None => out.push(Violation::UnknownKey {
                path: dotted(&push(path, key)),
                site: site(path),
                known: keys_of(&Node::Llm),
            }),
        }
    }
    check_llm_requirements(kind, value, path, out);
    if elsewhere && out.len() == before {
        out.push(none_of_the_shapes(path));
    }
}

/// The `required` and `dependentRequired` of an `llm.configs` shape.
fn check_llm_requirements(kind: &str, value: &Value, path: &[String], out: &mut Vec<Violation>) {
    if kind != "opencode" {
        return;
    }
    let missing: Vec<&str> = ["type", "model"]
        .into_iter()
        .filter(|k| value.get(k).is_none())
        .collect();
    if !missing.is_empty() {
        let quoted: Vec<String> = missing.iter().map(|k| format!("'{k}'")).collect();
        out.push(Violation::Breaks {
            path: dotted(path),
            detail: format!("missing properties: {}", quoted.join(", ")),
        });
    }
    if let Some(provider) = value.get("provider") {
        if value.get("context_window").is_none() {
            out.push(Violation::Breaks {
                path: dotted(path),
                detail: "properties 'context_window' required, if 'provider' property exists"
                    .to_string(),
            });
        }
        let missing: Vec<String> = ["base_url", "output_limit"]
            .into_iter()
            .filter(|k| provider.get(k).is_none())
            .map(|k| format!("'{k}'"))
            .collect();
        if !missing.is_empty() {
            out.push(Violation::Breaks {
                path: dotted(&push(path, "provider")),
                detail: format!("missing properties: {}", missing.join(", ")),
            });
        }
    }
}

/// Whether the schema describes `path` (a key, or a free-form label under a
/// map such as `agents.<name>.runtime`).
pub fn known_path(path: &[String]) -> bool {
    node_at(path).is_some()
}

/// Whether `value` is one the schema allows at `path`.
pub fn validate_at(path: &[String], value: &Value) -> Result<(), String> {
    let Some(node) = node_at(path) else {
        return Ok(());
    };
    let mut found = Vec::new();
    check(&node, value, &mut path.to_vec(), &mut found);
    match found.first() {
        None => Ok(()),
        Some(Violation::UnknownKey { path, .. }) => Err(format!("unknown key `{path}`")),
        Some(Violation::Holds {
            holds, expected, ..
        }) => Err(format!("holds a {holds}, expected {expected}")),
        Some(Violation::Breaks { detail, .. }) => Err(detail.clone()),
    }
}

/// The type of the scalars the schema puts at `path`, for output.
pub(crate) fn scalar_kind(path: &[String]) -> ScalarKind {
    match node_at(path).as_deref() {
        Some(Node::Int { .. }) => ScalarKind::Int,
        Some(Node::Bool) => ScalarKind::Bool,
        Some(Node::Free) => ScalarKind::Inferred,
        _ => ScalarKind::Str,
    }
}

/// How a scalar is printed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScalarKind {
    Int,
    Bool,
    Str,
    /// Not in the schema: numbers and booleans read as what they look like.
    Inferred,
}

/// The keys of a mapping at `path` in the order its record lists them, then
/// the rest sorted; `None` for a position the schema lists nothing at.
pub(crate) fn field_order(path: &[String]) -> Vec<String> {
    match node_at(path).as_deref() {
        Some(Node::Object { props, .. }) => props.iter().map(|(k, _)| (*k).to_string()).collect(),
        Some(Node::Llm) => llm_props("claude-code")
            .into_iter()
            .map(|(k, _)| k.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// The top-level keys, in the order a section is named: `config show nonsense`
/// lists them.
pub const SECTIONS: [&str; 26] = [
    "schema_version",
    "llm",
    "editor",
    "config",
    "sync",
    "agents",
    "default_agent",
    "workspace",
    "runtime",
    "permissions",
    "delegation",
    "shell_timeout",
    "isolation_images",
    "isolation_base",
    "isolation_devcontainer_service",
    "isolation_engines",
    "isolation_memory_limit",
    "isolation_pids_limit",
    "companions",
    "output_dir",
    "ui",
    "session_reap_age",
    "session_purge_age",
    "session_transcript_retention",
    "non_tty_format",
    "auth",
];

#[cfg(test)]
#[path = "schema.test.rs"]
mod tests;
