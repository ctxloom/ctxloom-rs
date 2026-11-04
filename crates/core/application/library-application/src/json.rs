//! Values of the configuration as JSON and as the lines Go prints them in.

use angzarr_yaml::Value;
use serde_json::{Map, Number};

/// A scalar as the value its text reads as.
fn scalar(text: &str) -> serde_json::Value {
    if text == "true" {
        return serde_json::Value::Bool(true);
    }
    if text == "false" {
        return serde_json::Value::Bool(false);
    }
    match text.parse::<i64>() {
        Ok(n) => serde_json::Value::Number(Number::from(n)),
        Err(_) => serde_json::Value::String(text.to_string()),
    }
}

fn convert(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Scalar(text) => scalar(text),
        Value::Seq(items) => serde_json::Value::Array(items.iter().map(convert).collect()),
        Value::Map(entries) => {
            let mut map = Map::new();
            for (key, inner) in entries {
                map.insert(key.clone(), convert(inner));
            }
            serde_json::Value::Object(map)
        }
    }
}

/// A `permissions` block as JSON: an object, `{}` for none.
pub fn permissions(block: &Value) -> serde_json::Value {
    match convert(block) {
        object @ serde_json::Value::Object(_) => object,
        _ => serde_json::Value::Object(Map::new()),
    }
}

/// A value on one line as Go's `fmt.Sprint` of a decoded document.
fn inline(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Scalar(text) => text.clone(),
        Value::Seq(items) => format!(
            "[{}]",
            items.iter().map(inline).collect::<Vec<_>>().join(", ")
        ),
        Value::Map(entries) => {
            let mut sorted: Vec<&(String, Value)> = entries.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            sorted
                .iter()
                .map(|(k, v)| format!("{k}={}", inline(v)))
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

/// The block on one line, for a listing: the neutral fields, then each
/// engine's block, sorted.
pub fn permissions_line(block: &Value) -> String {
    let Value::Map(entries) = block else {
        return String::new();
    };
    let mut parts = Vec::new();
    for key in ["approver", "approval_timeout", "sandbox", "network"] {
        if let Some(text) = block.get(key).and_then(Value::as_str) {
            if !text.is_empty() {
                parts.push(format!("{key}={text}"));
            }
        }
    }
    let mut engines: Vec<&(String, Value)> = entries
        .iter()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "approver" | "approval_timeout" | "sandbox" | "network"
            )
        })
        .collect();
    engines.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, doc) in engines {
        parts.push(format!("{name}={{{}}}", inline(doc)));
    }
    parts.join(", ")
}

#[cfg(test)]
#[path = "json.test.rs"]
mod tests;
