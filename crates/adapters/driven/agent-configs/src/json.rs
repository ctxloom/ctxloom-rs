//! JSON read with every member's byte span, and edited by splicing bytes,
//! so a file someone else owns keeps every byte the edit does not touch.
//!
//! The splices are a port of hew's JSON binding (`ext/json/edits.go`,
//! `internal/hewsplice`), the byte-preserving patcher Go taskloom writes
//! through: a member appended to an object takes its siblings' layout (a
//! line of its own at their indent in a block, `, ` after them on one
//! line), an empty object is seeded as hew seeds it, and a removal takes
//! exactly one adjoining comma and the member's own leading gap. A value
//! written is rendered in hew's compact house style (`{ "k": v }`,
//! `[a, b]`).

use std::collections::BTreeMap;
use std::fmt;

/// A JSON value as read: its kind and its bytes `[start, end)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Object(Vec<Member>),
    Array(Vec<Element>),
    /// A string, decoded.
    String(String),
    Number,
    Bool,
    Null,
}

/// One member of an object: its key decoded, where the key starts, its
/// value, and its own trailing comma's position, if it has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub key: String,
    pub key_start: usize,
    pub value: Node,
    pub comma: Option<usize>,
}

/// One element of an array, and its trailing comma's position.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub value: Node,
    pub comma: Option<usize>,
}

impl Node {
    /// The member under `key`, the first if the object repeats it.
    pub fn member(&self, key: &str) -> Option<(usize, &Member)> {
        match &self.kind {
            Kind::Object(members) => members.iter().enumerate().find(|(_, m)| m.key == key),
            _ => None,
        }
    }

    pub fn members(&self) -> Option<&[Member]> {
        match &self.kind {
            Kind::Object(members) => Some(members),
            _ => None,
        }
    }
}

/// Why bytes are not JSON: what was found where, by line and column (from
/// 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    pub line: usize,
    pub column: usize,
    pub what: String,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.what, self.line, self.column
        )
    }
}

/// `src` as one JSON document: one value, and nothing after it but
/// whitespace.
pub fn parse(src: &[u8]) -> Result<Node, SyntaxError> {
    let mut parser = Parser { src, at: 0 };
    let node = parser.document()?;
    parser.space();
    if parser.at < src.len() {
        return Err(parser.error("unexpected content after the document"));
    }
    Ok(node)
}

/// The first JSON value in `src`, whatever follows it, as Go's decoder
/// reads a config (`json.Decoder.Decode`); `None` for bytes that are only
/// whitespace, which Go reads as an empty object.
pub fn parse_first(src: &[u8]) -> Result<Option<Node>, SyntaxError> {
    let mut parser = Parser { src, at: 0 };
    parser.space();
    if parser.at == src.len() {
        return Ok(None);
    }
    parser.document().map(Some)
}

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn document(&mut self) -> Result<Node, SyntaxError> {
        self.space();
        self.value(0)
    }

    fn error(&self, what: &str) -> SyntaxError {
        let before = &self.src[..self.at.min(self.src.len())];
        let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
        let column = self.at
            - before
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |i| i + 1)
            + 1;
        SyntaxError {
            line,
            column,
            what: what.to_string(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }

    fn space(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.at += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Node, SyntaxError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting too deep"));
        }
        let start = self.at;
        let kind = match self.peek() {
            Some(b'{') => self.object(depth)?,
            Some(b'[') => self.array(depth)?,
            Some(b'"') => Kind::String(self.string()?),
            Some(b't') => self.word(b"true", Kind::Bool)?,
            Some(b'f') => self.word(b"false", Kind::Bool)?,
            Some(b'n') => self.word(b"null", Kind::Null)?,
            Some(b'-' | b'0'..=b'9') => self.number()?,
            Some(_) => return Err(self.error("unexpected character, looking for a value")),
            None => return Err(self.error("unexpected end of input, looking for a value")),
        };
        Ok(Node {
            start,
            end: self.at,
            kind,
        })
    }

    fn word(&mut self, word: &[u8], kind: Kind) -> Result<Kind, SyntaxError> {
        if self.src[self.at..].starts_with(word) {
            self.at += word.len();
            return Ok(kind);
        }
        Err(self.error("unexpected character, looking for a value"))
    }

    fn object(&mut self, depth: usize) -> Result<Kind, SyntaxError> {
        self.at += 1;
        let mut members = Vec::new();
        self.space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Kind::Object(members));
        }
        loop {
            self.space();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a string key"));
            }
            let key_start = self.at;
            let key = self.string()?;
            self.space();
            if self.peek() != Some(b':') {
                return Err(self.error("expected ':' after a key"));
            }
            self.at += 1;
            self.space();
            let value = self.value(depth + 1)?;
            self.space();
            match self.peek() {
                Some(b',') => {
                    members.push(Member {
                        key,
                        key_start,
                        value,
                        comma: Some(self.at),
                    });
                    self.at += 1;
                }
                Some(b'}') => {
                    members.push(Member {
                        key,
                        key_start,
                        value,
                        comma: None,
                    });
                    self.at += 1;
                    return Ok(Kind::Object(members));
                }
                _ => return Err(self.error("expected ',' or '}' after a member")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Kind, SyntaxError> {
        self.at += 1;
        let mut elements = Vec::new();
        self.space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Kind::Array(elements));
        }
        loop {
            self.space();
            let value = self.value(depth + 1)?;
            self.space();
            match self.peek() {
                Some(b',') => {
                    elements.push(Element {
                        value,
                        comma: Some(self.at),
                    });
                    self.at += 1;
                }
                Some(b']') => {
                    elements.push(Element { value, comma: None });
                    self.at += 1;
                    return Ok(Kind::Array(elements));
                }
                _ => return Err(self.error("expected ',' or ']' after an element")),
            }
        }
    }

    /// A string, decoded; invalid UTF-8 in it is replaced, as Go's
    /// decoder replaces it.
    fn string(&mut self) -> Result<String, SyntaxError> {
        self.at += 1;
        let mut bytes = Vec::new();
        loop {
            match self.peek() {
                None => return Err(self.error("unexpected end of input in a string")),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(String::from_utf8_lossy(&bytes).into_owned());
                }
                Some(b'\\') => {
                    self.at += 1;
                    self.escape(&mut bytes)?;
                }
                Some(b) if b < 0x20 => return Err(self.error("control character in a string")),
                Some(b) => {
                    bytes.push(b);
                    self.at += 1;
                }
            }
        }
    }

    fn escape(&mut self, bytes: &mut Vec<u8>) -> Result<(), SyntaxError> {
        let decoded = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.at += 1;
                let unit = self.hex4()?;
                let c = if (0xD800..0xDC00).contains(&unit)
                    && self.src[self.at..].starts_with(b"\\u")
                {
                    let mark = self.at;
                    self.at += 2;
                    let low = self.hex4()?;
                    if (0xDC00..0xE000).contains(&low) {
                        char::from_u32(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))
                    } else {
                        self.at = mark;
                        None
                    }
                } else {
                    char::from_u32(unit)
                };
                let c = c.unwrap_or(char::REPLACEMENT_CHARACTER);
                bytes.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                return Ok(());
            }
            _ => return Err(self.error("invalid escape in a string")),
        };
        self.at += 1;
        bytes.extend_from_slice(decoded.encode_utf8(&mut [0; 4]).as_bytes());
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, SyntaxError> {
        let digits = self
            .src
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| {
                u32::from_str_radix(d, 16)
                    .ok()
                    .filter(|_| d.chars().all(|c| c.is_ascii_hexdigit()))
            });
        match digits {
            Some(unit) => {
                self.at += 4;
                Ok(unit)
            }
            None => Err(self.error("invalid \\u escape in a string")),
        }
    }

    fn number(&mut self) -> Result<Kind, SyntaxError> {
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        match self.peek() {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => self.digits(),
            _ => return Err(self.error("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number"));
            }
            self.digits();
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.at += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.at += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number"));
            }
            self.digits();
        }
        Ok(Kind::Number)
    }

    fn digits(&mut self) {
        while let Some(b'0'..=b'9') = self.peek() {
            self.at += 1;
        }
    }
}

/// How deep a document may nest before it is refused rather than
/// recursed into.
const MAX_DEPTH: usize = 512;

/// A value to write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    String(String),
    Array(Vec<Value>),
    /// Rendered with its keys sorted, as Go's maps marshal.
    Object(BTreeMap<String, Value>),
}

/// `value` in hew's compact house style: objects `{ "k": v, "k2": v2 }`,
/// arrays `[v, v2]`, and empty ones `{}` and `[]`.
pub fn encode(value: &Value) -> String {
    match value {
        Value::String(s) => quote(s),
        // An empty array joins to `[]` itself; only an empty object needs
        // its own arm, without the inner spaces.
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(encode).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(members) if members.is_empty() => "{}".to_string(),
        Value::Object(members) => {
            let members: Vec<String> = members
                .iter()
                .map(|(k, v)| format!("{}: {}", quote(k), encode(v)))
                .collect();
            format!("{{ {} }}", members.join(", "))
        }
    }
}

/// `s` as a JSON string. hew quotes with Go's `strconv.Quote`, which
/// agrees for every string without control characters; this writes the
/// JSON escapes for those, where Go's would not be JSON.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Replace `[start, end)` of `src` with `text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

impl Edit {
    pub fn apply(&self, src: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(src.len() + self.text.len());
        out.extend_from_slice(&src[..self.start]);
        out.extend_from_slice(self.text.as_bytes());
        out.extend_from_slice(&src[self.end..]);
        out
    }
}

/// Append `"key": <value>` to the object `obj` (hew's `insertIntoObj`,
/// appending).
pub fn append_member(src: &[u8], obj: &Node, key: &str, value: &Value) -> Edit {
    let text = format!("{}: {}", quote(key), encode(value));
    let members = obj.members().unwrap_or_default();
    let first = members.first().map_or(obj.end, |m| m.key_start);
    let block = src[obj.start..first].contains(&b'\n');
    let Some(last) = members.last() else {
        if block {
            return seed_block(src, obj.start, obj.end, &text);
        }
        // Inserted after the brace, so the interior is kept after the
        // member: `{}`, `{ }` and `{  }` stay three documents, so a removal
        // can give each back.
        return Edit {
            start: obj.start + 1,
            end: obj.start + 1,
            text: format!(" {text} "),
        };
    };
    let after = last.value.end;
    let text = if block {
        format!(",\n{}{text}", line_indent(src, first))
    } else {
        format!(", {text}")
    };
    Edit {
        start: after,
        end: after,
        text,
    }
}

/// Take the member at `index` out of the object `obj`, with exactly one
/// adjoining comma and its own leading gap (hew's `genericRemove`).
pub fn remove_member(src: &[u8], obj: &Node, index: usize) -> Edit {
    let members = obj.members().unwrap_or_default();
    let member = &members[index];
    let start_after_previous = |previous: &Member, keep_comma: bool| match previous.comma {
        Some(comma) if keep_comma => comma + 1,
        Some(comma) => comma,
        None => previous.value.end,
    };
    if index + 1 < members.len() {
        let start = match index {
            0 => obj.start + 1,
            _ => start_after_previous(&members[index - 1], true),
        };
        let end = member.comma.map_or(member.value.end, |comma| comma + 1);
        return Edit {
            start,
            end,
            text: String::new(),
        };
    }
    let start = match index {
        0 => obj.start + 1,
        _ => start_after_previous(&members[index - 1], false),
    };
    let mut end = member.value.end;
    // A padded container's only member takes its trailing pad too: the
    // inverse of seeding `{ member }`.
    if index == 0 && src[obj.start + 1] == b' ' && src.get(end) == Some(&b' ') {
        end += 1;
    }
    Edit {
        start,
        end,
        text: String::new(),
    }
}

/// Seed an empty block object `[open, end)` with its first member, on a
/// line of its own one indent in (hew's `SeedBlock`).
fn seed_block(src: &[u8], open: usize, end: usize, child: &str) -> Edit {
    let outer = line_indent(src, open);
    let interior = String::from_utf8_lossy(&src[open + 1..end - 1]);
    let kept = interior.trim_end_matches([' ', '\t', '\r', '\n']);
    Edit {
        start: open + 1,
        end: end - 1,
        text: format!("{kept}\n{outer}{}{child}\n{outer}", indent_unit(src)),
    }
}

/// The leading whitespace of the line holding `at`, and only that.
fn line_indent(src: &[u8], at: usize) -> String {
    let line = src[..at]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let indent = src[line..at]
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    String::from_utf8_lossy(&src[line..line + indent]).into_owned()
}

/// The file's own indent step: the leading whitespace of its first
/// indented line that carries content, else two spaces.
fn indent_unit(src: &[u8]) -> String {
    for line in src.split(|&b| b == b'\n') {
        let body = line
            .iter()
            .position(|&b| b != b' ' && b != b'\t')
            .unwrap_or(line.len());
        let rest = &line[body..];
        if !rest.is_empty() && rest != b"\r" && body > 0 {
            return String::from_utf8_lossy(&line[..body]).into_owned();
        }
    }
    "  ".to_string()
}

#[cfg(test)]
#[path = "json.test.rs"]
mod tests;
