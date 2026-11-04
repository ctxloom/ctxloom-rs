//! A JSON value that keeps what it read: numbers as the digits they were
//! written in (no float can hold `18446744073709551615` or `-9223372036854775808`
//! exactly, and a file the user wrote by hand comes back with its digits) and
//! objects in the order they were written. Go's hew edits the file's bytes in
//! place; this reads and writes the document, so what it promises is the
//! digits and the order, not the whitespace.

use std::fmt::Write;

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
        let mut reader = Reader {
            bytes: text.as_bytes(),
            at: 0,
        };
        reader.skip();
        let value = reader.value(0)?;
        reader.skip();
        if reader.at != reader.bytes.len() {
            return Err(format!("unexpected text at byte {}", reader.at));
        }
        Ok(value)
    }

    /// The document, indented by two spaces as Go's `json.MarshalIndent` does,
    /// without a trailing newline.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => out.push_str(n),
            Json::String(s) => quote(out, s),
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Array(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    indent(out, depth + 1);
                    item.write(out, depth + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                indent(out, depth);
                out.push(']');
            }
            Json::Object(members) if members.is_empty() => out.push_str("{}"),
            Json::Object(members) => {
                out.push_str("{\n");
                for (i, (key, value)) in members.iter().enumerate() {
                    indent(out, depth + 1);
                    quote(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                    out.push_str(if i + 1 < members.len() { ",\n" } else { "\n" });
                }
                indent(out, depth);
                out.push('}');
            }
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn quote(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Deep enough for any file a person writes, shallow enough for the stack.
const DEPTH: usize = 128;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("expected {:?} at byte {}", byte as char, self.at))
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(format!("unexpected text at byte {}", self.at))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > DEPTH {
            return Err("nested too deeply".to_string());
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(format!("unexpected text at byte {}", self.at)),
            None => Err("unexpected end of the document".to_string()),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.at += 1;
        }
        let digits = String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned();
        if digits.parse::<f64>().is_err() {
            return Err(format!("{digits:?} is not a number"));
        }
        Ok(Json::Number(digits))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err("a string is not closed".to_string());
            };
            self.at += 1;
            match byte {
                b'"' => break,
                b'\\' => self.escape(&mut out)?,
                0..=0x1f => {
                    return Err(format!(
                        "a control character in a string at byte {}",
                        self.at
                    ))
                }
                other => out.push(other),
            }
        }
        String::from_utf8(out).map_err(|e| e.to_string())
    }

    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let Some(byte) = self.peek() else {
            return Err("a string is not closed".to_string());
        };
        self.at += 1;
        let plain = match byte {
            b'"' => b'"',
            b'\\' => b'\\',
            b'/' => b'/',
            b'b' => 8,
            b'f' => 12,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'u' => {
                let c = self.unicode()?;
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                return Ok(());
            }
            other => return Err(format!("bad escape \\{}", other as char)),
        };
        out.push(plain);
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let end = self.at + 4;
        let digits = self
            .bytes
            .get(self.at..end)
            .and_then(|b| std::str::from_utf8(b).ok())
            .and_then(|s| u32::from_str_radix(s, 16).ok())
            .ok_or_else(|| format!("bad \\u escape at byte {}", self.at))?;
        self.at = end;
        Ok(digits)
    }

    fn unicode(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&first) {
            if self.bytes.get(self.at..self.at + 2) != Some(b"\\u") {
                return Err("a lone surrogate in a string".to_string());
            }
            self.at += 2;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err("a lone surrogate in a string".to_string());
            }
            0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00)
        } else {
            first
        };
        char::from_u32(code).ok_or_else(|| "a lone surrogate in a string".to_string())
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            self.skip();
            items.push(self.value(depth + 1)?);
            self.skip();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(format!("expected , or ] at byte {}", self.at)),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut members: Vec<(String, Json)> = Vec::new();
        self.skip();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.skip();
            let key = self.string()?;
            self.skip();
            self.expect(b':')?;
            self.skip();
            let value = self.value(depth + 1)?;
            // A key written twice: the last wins, in the first's place, as
            // Go's decoder reads it.
            match members.iter_mut().find(|(k, _)| *k == key) {
                Some((_, held)) => *held = value,
                None => members.push((key, value)),
            }
            self.skip();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object(members));
                }
                _ => return Err(format!("expected , or }} at byte {}", self.at)),
            }
        }
    }
}

#[cfg(test)]
#[path = "json.test.rs"]
mod tests;
