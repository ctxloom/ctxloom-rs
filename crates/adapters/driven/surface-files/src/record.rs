//! The ownership record: for each file ctxloom has put something in, the bytes
//! it held before, the bytes ctxloom wrote, and every claim in it. Kept as one
//! JSON document (which is YAML too) beside the others in the home's
//! `records/`, one per target directory.
//!
//! Go's `*.claims.yaml` is a different document; reading Go's is the cutover's
//! (S15), and what this keeps is the same promise: `uninstall` and
//! `materialize --release` give the user's files back.

use std::collections::BTreeMap;

use surface_ports::{Claim, Json, JsonOp, Kind, SurfaceError};

/// One claim, and whose it is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Owned {
    pub engine: String,
    pub kind: Kind,
    pub claim: Claim,
}

/// A file ctxloom has put something in.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct FileRecord {
    /// The file was there before ctxloom's first claim in it.
    pub existed: bool,
    /// What it held then.
    pub original: Vec<u8>,
    /// What ctxloom last wrote.
    pub written: Vec<u8>,
    pub claims: Vec<Owned>,
}

/// The record of one target directory.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct Record {
    pub files: BTreeMap<String, FileRecord>,
}

const SCHEMA: &str = "1";

pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn op_json(op: &JsonOp) -> Json {
    match op {
        JsonOp::Set { path, value } => Json::object([
            ("op", Json::text("set")),
            ("path", Json::strings(path)),
            ("value", value.clone()),
        ]),
        JsonOp::SetCommand { path, value } => Json::object([
            ("op", Json::text("set-command")),
            ("path", Json::strings(path)),
            ("value", value.clone()),
        ]),
        JsonOp::Push { path, value } => Json::object([
            ("op", Json::text("push")),
            ("path", Json::strings(path)),
            ("value", value.clone()),
        ]),
        JsonOp::HookEntry {
            event,
            matcher,
            value,
        } => Json::object([
            ("op", Json::text("hook")),
            ("event", Json::text(event.as_str())),
            ("matcher", matcher.as_deref().map_or(Json::Null, Json::text)),
            ("value", value.clone()),
        ]),
    }
}

fn claim_json(claim: &Claim) -> Json {
    match claim {
        Claim::Section { text, .. } => Json::object([
            ("type", Json::text("section")),
            ("text", Json::text(text.as_str())),
        ]),
        Claim::Json { ops, .. } => Json::object([
            ("type", Json::text("json")),
            ("ops", Json::Array(ops.iter().map(op_json).collect())),
        ]),
        Claim::Whole {
            bytes, executable, ..
        } => Json::object([
            ("type", Json::text("whole")),
            ("bytes", Json::text(hex(bytes))),
            ("executable", Json::Bool(*executable)),
        ]),
    }
}

impl Record {
    pub(crate) fn encode(&self, target: &str) -> String {
        let files = self
            .files
            .iter()
            .map(|(path, file)| {
                let claims = file
                    .claims
                    .iter()
                    .map(|owned| {
                        Json::object([
                            ("engine", Json::text(owned.engine.as_str())),
                            ("kind", Json::text(owned.kind.name())),
                            ("claim", claim_json(&owned.claim)),
                        ])
                    })
                    .collect();
                (
                    path.clone(),
                    Json::object([
                        ("existed", Json::Bool(file.existed)),
                        ("original", Json::text(hex(&file.original))),
                        ("written", Json::text(hex(&file.written))),
                        ("claims", Json::Array(claims)),
                    ]),
                )
            })
            .collect::<Vec<_>>();
        let doc = Json::object([
            ("schema_version", Json::text(SCHEMA)),
            ("target", Json::text(target)),
            ("files", Json::Object(files)),
        ]);
        format!("{}\n", doc.pretty())
    }

    pub(crate) fn decode(file: &str, text: &str) -> Result<Record, SurfaceError> {
        let bad = |detail: &str| SurfaceError::Record {
            file: file.to_string(),
            detail: detail.to_string(),
        };
        let doc = Json::parse(text).map_err(|e| bad(&e))?;
        if doc.get("schema_version").and_then(Json::as_str) != Some(SCHEMA) {
            return Err(bad("its schema_version is not one this ctxloom reads"));
        }
        let Some(Json::Object(files)) = doc.get("files") else {
            return Err(bad("it holds no files"));
        };
        let mut record = Record::default();
        for (path, entry) in files {
            let bytes = |key: &str| {
                entry
                    .get(key)
                    .and_then(Json::as_str)
                    .and_then(unhex)
                    .ok_or_else(|| bad(&format!("{path}: {key} is not bytes")))
            };
            let existed = matches!(entry.get("existed"), Some(Json::Bool(true)));
            let mut claims = Vec::new();
            for owned in entry
                .get("claims")
                .and_then(Json::as_array)
                .ok_or_else(|| bad(&format!("{path}: no claims")))?
            {
                claims.push(
                    decode_owned(path, owned)
                        .ok_or_else(|| bad(&format!("{path}: a claim is not one")))?,
                );
            }
            record.files.insert(
                path.clone(),
                FileRecord {
                    existed,
                    original: bytes("original")?,
                    written: bytes("written")?,
                    claims,
                },
            );
        }
        Ok(record)
    }
}

fn strings(value: Option<&Json>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|s| s.as_str().map(str::to_string))
        .collect()
}

fn decode_op(op: &Json) -> Option<JsonOp> {
    let value = op.get("value")?.clone();
    Some(match op.get("op")?.as_str()? {
        "set" => JsonOp::Set {
            path: strings(op.get("path"))?,
            value,
        },
        "set-command" => JsonOp::SetCommand {
            path: strings(op.get("path"))?,
            value,
        },
        "push" => JsonOp::Push {
            path: strings(op.get("path"))?,
            value,
        },
        "hook" => JsonOp::HookEntry {
            event: op.get("event")?.as_str()?.to_string(),
            matcher: op.get("matcher")?.as_str().map(str::to_string),
            value,
        },
        _ => return None,
    })
}

fn decode_owned(path: &str, owned: &Json) -> Option<Owned> {
    let engine = owned.get("engine")?.as_str()?.to_string();
    let kind = Kind::parse(owned.get("kind")?.as_str()?)?;
    let claim = owned.get("claim")?;
    let path = path.to_string();
    let claim = match claim.get("type")?.as_str()? {
        "section" => Claim::Section {
            path,
            text: claim.get("text")?.as_str()?.to_string(),
        },
        "json" => Claim::Json {
            path,
            ops: claim
                .get("ops")?
                .as_array()?
                .iter()
                .map(decode_op)
                .collect::<Option<Vec<_>>>()?,
        },
        "whole" => Claim::Whole {
            path,
            bytes: unhex(claim.get("bytes")?.as_str()?)?,
            executable: matches!(claim.get("executable"), Some(Json::Bool(true))),
        },
        _ => return None,
    };
    Some(Owned {
        engine,
        kind,
        claim,
    })
}

#[cfg(test)]
#[path = "record.test.rs"]
mod tests;
