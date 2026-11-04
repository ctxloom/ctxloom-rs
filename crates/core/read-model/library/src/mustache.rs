//! Variable substitution in a fragment (Go's `composite/substitute.go`, over
//! `cbroglie/mustache`): `{{name}}` (HTML-escaped), `{{{name}}}` and
//! `{{&name}}` (raw), sections `{{#x}}..{{/x}}` and `{{^x}}..{{/x}}`,
//! comments `{{! }}`, and the Set Delimiter escape `{{=<% %>=}}`.
//!
//! An undefined PLAIN variable renders verbatim, as the author wrote it
//! (`{{ARGS}}` stays `{{ARGS}}`), and is reported; a name used as a section is
//! falsy when undefined, so a presence toggle (`{{#DEBUG}}`) keeps its
//! polarity. A template that does not parse or render is served as written
//! and reported.

use std::collections::{BTreeMap, BTreeSet};

/// `content` with `vars` substituted; every problem is handed to `report`.
pub fn substitute(
    content: &str,
    vars: &BTreeMap<String, String>,
    report: &mut dyn FnMut(String),
) -> String {
    let elems = match Parser::new(content).parse() {
        Ok(elems) => elems,
        Err(why) => {
            report(format!("failed to parse template: {why}"));
            return content.to_string();
        }
    };
    check_tags(&elems, vars, &mut BTreeSet::new(), report);
    let mut data = vars.clone();
    let mut sections = BTreeSet::new();
    let mut plain = BTreeMap::new();
    collect(&elems, vars, &mut sections, &mut plain);
    for (name, literal) in plain {
        if !sections.contains(&name) {
            data.insert(name, literal);
        }
    }
    let mut out = String::new();
    render(&elems, &data, &[], &mut out);
    out
}

/// A parsed template.
#[derive(Debug)]
enum Elem {
    Text(String),
    Var {
        name: String,
        raw: bool,
    },
    Section {
        name: String,
        inverted: bool,
        elems: Vec<Elem>,
    },
}

const SKIP_WHITESPACE_TAG_TYPES: &str = "#^/<>=!";

struct Parser<'a> {
    data: &'a str,
    otag: String,
    ctag: String,
    p: usize,
    line: usize,
}

/// What stopped the parse.
enum Stop {
    Eof,
    Error(String),
}

impl<'a> Parser<'a> {
    fn new(data: &'a str) -> Parser<'a> {
        Parser {
            data,
            otag: "{{".to_string(),
            ctag: "}}".to_string(),
            p: 0,
            line: 1,
        }
    }

    fn parse(&mut self) -> Result<Vec<Elem>, String> {
        let mut elems = Vec::new();
        match self.parse_into(&mut elems, None) {
            Err(Stop::Error(why)) => Err(why),
            Err(Stop::Eof) | Ok(()) => Ok(elems),
        }
    }

    /// Text up to the next open tag, the padding before it on its line, and
    /// whether the tag may stand alone on its line. `Err(Eof)` carries the
    /// remaining text.
    fn read_text(&mut self) -> Result<(String, String, bool), String> {
        let prev = self.p;
        let Some(found) = self.data[self.p..].find(&self.otag) else {
            let rest = self.data[self.p..].to_string();
            self.p = self.data.len();
            return Err(rest);
        };
        let at = self.p + found;
        self.line += self.data[prev..at].matches('\n').count();
        self.p = at + self.otag.len();
        let b = self.data.as_bytes();
        let mut i = at;
        while i > prev && (b[i - 1] == b' ' || b[i - 1] == b'\t') {
            i -= 1;
        }
        let may_standalone = i == 0 || b[i - 1] == b'\n';
        if may_standalone {
            Ok((
                self.data[prev..i].to_string(),
                self.data[i..at].to_string(),
                true,
            ))
        } else {
            Ok((self.data[prev..at].to_string(), String::new(), false))
        }
    }

    fn read_tag(&mut self, may_standalone: bool) -> Result<(String, bool), Stop> {
        let close = if self.data[self.p..].starts_with('{') {
            format!("}}{}", self.ctag)
        } else {
            self.ctag.clone()
        };
        let Some(found) = self.data[self.p..].find(&close) else {
            return Err(Stop::Error(format!(
                "Line {}: unmatched open tag",
                self.line
            )));
        };
        // A raw tag keeps its own closing brace: `{{{name}}}` reads `{name}`.
        let text = &self.data[self.p..self.p + found + close.len() - self.ctag.len()];
        self.line += text.matches('\n').count();
        self.p += found + close.len();
        let tag = text.trim().to_string();
        if tag.is_empty() {
            return Err(Stop::Error(format!("Line {}: empty tag", self.line)));
        }
        let b = self.data.as_bytes();
        let mut eow = self.p;
        for (i, c) in b.iter().enumerate().skip(self.p) {
            if *c != b' ' && *c != b'\t' {
                eow = i;
                break;
            }
        }
        let mut standalone = true;
        if may_standalone {
            let first = &tag[..1];
            if !SKIP_WHITESPACE_TAG_TYPES.contains(first) {
                standalone = false;
            } else if eow == b.len() {
                self.p = eow;
            } else if b[eow] == b'\n' {
                self.p = eow + 1;
                self.line += 1;
            } else if b[eow] == b'\r' && b.get(eow + 1) == Some(&b'\n') {
                self.p = eow + 2;
                self.line += 1;
            } else {
                standalone = false;
            }
        }
        Ok((tag, standalone))
    }

    /// Parse into `elems` until the end of the input, or (inside the section
    /// `within`) its closing tag.
    fn parse_into(
        &mut self,
        elems: &mut Vec<Elem>,
        within: Option<(&str, usize)>,
    ) -> Result<(), Stop> {
        loop {
            let (text, padding, may_standalone) = match self.read_text() {
                Ok(t) => t,
                Err(rest) => {
                    return match within {
                        Some((name, line)) => Err(Stop::Error(format!(
                            "Line {line}: Section '{name}' has no closing tag"
                        ))),
                        None => {
                            elems.push(Elem::Text(rest));
                            Err(Stop::Eof)
                        }
                    };
                }
            };
            elems.push(Elem::Text(text));
            let (tag, standalone) = self.read_tag(may_standalone)?;
            if !standalone {
                elems.push(Elem::Text(padding));
            }
            let first = tag.as_bytes()[0];
            match first {
                b'!' => {}
                b'#' | b'^' => {
                    let name = tag[1..].trim().to_string();
                    let mut body = Vec::new();
                    let line = self.line;
                    self.parse_into(&mut body, Some((&name, line)))?;
                    elems.push(Elem::Section {
                        name,
                        inverted: first == b'^',
                        elems: body,
                    });
                }
                b'/' => {
                    let name = tag[1..].trim();
                    return match within {
                        Some((open, _)) if open == name => Ok(()),
                        Some(_) => Err(Stop::Error(format!(
                            "Line {}: Interleaved closing tag: {name}",
                            self.line
                        ))),
                        None => Err(Stop::Error(format!(
                            "Line {}: unmatched close tag",
                            self.line
                        ))),
                    };
                }
                b'>' => {
                    return Err(Stop::Error(format!(
                        "Line {}: partials are not supported: {}",
                        self.line,
                        tag[1..].trim()
                    )))
                }
                b'=' => self.custom_delimiter(&tag, elems),
                b'{' => {
                    if tag.ends_with('}') {
                        elems.push(Elem::Var {
                            name: tag[1..tag.len() - 1].trim().to_string(),
                            raw: true,
                        });
                    }
                }
                b'&' => elems.push(Elem::Var {
                    name: tag[1..].trim().to_string(),
                    raw: true,
                }),
                _ => elems.push(Elem::Var {
                    name: tag,
                    raw: false,
                }),
            }
        }
    }

    fn custom_delimiter(&mut self, tag: &str, elems: &mut Vec<Elem>) {
        if tag.len() < 3 || !tag.ends_with('=') {
            elems.push(Elem::Var {
                name: tag.to_string(),
                raw: false,
            });
            return;
        }
        let trimmed = tag[1..tag.len() - 1].trim();
        match trimmed.split_once(' ') {
            Some((open, close)) => {
                self.otag = open.to_string();
                self.ctag = close.to_string();
            }
            None => elems.push(Elem::Var {
                name: tag.to_string(),
                raw: false,
            }),
        }
    }
}

/// Report each name a template uses that no variable defines, once.
fn check_tags(
    elems: &[Elem],
    vars: &BTreeMap<String, String>,
    seen: &mut BTreeSet<String>,
    report: &mut dyn FnMut(String),
) {
    for elem in elems {
        match elem {
            Elem::Text(_) => {}
            Elem::Var { name, .. } => note(name, vars, seen, report),
            Elem::Section { name, elems, .. } => {
                note(name, vars, seen, report);
                check_tags(elems, vars, seen, report);
            }
        }
    }
}

fn note(
    name: &str,
    vars: &BTreeMap<String, String>,
    seen: &mut BTreeSet<String>,
    report: &mut dyn FnMut(String),
) {
    if seen.insert(name.to_string()) && !vars.contains_key(name) {
        report(format!("undefined variable: {{{{{name}}}}}"));
    }
}

/// The names used as sections anywhere, and, for each name used only as a
/// plain variable and undefined, the literal text to render for it.
fn collect(
    elems: &[Elem],
    vars: &BTreeMap<String, String>,
    sections: &mut BTreeSet<String>,
    plain: &mut BTreeMap<String, String>,
) {
    for elem in elems {
        match elem {
            Elem::Text(_) => {}
            Elem::Var { name, .. } => {
                if !vars.contains_key(name) {
                    plain
                        .entry(name.clone())
                        .or_insert_with(|| format!("{{{{{name}}}}}"));
                }
            }
            Elem::Section { name, elems, .. } => {
                sections.insert(name.clone());
                collect(elems, vars, sections, plain);
            }
        }
    }
}

/// The values a name is looked up in, innermost first: a section over a
/// string pushes the string.
fn render(elems: &[Elem], data: &BTreeMap<String, String>, inner: &[&str], out: &mut String) {
    for elem in elems {
        match elem {
            Elem::Text(text) => out.push_str(text),
            Elem::Var { name, raw } => {
                let value = lookup(name, data, inner).unwrap_or_default();
                if *raw {
                    out.push_str(&value);
                } else {
                    escape_into(&value, out);
                }
            }
            Elem::Section {
                name,
                inverted,
                elems,
            } => {
                let value = lookup(name, data, inner);
                let empty = value.as_deref().is_none_or(|v| v.trim().is_empty());
                if *inverted {
                    if empty {
                        render(elems, data, inner, out);
                    }
                } else if let Some(value) = value.filter(|_| !empty) {
                    let mut deeper = vec![value.as_str()];
                    deeper.extend_from_slice(inner);
                    render(elems, data, &deeper, out);
                }
            }
        }
    }
}

/// A name's value: `.` is the innermost string; a dotted name finds nothing in
/// a map of strings; any other name is a key of the variables.
fn lookup(name: &str, data: &BTreeMap<String, String>, inner: &[&str]) -> Option<String> {
    if name == "." {
        return inner.first().map(ToString::to_string);
    }
    if name.contains('.') {
        return None;
    }
    data.get(name).cloned()
}

/// Go's `html/template.HTMLEscape`.
fn escape_into(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '"' => out.push_str("&#34;"),
            '\'' => out.push_str("&#39;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\0' => out.push('\u{fffd}'),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
#[path = "mustache.test.rs"]
mod tests;
