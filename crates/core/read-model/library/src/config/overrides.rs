//! `CTXLOOM_CONFIG_<PATH>` and `--config-set path=value` (Go's
//! `confload/overlay.go`).
//!
//! A name lost its case on the way (a variable's, and a shell's), so a path
//! resolves case-insensitively to a key that is already there, adopting its
//! casing; two keys that differ only in case are an error, not a guess. A
//! path no key matches resolves through the schema (underscores are ambiguous:
//! `LLM_DEFAULTS_PRIMARY` is `llm.defaults.primary`); one the schema does not
//! know is created as written, with a warning. `--config-set` keeps the case
//! it was typed in.

use angzarr_yaml::Value;

/// The prefix of an override variable.
pub const ENV_PREFIX: &str = "CTXLOOM_CONFIG_";

/// The raw overrides, before they meet a base.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Overrides {
    /// Variable name (past the prefix) to value.
    pub env: Vec<(String, Value)>,
    /// `--config-set` path to value.
    pub flags: Vec<(String, Value)>,
}

/// A value from the environment or a flag: a comma separated text is a list;
/// anything else is the text.
pub fn coerce(raw: &str) -> Value {
    let is_number = raw.parse::<i64>().is_ok();
    let is_bool = matches!(
        raw,
        "1" | "t" | "T" | "TRUE" | "true" | "True" | "0" | "f" | "F" | "FALSE" | "false" | "False"
    );
    if !is_number && !is_bool && raw.contains(',') {
        return Value::Seq(raw.split(',').map(|part| coerce(part.trim())).collect());
    }
    Value::Scalar(raw.to_string())
}

/// Collect the overrides from the environment (every variable that begins
/// [`ENV_PREFIX`]) and the `--config-set` entries. An entry with no `=` or an
/// empty path is a problem, named; the rest are read.
pub fn read(environ: &[(String, String)], config_set: &[String]) -> (Overrides, Vec<String>) {
    let mut env: Vec<(String, Value)> = environ
        .iter()
        .filter_map(|(name, value)| {
            let suffix = name.strip_prefix(ENV_PREFIX)?;
            (!suffix.is_empty()).then(|| (suffix.to_string(), coerce(value)))
        })
        .collect();
    env.sort_by(|a, b| a.0.cmp(&b.0));
    let mut problems = Vec::new();
    let mut flags: Vec<(String, Value)> = Vec::new();
    for entry in config_set {
        let Some((path, raw)) = entry.split_once('=') else {
            problems.push(format!("--config-set {entry:?}: expected <path>=<value>"));
            continue;
        };
        if path.is_empty() {
            problems.push(format!("--config-set {entry:?}: empty path before \"=\""));
            continue;
        }
        let value = coerce(raw);
        match flags.iter_mut().find(|(p, _)| p == path) {
            Some(existing) => existing.1 = value,
            None => flags.push((path.to_string(), value)),
        }
    }
    flags.sort_by(|a, b| a.0.cmp(&b.0));
    (Overrides { env, flags }, problems)
}

/// What a schema says of a value at a path: fine, or why not.
pub type Verdict = Result<(), String>;

/// What applying overrides to a base came to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Applied {
    pub value: Value,
    /// Problems that kept an override from applying (an ambiguous case, a
    /// value the schema refuses).
    pub errors: Vec<String>,
    /// Notices: an override that matches no known key and was applied anyway.
    pub warnings: Vec<String>,
}

/// Where overrides come from, for a message.
#[derive(Clone, Copy)]
struct Source {
    /// How a problem names the override: the variable, or the flag.
    display: fn(&str) -> String,
    /// What it is called in "env set ...".
    label: &'static str,
    /// Whether the path keeps the case it was typed in (a flag's does).
    keep_case: bool,
    tokenize: fn(&str) -> Vec<String>,
}

const ENV: Source = Source {
    display: |name| format!("{ENV_PREFIX}{name}"),
    label: "env",
    keep_case: false,
    tokenize: |name| name.split('_').map(ToString::to_string).collect(),
};

const FLAG: Source = Source {
    display: |path| format!("--config-set {path}"),
    label: "--config-set",
    keep_case: true,
    tokenize: |path| path.split('.').map(ToString::to_string).collect(),
};

/// Apply the environment's overrides, then the flags', to `base`.
/// `known_path` says whether the schema knows a path; `validate` checks a
/// value at a path.
pub fn apply(
    base: &Value,
    overrides: &Overrides,
    known_path: &dyn Fn(&[String]) -> bool,
    validate: &dyn Fn(&[String], &Value) -> Verdict,
) -> Applied {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut value = if matches!(base, Value::Map(_)) {
        base.clone()
    } else {
        Value::Map(Vec::new())
    };
    for (source, entries) in [(ENV, &overrides.env), (FLAG, &overrides.flags)] {
        let layer = resolve(
            &value,
            entries,
            source,
            known_path,
            validate,
            &mut errors,
            &mut warnings,
        );
        super::deep_merge(&mut value, &layer);
    }
    Applied {
        value,
        errors,
        warnings,
    }
}

/// The overrides of one source resolved against `base`, as a nested mapping.
fn resolve(
    base: &Value,
    entries: &[(String, Value)],
    source: Source,
    known_path: &dyn Fn(&[String]) -> bool,
    validate: &dyn Fn(&[String], &Value) -> Verdict,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Value {
    let mut layer = Value::Map(Vec::new());
    for (name, value) in entries {
        let tokens = (source.tokenize)(name);
        if tokens.is_empty() {
            continue;
        }
        let display = (source.display)(name);
        let path = match resolve_path(base, &tokens, &display, source.keep_case, known_path) {
            Ok((path, unknown)) => {
                if unknown {
                    warnings.push(format!(
                        "{display} does not match any known config key (resolved as {}); setting it anyway",
                        path.join(".")
                    ));
                }
                path
            }
            Err(problem) => {
                errors.push(problem);
                continue;
            }
        };
        if let Err(why) = validate(&path, value) {
            errors.push(format!(
                "{display}: {} set {:?} to a value the schema refuses: {why}",
                source.label,
                path.join(".")
            ));
        }
        set_at(&mut layer, &path, value.clone());
    }
    layer
}

/// `value` set at `path` of the mapping, the mappings on the way created.
fn set_at(root: &mut Value, path: &[String], value: Value) {
    match path {
        [] => {}
        [last] => *root.entry(last) = value,
        [first, rest @ ..] => {
            let child = root.entry(first);
            if !matches!(child, Value::Map(_)) {
                *child = Value::Map(Vec::new());
            }
            set_at(child, rest, value);
        }
    }
}

/// The path the tokens name in `base`, and whether it matches nothing known.
fn resolve_path(
    base: &Value,
    tokens: &[String],
    display: &str,
    keep_case: bool,
    known_path: &dyn Fn(&[String]) -> bool,
) -> Result<(Vec<String>, bool), String> {
    let mut resolved: Vec<String> = Vec::new();
    let mut level = Some(base);
    let mut remaining = tokens;
    while let (Some(here), false) = (level, remaining.is_empty()) {
        match descend_one_level(here, remaining) {
            Descent::Ambiguous(hits) => {
                let quoted: Vec<String> = hits.iter().map(|h| format!("{h:?}")).collect();
                return Err(format!(
                    "{display}: ambiguous override -- config has both {}; cannot tell which was meant (env/flag names lose case information)",
                    quoted.join(" and ")
                ));
            }
            Descent::None => break,
            Descent::Matched {
                key,
                next,
                consumed,
            } => {
                resolved.push(key);
                remaining = &remaining[consumed..];
                level = next;
            }
        }
    }
    if remaining.is_empty() {
        return Ok((resolved, false));
    }
    if let Some(path) = resolve_by_schema(&resolved, remaining, keep_case, known_path) {
        return Ok((path, false));
    }
    resolved.extend(remaining.iter().cloned());
    Ok((resolved, true))
}

enum Descent<'a> {
    Ambiguous(Vec<String>),
    None,
    Matched {
        key: String,
        next: Option<&'a Value>,
        consumed: usize,
    },
}

/// The widest run of tokens (joined by `_`) naming a key of `level`, ignoring
/// case.
fn descend_one_level<'a>(level: &'a Value, remaining: &[String]) -> Descent<'a> {
    let Some(entries) = level.as_map() else {
        return Descent::None;
    };
    for width in (1..=remaining.len()).rev() {
        let candidate = remaining[..width].join("_").to_lowercase();
        let mut hits: Vec<&(String, Value)> = entries
            .iter()
            .filter(|(k, _)| k.to_lowercase() == candidate)
            .collect();
        if hits.len() > 1 {
            let mut names: Vec<String> = hits.iter().map(|(k, _)| k.clone()).collect();
            names.sort();
            return Descent::Ambiguous(names);
        }
        if let Some((key, value)) = hits.pop() {
            return Descent::Matched {
                key: key.clone(),
                next: matches!(value, Value::Map(_)).then_some(value),
                consumed: width,
            };
        }
    }
    Descent::None
}

/// The most longest-key reading of the remaining tokens the schema knows.
fn resolve_by_schema(
    resolved: &[String],
    remaining: &[String],
    keep_case: bool,
    known_path: &dyn Fn(&[String]) -> bool,
) -> Option<Vec<String>> {
    for groups in partitions(remaining) {
        let build = |lower: bool| -> Vec<String> {
            let mut path = resolved.to_vec();
            for group in &groups {
                let joined = group.join("_");
                path.push(if lower { joined.to_lowercase() } else { joined });
            }
            path
        };
        if keep_case {
            let original = build(false);
            if known_path(&original) {
                return Some(original);
            }
        }
        let lowered = build(true);
        if known_path(&lowered) {
            return Some(lowered);
        }
    }
    None
}

/// The ways to cut `tokens` into runs, the most runs first (ties in the order
/// of their split masks).
fn partitions(tokens: &[String]) -> Vec<Vec<Vec<String>>> {
    const MAX_TOKENS: usize = 16;
    let n = tokens.len();
    if n == 0 || n > MAX_TOKENS {
        return Vec::new();
    }
    let gaps = n - 1;
    let mut masks: Vec<(usize, u32)> = (0..(1usize << gaps)).map(|m| (m, m.count_ones())).collect();
    masks.sort_by_key(|&(_, ones)| std::cmp::Reverse(ones));
    masks
        .into_iter()
        .map(|(mask, _)| {
            let mut groups = Vec::new();
            let mut start = 0;
            for i in 0..gaps {
                if mask & (1 << i) != 0 {
                    groups.push(tokens[start..=i].to_vec());
                    start = i + 1;
                }
            }
            groups.push(tokens[start..].to_vec());
            groups
        })
        .collect()
}

#[cfg(test)]
#[path = "overrides.test.rs"]
mod tests;
