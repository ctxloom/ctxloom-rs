//! A companion's loadout document (Go's `bundles.ParseLoadout`): what a
//! binary on the `PATH` prints for `<bin> loadout --format yaml`. One document
//! with two sections of different lifecycles: `run:`, a bundle consumed every
//! session, and `init:`, typed fields consumed once at setup.
//!
//! Strict at every level: a key the document does not model is refused, not
//! dropped, and a bare bundle at the top level is an unknown key, not a
//! loadout with an empty run section. A document with neither section
//! contributed nothing and fails loudly.

use angzarr_yaml::Value;

use crate::document::parse_document;
use crate::error::BundleError;
use crate::model::Bundle;
use crate::yamlx::{self, key_line, nearest};

/// The sections a loadout may declare.
const TOP_KEYS: [&str; 2] = ["init", "run"];
const INIT_KEYS: [&str; 4] = ["legacy_context", "questions", "setup_guidance", "tooling"];
const QUESTION_KEYS: [&str; 2] = ["id", "prompt"];

/// A companion's advertised loadout.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct Loadout {
    /// The `run:` bundle; empty where the document declares none.
    pub run: Bundle,
    /// The `init:` section; zero where the document declares none.
    pub init: InitLoadout,
}

/// The setup-time half of a loadout, every field read by name.
#[derive(Clone, PartialEq, Eq, Default, Debug)]
pub struct InitLoadout {
    /// Text spliced into the init prompt after the built-in body.
    pub setup_guidance: String,
    /// The tools the companion's content needs where agents run.
    pub tooling: String,
    /// Where the companion's pre-existing project context lives.
    pub legacy_context: Vec<String>,
    /// What init should put to the human on the companion's behalf.
    pub questions: Vec<InitQuestion>,
}

impl InitLoadout {
    /// Whether the section declares nothing at all.
    pub fn is_zero(&self) -> bool {
        self.setup_guidance.is_empty()
            && self.tooling.is_empty()
            && self.legacy_context.is_empty()
            && self.questions.is_empty()
    }
}

/// One question a companion asks init to put to the human.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InitQuestion {
    /// Keys the answer; stable across the companion's releases.
    pub id: String,
    /// The question as the human sees it.
    pub prompt: String,
}

/// Parse a loadout document.
pub fn parse_loadout(text: &str) -> Result<Loadout, BundleError> {
    let root = yamlx::parse(text).map_err(|cause| BundleError::Syntax { cause })?;
    let sections = match &root {
        Value::Null => &[][..],
        Value::Map(entries) => entries,
        other => {
            return Err(bad(
                "(document)",
                format!(
                    "expected a mapping at the top level, found {}",
                    yamlx::kind(other)
                ),
            ))
        }
    };
    for (key, _) in sections {
        if !TOP_KEYS.contains(&key.as_str()) {
            return Err(unknown(
                key,
                &[],
                text,
                "the top level of the loadout",
                &TOP_KEYS,
            ));
        }
    }
    let init = match root.get("init") {
        Some(value) => init_of(value, text)?,
        None => InitLoadout::default(),
    };
    let run = root.get("run").filter(|value| **value != Value::Null);
    let Some(run) = run else {
        if init.is_zero() {
            return Err(BundleError::LoadoutEmpty {
                bytes: text.len() as u64,
            });
        }
        return Ok(Loadout {
            run: Bundle::default(),
            init,
        });
    };
    // The run section takes exactly the path every bundle takes.
    let run = parse_document(&angzarr_yaml::write(run, usize::MAX))?;
    Ok(Loadout { run, init })
}

fn init_of(value: &Value, text: &str) -> Result<InitLoadout, BundleError> {
    let fields = match value {
        Value::Null => return Ok(InitLoadout::default()),
        Value::Map(entries) => entries,
        other => {
            return Err(bad(
                "init",
                format!("expected a mapping, found {}", yamlx::kind(other)),
            ))
        }
    };
    for (key, _) in fields {
        if !INIT_KEYS.contains(&key.as_str()) {
            return Err(unknown(key, &["init"], text, "`init`", &INIT_KEYS));
        }
    }
    let scalar = |key: &str| -> Result<String, BundleError> {
        match value.get(key) {
            Some(v) => yamlx::string(v).map_err(|detail| bad(format!("init.{key}"), detail)),
            None => Ok(String::new()),
        }
    };
    let legacy_context = match value.get("legacy_context") {
        Some(v) => yamlx::string_list(v).map_err(|detail| bad("init.legacy_context", detail))?,
        None => Vec::new(),
    };
    Ok(InitLoadout {
        setup_guidance: scalar("setup_guidance")?,
        tooling: scalar("tooling")?,
        legacy_context,
        questions: match value.get("questions") {
            Some(v) => questions_of(v, text)?,
            None => Vec::new(),
        },
    })
}

fn questions_of(value: &Value, text: &str) -> Result<Vec<InitQuestion>, BundleError> {
    let items = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Seq(items) => items,
        other => {
            return Err(bad(
                "init.questions",
                format!("expected a list, found {}", yamlx::kind(other)),
            ))
        }
    };
    items
        .iter()
        .map(|item| {
            let Value::Map(fields) = item else {
                return Err(bad(
                    "init.questions[]",
                    format!("expected a mapping, found {}", yamlx::kind(item)),
                ));
            };
            for (key, _) in fields {
                if !QUESTION_KEYS.contains(&key.as_str()) {
                    return Err(unknown(
                        key,
                        &["init", "questions", "[]"],
                        text,
                        "`init.questions[]`",
                        &QUESTION_KEYS,
                    ));
                }
            }
            let field = |key: &str| -> Result<String, BundleError> {
                match item.get(key) {
                    Some(v) => yamlx::string(v)
                        .map_err(|detail| bad(format!("init.questions[].{key}"), detail)),
                    None => Ok(String::new()),
                }
            };
            Ok(InitQuestion {
                id: field("id")?,
                prompt: field("prompt")?,
            })
        })
        .collect()
}

fn bad(path: impl Into<String>, detail: impl Into<String>) -> BundleError {
    BundleError::BadValue {
        path: path.into(),
        detail: detail.into(),
    }
}

fn unknown(key: &str, path: &[&str], text: &str, site: &str, valid: &[&str]) -> BundleError {
    let mut full: Vec<&str> = path.to_vec();
    full.push(key);
    BundleError::UnknownKey {
        key: key.to_string(),
        line: key_line(text, &full),
        site: site.to_string(),
        suggestion: nearest(key, valid),
        valid: valid.join(", "),
    }
}

#[cfg(test)]
#[path = "loadout.test.rs"]
mod tests;
