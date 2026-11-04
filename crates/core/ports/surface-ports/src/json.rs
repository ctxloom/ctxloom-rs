//! A JSON value that keeps what it read: numbers as the digits they were
//! written in (no float can hold `18446744073709551615` or `-9223372036854775808`
//! exactly, and a file the user wrote by hand comes back with its digits) and
//! objects in the order they were written. Go's hew edits the file's bytes in
//! place; this reads and writes the document, so what it promises is the
//! digits and the order, not the whitespace. The reading and writing are
//! `serde_json`'s, with its `arbitrary_precision` for the digits; it spells an
//! exponent in lowercase with its sign (`1E5` is kept as `1e+5`).

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Number;

/// A JSON value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    /// The number's digits, as written.
    Number(String),
    String(String),
    Array(Vec<Json>),
    /// Members in the order they were written.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A string value.
    pub fn text(value: impl Into<String>) -> Json {
        Json::String(value.into())
    }

    /// An object from members.
    pub fn object<K: Into<String>>(members: impl IntoIterator<Item = (K, Json)>) -> Json {
        Json::Object(members.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// An array of strings.
    pub fn strings<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> Json {
        Json::Array(
            items
                .into_iter()
                .map(|s| Json::String(s.as_ref().to_string()))
                .collect(),
        )
    }

    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The member `key` of an object, to change.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Json> {
        match self {
            Json::Object(members) => members.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The text of a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    /// The elements of an array.
    pub fn as_array(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Set the member `key` of an object (replacing it in place, or adding it
    /// last); false for a value that is not an object.
    pub fn set(&mut self, key: &str, value: Json) -> bool {
        match self {
            Json::Object(members) => {
                match members.iter_mut().find(|(k, _)| k == key) {
                    Some((_, v)) => *v = value,
                    None => members.push((key.to_string(), value)),
                }
                true
            }
            _ => false,
        }
    }

    /// Take the member `key` out of an object.
    pub fn remove(&mut self, key: &str) -> Option<Json> {
        match self {
            Json::Object(members) => {
                let at = members.iter().position(|(k, _)| k == key)?;
                Some(members.remove(at).1)
            }
            _ => None,
        }
    }

    /// Read one document; the whole text must be it.
    pub fn parse(text: &str) -> Result<Json, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The document, indented by two spaces as Go's `json.MarshalIndent` does,
    /// without a trailing newline.
    pub fn pretty(&self) -> String {
        // Writing a `Json` to a string cannot fail: a number that is not one
        // is written as null, and nothing else here can be refused.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(b) => serializer.serialize_bool(*b),
            Json::Number(digits) => match digits.parse::<Number>() {
                Ok(number) => number.serialize(serializer),
                Err(_) => serializer.serialize_unit(),
            },
            Json::String(s) => serializer.serialize_str(s),
            Json::Array(items) => serializer.collect_seq(items),
            Json::Object(members) => serializer.collect_map(members.iter().map(|(k, v)| (k, v))),
        }
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(Reader)
    }
}

/// With `arbitrary_precision`, serde_json hands a number to a visitor as a
/// one-member map under this key, its digits as the value.
const NUMBER_KEY: &str = "$serde_json::private::Number";

struct Reader;

impl<'de> Visitor<'de> for Reader {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E: de::Error>(self, b: bool) -> Result<Json, E> {
        Ok(Json::Bool(b))
    }

    fn visit_u64<E: de::Error>(self, n: u64) -> Result<Json, E> {
        Ok(Json::Number(n.to_string()))
    }

    fn visit_i64<E: de::Error>(self, n: i64) -> Result<Json, E> {
        Ok(Json::Number(n.to_string()))
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Json, E> {
        Ok(Json::String(s.to_string()))
    }

    fn visit_string<E: de::Error>(self, s: String) -> Result<Json, E> {
        Ok(Json::String(s))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut members: Vec<(String, Json)> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value()?;
            // A key written twice: the last wins, in the first's place, as
            // Go's decoder reads it.
            match members.iter_mut().find(|(k, _)| *k == key) {
                Some((_, held)) => *held = value,
                None => members.push((key, value)),
            }
        }
        match members.as_mut_slice() {
            [(key, Json::String(digits))] if key == NUMBER_KEY => {
                Ok(Json::Number(std::mem::take(digits)))
            }
            _ => Ok(Json::Object(members)),
        }
    }
}

#[cfg(test)]
#[path = "json.test.rs"]
mod tests;
