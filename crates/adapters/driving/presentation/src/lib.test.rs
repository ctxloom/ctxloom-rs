//! Rendering a view, and the crate's independence from any domain.

use std::io::{self, Write};

use serde::Serialize;

use super::{render, write_json, write_json_line, Format, Render};

/// A toy view: its JSON a list, its text a sentence.
struct Count(Vec<u8>);

impl Render for Count {
    fn json(&self) -> impl Serialize + '_ {
        &self.0
    }

    fn text(&self, out: &mut dyn Write) -> io::Result<()> {
        writeln!(out, "{} items", self.0.len())
    }
}

fn rendered(format: Format, view: &impl Render) -> String {
    let mut out = Vec::new();
    render(&mut out, format, view).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn test_a_view_renders_as_its_text_or_its_json() {
    let view = Count(vec![1, 2]);
    assert_eq!(rendered(Format::Text, &view), "2 items\n");
    assert_eq!(rendered(Format::Json, &view), "[\n  1,\n  2\n]\n");
}

#[test]
fn test_json_is_one_indented_document_and_a_newline() {
    let mut out = Vec::new();
    write_json(&mut out, &serde_json::json!({"a": []})).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "{\n  \"a\": []\n}\n");
}

/// A writer that records what reached it by a flush.
#[derive(Default)]
struct Flushed {
    pending: Vec<u8>,
    flushed: Vec<u8>,
}

impl std::io::Write for Flushed {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushed.append(&mut self.pending);
        Ok(())
    }
}

#[test]
fn test_a_json_line_is_one_compact_document_a_newline_and_flushed() {
    let mut out = Flushed::default();
    write_json_line(&mut out, &serde_json::json!({"a": [1, 2], "b": "c"})).unwrap();
    write_json_line(&mut out, &serde_json::json!({"a": []})).unwrap();
    assert_eq!(
        String::from_utf8(out.flushed).unwrap(),
        "{\"a\":[1,2],\"b\":\"c\"}\n{\"a\":[]}\n"
    );
    assert!(out.pending.is_empty(), "each line is flushed");
}

/// The names of the dependencies a manifest declares, in every section.
fn dependencies(manifest: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.ends_with("dependencies]");
        } else if in_dependencies && !line.is_empty() && !line.starts_with('#') {
            names.extend(line.split(['=', '.', ' ']).next());
        }
    }
    names
}

#[test]
fn test_it_depends_on_no_crate_of_this_workspace_but_annals_diagnostic() {
    let workspace = include_str!("../../../../../Cargo.toml");
    let ours: Vec<&str> = workspace
        .lines()
        // annals is pinned by git rather than path, but is still not
        // presentation's to depend on (only annals-diagnostic is).
        .filter(|line| line.contains("path = \"") || line.contains("/benjaminabbitt/annals\""))
        .filter_map(|line| line.split([' ', '=']).next())
        .filter(|name| *name != "annals-diagnostic")
        .collect();
    assert!(ours.contains(&"tasks"), "the workspace's crates: {ours:?}");
    assert!(ours.contains(&"annals"), "{ours:?}");
    let mine = dependencies(include_str!("../Cargo.toml"));
    assert!(mine.contains(&"annals-diagnostic"), "{mine:?}");
    let domain: Vec<&&str> = mine.iter().filter(|name| ours.contains(name)).collect();
    assert!(
        domain.is_empty(),
        "presentation is reused by every binary, so it is domain-free: {domain:?}"
    );
}
