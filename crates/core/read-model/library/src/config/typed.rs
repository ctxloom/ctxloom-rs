//! A config as the types it holds, for output: the reader keeps every scalar
//! as its text, and the schema says which are numbers, booleans and strings.

use angzarr_yaml::Value;

use super::schema::{self, ScalarKind};

/// A value with its type.
#[derive(Clone, PartialEq, Debug)]
pub enum Typed {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Seq(Vec<Typed>),
    /// Entries in the order to print them.
    Map(Vec<(String, Typed)>),
}

/// How a mapping's entries are ordered when printed as YAML.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    /// Sorted by key at every depth (the whole document).
    Sorted,
    /// A section in the order of the fields of its record (Go's struct
    /// order), user-named keys sorted.
    Fields,
}

/// `value`, the document's section at `path` (empty for the whole document),
/// given the types the schema says its scalars hold.
pub fn typed(value: &Value, path: &[String], order: Order) -> Typed {
    let mut here = value;
    for key in path {
        match here.get(key) {
            Some(next) => here = next,
            None => return Typed::Map(Vec::new()),
        }
    }
    convert(here, &mut path.to_vec(), order)
}

fn convert(value: &Value, path: &mut Vec<String>, order: Order) -> Typed {
    match value {
        Value::Null => Typed::Null,
        Value::Scalar(text) => scalar(text, path),
        Value::Seq(items) => Typed::Seq(
            items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    path.push(i.to_string());
                    let out = convert(item, path, order);
                    path.pop();
                    out
                })
                .collect(),
        ),
        Value::Map(entries) => {
            let mut keys: Vec<&String> = entries.iter().map(|(k, _)| k).collect();
            keys.sort();
            if order == Order::Fields {
                let fields = schema::field_order(path);
                let rank = |k: &String| fields.iter().position(|f| f == k).unwrap_or(usize::MAX);
                keys.sort_by_key(|k| rank(k));
            }
            Typed::Map(
                keys.into_iter()
                    .filter_map(|key| {
                        let (_, entry) = entries.iter().find(|(k, _)| k == key)?;
                        path.push(key.clone());
                        let out = convert(entry, path, order);
                        path.pop();
                        Some((key.clone(), out))
                    })
                    .collect(),
            )
        }
    }
}

fn scalar(text: &str, path: &[String]) -> Typed {
    match schema::scalar_kind(path) {
        ScalarKind::Int => text
            .parse()
            .map_or_else(|_| Typed::Str(text.to_string()), Typed::Int),
        ScalarKind::Bool => match text {
            "true" => Typed::Bool(true),
            "false" => Typed::Bool(false),
            _ => Typed::Str(text.to_string()),
        },
        ScalarKind::Inferred => match text {
            "true" => Typed::Bool(true),
            "false" => Typed::Bool(false),
            _ => text
                .parse()
                .map_or_else(|_| Typed::Str(text.to_string()), Typed::Int),
        },
        ScalarKind::Str => Typed::Str(text.to_string()),
    }
}

/// The YAML Go prints: two-space indentation, a list's dashes indented under
/// its key, a string that would read back as another type double-quoted
/// (`"512"`, `"true"`).
pub fn to_yaml(value: &Typed) -> String {
    let mut out = String::new();
    match value {
        Typed::Map(entries) if entries.is_empty() => out.push_str("{}\n"),
        Typed::Map(entries) => emit_map(entries, 0, &mut out),
        Typed::Seq(items) if items.is_empty() => out.push_str("[]\n"),
        Typed::Seq(items) => emit_seq(items, 0, &mut out),
        other => {
            out.push_str(&inline(other));
            out.push('\n');
        }
    }
    out
}

fn emit_map(entries: &[(String, Typed)], indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    for (key, value) in entries {
        let key = quote_if_needed(key);
        match value {
            Typed::Map(inner) if !inner.is_empty() => {
                out.push_str(&format!("{pad}{key}:\n"));
                emit_map(inner, indent + 2, out);
            }
            Typed::Seq(items) if !items.is_empty() => {
                out.push_str(&format!("{pad}{key}:\n"));
                emit_seq(items, indent + 2, out);
            }
            other => out.push_str(&format!("{pad}{key}: {}\n", inline(other))),
        }
    }
}

fn emit_seq(items: &[Typed], indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    for item in items {
        match item {
            Typed::Map(inner) if !inner.is_empty() => {
                let mut block = String::new();
                emit_map(inner, indent + 2, &mut block);
                out.push_str(&format!("{pad}- {}", &block[indent + 2..]));
            }
            Typed::Seq(inner) if !inner.is_empty() => {
                let mut block = String::new();
                emit_seq(inner, indent + 2, &mut block);
                out.push_str(&format!("{pad}- {}", &block[indent + 2..]));
            }
            other => out.push_str(&format!("{pad}- {}\n", inline(other))),
        }
    }
}

/// A scalar or an empty collection on one line.
fn inline(value: &Typed) -> String {
    match value {
        Typed::Null => "null".to_string(),
        Typed::Bool(b) => b.to_string(),
        Typed::Int(n) => n.to_string(),
        Typed::Float(f) => f.to_string(),
        Typed::Str(s) => quote_if_needed(s),
        Typed::Seq(_) => "[]".to_string(),
        Typed::Map(_) => "{}".to_string(),
    }
}

/// `text` as a YAML scalar: plain where it reads back as the same string,
/// double-quoted where it would not.
fn quote_if_needed(text: &str) -> String {
    if needs_quotes(text) {
        let escaped: String = text
            .chars()
            .flat_map(|c| match c {
                '"' => vec!['\\', '"'],
                '\\' => vec!['\\', '\\'],
                '\n' => vec!['\\', 'n'],
                '\t' => vec!['\\', 't'],
                other => vec![other],
            })
            .collect();
        format!("\"{escaped}\"")
    } else {
        text.to_string()
    }
}

fn needs_quotes(text: &str) -> bool {
    const WORDS: [&str; 22] = [
        "null", "Null", "NULL", "~", "true", "True", "TRUE", "false", "False", "FALSE", "y", "Y",
        "yes", "Yes", "YES", "n", "N", "no", "No", "NO", "on", "off",
    ];
    if text.is_empty()
        || WORDS.contains(&text)
        || text.eq_ignore_ascii_case("on")
        || text.eq_ignore_ascii_case("off")
    {
        return true;
    }
    if text.parse::<f64>().is_ok() || text.parse::<i64>().is_ok() || looks_numeric(text) {
        return true;
    }
    if text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace) {
        return true;
    }
    if text.contains(['\n', '\t', '\r'])
        || text.contains(": ")
        || text.contains(" #")
        || text.ends_with(':')
    {
        return true;
    }
    let mut chars = text.chars();
    let first = chars.next().unwrap_or(' ');
    match first {
        '-' | '?' | ':' => chars.next().is_none_or(char::is_whitespace),
        ',' | '[' | ']' | '{' | '}' | '#' | '&' | '*' | '!' | '|' | '>' | '\'' | '"' | '%'
        | '@' | '`' => true,
        _ => false,
    }
}

/// Hex, octal, infinity and not-a-number spellings a YAML reader takes as numbers.
fn looks_numeric(text: &str) -> bool {
    let t = text.trim_start_matches(['+', '-']);
    t.starts_with("0x")
        || t.starts_with("0o")
        || t.eq_ignore_ascii_case(".inf")
        || t.eq_ignore_ascii_case(".nan")
}

#[cfg(test)]
#[path = "typed.test.rs"]
mod tests;
