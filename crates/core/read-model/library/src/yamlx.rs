//! Small readers over the parsed YAML tree (`angzarr_yaml::Value`) the
//! decoders share: scalars as text, lists, maps, booleans and integers, the
//! "did you mean" key matcher (Go's `keymatch.Nearest`) and the line a key sits
//! on (the parsed tree keeps no line numbers).

use std::collections::BTreeMap;

use angzarr_yaml::Value;

/// Parse YAML text. A document start (`---`, the first thing in the text) is
/// the one marker Go's reader takes in stride; the line is blanked so that
/// every line keeps its number.
pub(crate) fn parse(text: &str) -> Result<Value, angzarr_yaml::Error> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let first = lines.iter().position(|line| {
        let t = line.trim();
        !t.is_empty() && !t.starts_with('#')
    });
    if let Some(i) = first.filter(|i| lines[*i].trim_end() == "---") {
        lines[i] = "";
        return angzarr_yaml::parse(&lines.join("\n"));
    }
    angzarr_yaml::parse(text)
}

/// The scalar's text, or empty for anything else (a null, a collection).
pub(crate) fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

/// A list of strings: a null is none, a sequence of scalars is its entries (a
/// null entry is the empty string, as Go's decode reads it), anything else is
/// refused with what was found.
pub(crate) fn string_list(value: &Value) -> Result<Vec<String>, String> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Seq(items) => items
            .iter()
            .map(|item| match item {
                Value::Scalar(s) => Ok(s.clone()),
                Value::Null => Ok(String::new()),
                other => Err(format!("expected a string, found {}", kind(other))),
            })
            .collect(),
        other => Err(format!("expected a list, found {}", kind(other))),
    }
}

/// A map of strings to strings: a null is none.
pub(crate) fn string_map(value: &Value) -> Result<BTreeMap<String, String>, String> {
    match value {
        Value::Null => Ok(BTreeMap::new()),
        Value::Map(entries) => entries
            .iter()
            .map(|(k, v)| match v {
                Value::Scalar(s) => Ok((k.clone(), s.clone())),
                Value::Null => Ok((k.clone(), String::new())),
                other => Err(format!("{k}: expected a string, found {}", kind(other))),
            })
            .collect(),
        other => Err(format!("expected a mapping, found {}", kind(other))),
    }
}

/// A string: a null is empty, a collection is refused.
pub(crate) fn string(value: &Value) -> Result<String, String> {
    match value {
        Value::Null => Ok(String::new()),
        Value::Scalar(s) => Ok(s.clone()),
        other => Err(format!("expected a string, found {}", kind(other))),
    }
}

/// A boolean, YAML 1.1 spellings included (`yes`, `on`), as Go's decode reads
/// one into a `bool`; a null is false.
pub(crate) fn boolean(value: &Value) -> Result<bool, String> {
    match value {
        Value::Null => Ok(false),
        Value::Scalar(s) => match s.to_lowercase().as_str() {
            "true" | "yes" | "on" | "y" => Ok(true),
            "false" | "no" | "off" | "n" => Ok(false),
            _ => Err(format!("expected a boolean, found \"{s}\"")),
        },
        other => Err(format!("expected a boolean, found {}", kind(other))),
    }
}

/// An integer; a null is zero.
pub(crate) fn integer(value: &Value) -> Result<i64, String> {
    match value {
        Value::Null => Ok(0),
        Value::Scalar(s) => s
            .trim()
            .replace('_', "")
            .parse::<i64>()
            .map_err(|_| format!("expected an integer, found \"{s}\"")),
        other => Err(format!("expected an integer, found {}", kind(other))),
    }
}

/// What kind of value `value` is, for a refusal.
pub(crate) fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "nothing",
        Value::Scalar(_) => "a scalar",
        Value::Seq(_) => "a list",
        Value::Map(_) => "a mapping",
    }
}

/// Levenshtein distance over characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// The known key `key` was most likely meant to be (Go's `keymatch.Nearest`):
/// the first of the nearest, within an edit budget of a third of the key's
/// length (at least one, at most two); empty when none is near enough.
pub(crate) fn nearest(key: &str, known: &[&str]) -> String {
    let budget = (key.chars().count() / 3).clamp(1, 2);
    let mut best = ("", usize::MAX);
    for candidate in known {
        let distance = edit_distance(key, candidate);
        if distance < best.1 {
            best = (candidate, distance);
        }
    }
    if best.1 > budget {
        String::new()
    } else {
        best.0.to_string()
    }
}

/// How many leading spaces `line` has.
fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Whether `line` (already trimmed of indentation, a leading `- ` removed)
/// is the entry of key `key`.
fn is_key_line(trimmed: &str, key: &str) -> bool {
    let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim_start();
    for quote in ["", "\"", "'"] {
        let head = format!("{quote}{key}{quote}:");
        if let Some(rest) = trimmed.strip_prefix(&head) {
            if rest.is_empty() || rest.starts_with(' ') {
                return true;
            }
        }
    }
    false
}

/// The 1-based line of the key at `path` in `source` (a segment `[]` steps
/// into a list's items), as near as the text lets one tell; the last segment
/// found when a deeper one is not, and 1 when none is.
pub(crate) fn key_line(source: &str, path: &[&str]) -> u64 {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = 0;
    let mut from = 0;
    let mut parent_indent: Option<usize> = None;
    for segment in path.iter().filter(|s| **s != "[]") {
        let mut hit = None;
        for (i, line) in lines.iter().enumerate().skip(from) {
            let trimmed = line.trim_start_matches(' ');
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let indent = indent_of(line);
            if parent_indent.is_some_and(|p| indent <= p) {
                break;
            }
            let top_level_only = parent_indent.is_none() && indent != 0;
            if !top_level_only && is_key_line(trimmed, segment) {
                hit = Some((i, indent));
                break;
            }
        }
        let Some((i, indent)) = hit else { break };
        found = i + 1;
        from = i + 1;
        parent_indent = Some(indent);
    }
    u64::try_from(found.max(1)).unwrap_or(1)
}

#[cfg(test)]
#[path = "yamlx.test.rs"]
mod tests;
