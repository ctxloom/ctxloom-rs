//! The resources: the tag schema a write is held to, and the tags in use,
//! each read fresh through the use cases and answered as Go taskloom's
//! resources answer, its JSON indented and escaped as Go's encoder does.

use std::collections::BTreeMap;
use std::io;

use presentation::Catalog;
use rmcp::model::Resource;
use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};
use tasks_application::{ListQuery, Reach, TagSchema, Vocabulary};
use tasks_presentation::TagCountJson;

use crate::{McpNotice, Settings};

pub const TAG_SCHEMA: &str = "taskloom://tag-schema";
pub const TAG_VOCABULARY: &str = "taskloom://tag-vocabulary";

/// The MIME type both resources are read as.
pub const JSON: &str = "application/json";

/// The resources this server offers, as `resources/list` answers: Go
/// taskloom's (URI, name, description and MIME type) verbatim, in Go's
/// order.
pub const RESOURCES: &str = include_str!("resources.json");

/// The resources `RESOURCES` describes.
pub fn offered() -> Vec<Resource> {
    serde_json::from_str(RESOURCES).expect("RESOURCES is a JSON array of resources")
}

/// One declared target: Go's `tagSchemaTargetDoc`, every facet the
/// resource reports of it.
#[derive(Serialize, Default)]
struct TargetDoc<'a> {
    target: &'a str,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    scalar: bool,
    #[serde(rename = "enum", skip_serializing_if = "<[String]>::is_empty")]
    members: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<RangeDoc>,
    #[serde(skip_serializing_if = "str::is_empty")]
    priority_fn: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    decay_fn: &'a str,
}

#[derive(Serialize)]
struct RangeDoc {
    min: f64,
    max: f64,
}

/// Go's `tagSchemaDoc`.
#[derive(Serialize)]
struct SchemaDoc<'a> {
    targets: Vec<TargetDoc<'a>>,
}

/// `schema` as the tag-schema resource reads: one entry per target any
/// arity, enum, range or formula is declared for, by target, as Go's
/// `buildTagSchemaDoc`. What a listing hides and a target's type are not
/// write-time vocabulary, so are left out, as Go leaves them.
pub fn schema_text(schema: &TagSchema) -> String {
    let mut targets: BTreeMap<&str, TargetDoc> = BTreeMap::new();
    for (target, scalar) in schema.arities() {
        entry(&mut targets, target).scalar = scalar;
    }
    for (target, members) in schema.enums() {
        entry(&mut targets, target).members = members;
    }
    for (target, range) in schema.ranges() {
        entry(&mut targets, target).range = Some(RangeDoc {
            min: range.min,
            max: range.max,
        });
    }
    for (target, formula) in schema.priority_fns() {
        entry(&mut targets, target).priority_fn = formula;
    }
    for (target, formula) in schema.decay_fns() {
        entry(&mut targets, target).decay_fn = formula;
    }
    go_indented(&SchemaDoc {
        targets: targets.into_values().collect(),
    })
}

/// `target`'s entry, begun if it has none yet.
fn entry<'m, 'a>(
    targets: &'m mut BTreeMap<&'a str, TargetDoc<'a>>,
    target: &'a str,
) -> &'m mut TargetDoc<'a> {
    targets.entry(target).or_insert_with(|| TargetDoc {
        target,
        ..TargetDoc::default()
    })
}

/// Go's `tagVocabularyDoc`: the project read, or a warning saying why
/// none was, and the tags in use.
#[derive(Serialize)]
struct VocabularyDoc<'a> {
    #[serde(skip_serializing_if = "str::is_empty")]
    project_id: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    project_dir: &'a str,
    #[serde(skip_serializing_if = "String::is_empty")]
    warning: String,
    tags: Vec<TagCountJson>,
}

/// What the tag-vocabulary resource counts: every task of the project
/// here (or the one pinned), whatever its status, as Go's counts the
/// whole project.
pub fn vocabulary_query(settings: &Settings) -> ListQuery {
    ListQuery {
        project: settings.project.clone(),
        ..ListQuery::default()
    }
}

/// `vocabulary` as the tag-vocabulary resource reads: the project it
/// counted, or why it counted none, or every project for want of one.
pub fn vocabulary_text(vocabulary: &Vocabulary, catalog: &Catalog) -> String {
    let project = vocabulary.reach.project();
    let warning = match &vocabulary.reach {
        Reach::Project(_) | Reach::Every { fallback: None, .. } => String::new(),
        Reach::NoProject => catalog.message(&McpNotice::NoProject),
        Reach::Every {
            fallback: Some(directory),
            ..
        } => catalog.message(&McpNotice::EveryProject {
            directory: directory.clone(),
        }),
    };
    go_indented(&VocabularyDoc {
        project_id: project.map_or("", |p| p.id.as_str()),
        project_dir: project.map_or("", |p| p.root.as_str()),
        warning,
        tags: vocabulary.tags.iter().map(TagCountJson::of).collect(),
    })
}

/// `value` as Go's `json.MarshalIndent(value, "", "  ")` writes it.
fn go_indented(value: &impl Serialize) -> String {
    let mut out = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, GoFormatter::default());
    // Serializing to memory fails only on a non-string map key, and these
    // documents have none.
    value
        .serialize(&mut serializer)
        .expect("a resource document serializes");
    String::from_utf8(out).expect("serde_json writes UTF-8")
}

/// serde_json's pretty printer, two spaces a level, as Go's encoder
/// writes: a float in Go's shortest form (`5`, not `5.0`), and `<`, `>`,
/// `&`, U+2028 and U+2029 escaped in strings.
#[derive(Default)]
struct GoFormatter {
    pretty: PrettyFormatter<'static>,
}

impl Formatter for GoFormatter {
    fn write_f64<W: ?Sized + io::Write>(&mut self, writer: &mut W, value: f64) -> io::Result<()> {
        writer.write_all(go_float(value).as_bytes())
    }

    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut rest = fragment;
        while let Some(at) = rest.find(['<', '>', '&', '\u{2028}', '\u{2029}']) {
            writer.write_all(&rest.as_bytes()[..at])?;
            let c = rest[at..].chars().next().expect("a character was found");
            write!(writer, "\\u{:04x}", u32::from(c))?;
            rest = &rest[at + c.len_utf8()..];
        }
        writer.write_all(rest.as_bytes())
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_array(writer)
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array(writer)
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_array_value(writer, first)
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_array_value(writer)
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object(writer)
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object(writer)
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pretty.begin_object_key(writer, first)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.begin_object_value(writer)
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.pretty.end_object_value(writer)
    }
}

/// `value` as Go's encoder writes a float64: its shortest form, plain
/// from 1e-6 up to 1e21, else in exponent form with a signed exponent of
/// at least one digit (`1e-7`, `1e+21`).
fn go_float(value: f64) -> String {
    let magnitude = value.abs();
    if magnitude == 0.0 || (1e-6..1e21).contains(&magnitude) {
        return format!("{value}");
    }
    let exponent_form = format!("{value:e}");
    match exponent_form.split_once('e') {
        Some((mantissa, exponent)) if !exponent.starts_with('-') => {
            format!("{mantissa}e+{exponent}")
        }
        _ => exponent_form,
    }
}

#[cfg(test)]
#[path = "resources.test.rs"]
mod tests;
