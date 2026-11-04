//! The ownership record: for each file ctxloom has put something in, the bytes
//! it held before, the bytes ctxloom wrote, and every claim in it. Kept as one
//! JSON document (which is YAML too) beside the others in the home's
//! `records/`, one per target directory.
//!
//! Go's `*.claims.yaml` is a different document; reading Go's is the cutover's
//! (S15), and what this keeps is the same promise: `uninstall` and
//! `materialize --release` give the user's files back.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
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

// The document as it is written: these shapes are the persisted format, field
// order included (serde writes fields in declaration order, a tag first).

#[derive(Serialize, Deserialize)]
struct Doc {
    schema_version: String,
    /// Written for whoever reads the file; reading ignores it.
    #[serde(skip_deserializing)]
    target: String,
    files: BTreeMap<String, FileWire>,
}

/// The document's version, read before the rest so that a version this
/// ctxloom does not read is refused as that, whatever else it holds.
#[derive(Deserialize)]
struct Header {
    schema_version: Option<Json>,
}

#[derive(Serialize, Deserialize)]
struct FileWire {
    #[serde(default, deserialize_with = "only_true")]
    existed: bool,
    #[serde(with = "hexed")]
    original: Vec<u8>,
    #[serde(with = "hexed")]
    written: Vec<u8>,
    claims: Vec<OwnedWire>,
}

#[derive(Serialize, Deserialize)]
struct OwnedWire {
    engine: String,
    #[serde(with = "named")]
    kind: Kind,
    claim: ClaimWire,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClaimWire {
    Section {
        text: String,
    },
    Json {
        ops: Vec<OpWire>,
    },
    Whole {
        #[serde(with = "hexed")]
        bytes: Vec<u8>,
        #[serde(default, deserialize_with = "only_true")]
        executable: bool,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
enum OpWire {
    #[serde(rename = "set")]
    Set { path: Vec<String>, value: Json },
    #[serde(rename = "set-command")]
    SetCommand { path: Vec<String>, value: Json },
    #[serde(rename = "push")]
    Push { path: Vec<String>, value: Json },
    #[serde(rename = "hook")]
    Hook {
        event: String,
        /// Always written (null for none); read as none unless a string.
        #[serde(deserialize_with = "string_or_none")]
        matcher: Option<String>,
        value: Json,
    },
}

/// Bytes as lower-case hex text.
mod hexed {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        hex::decode(String::deserialize(d)?).map_err(D::Error::custom)
    }
}

/// A kind as the name `--surface` uses.
mod named {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    use surface_ports::Kind;

    pub(super) fn serialize<S: Serializer>(kind: &Kind, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(kind.name())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Kind, D::Error> {
        let name = String::deserialize(d)?;
        Kind::parse(&name).ok_or_else(|| D::Error::custom(format!("{name} is not a kind")))
    }
}

/// A flag that is set only by `true`; any other value is unset.
fn only_true<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(matches!(Json::deserialize(d)?, Json::Bool(true)))
}

fn string_or_none<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Json::deserialize(d)?.as_str().map(str::to_string))
}

impl From<&JsonOp> for OpWire {
    fn from(op: &JsonOp) -> OpWire {
        match op {
            JsonOp::Set { path, value } => OpWire::Set {
                path: path.clone(),
                value: value.clone(),
            },
            JsonOp::SetCommand { path, value } => OpWire::SetCommand {
                path: path.clone(),
                value: value.clone(),
            },
            JsonOp::Push { path, value } => OpWire::Push {
                path: path.clone(),
                value: value.clone(),
            },
            JsonOp::HookEntry {
                event,
                matcher,
                value,
            } => OpWire::Hook {
                event: event.clone(),
                matcher: matcher.clone(),
                value: value.clone(),
            },
        }
    }
}

impl From<OpWire> for JsonOp {
    fn from(op: OpWire) -> JsonOp {
        match op {
            OpWire::Set { path, value } => JsonOp::Set { path, value },
            OpWire::SetCommand { path, value } => JsonOp::SetCommand { path, value },
            OpWire::Push { path, value } => JsonOp::Push { path, value },
            OpWire::Hook {
                event,
                matcher,
                value,
            } => JsonOp::HookEntry {
                event,
                matcher,
                value,
            },
        }
    }
}

impl From<&Claim> for ClaimWire {
    fn from(claim: &Claim) -> ClaimWire {
        match claim {
            Claim::Section { text, .. } => ClaimWire::Section { text: text.clone() },
            Claim::Json { ops, .. } => ClaimWire::Json {
                ops: ops.iter().map(OpWire::from).collect(),
            },
            Claim::Whole {
                bytes, executable, ..
            } => ClaimWire::Whole {
                bytes: bytes.clone(),
                executable: *executable,
            },
        }
    }
}

impl ClaimWire {
    /// The claim in the file at `path` (the record keeps it once, as the key).
    fn into_claim(self, path: String) -> Claim {
        match self {
            ClaimWire::Section { text } => Claim::Section { path, text },
            ClaimWire::Json { ops } => Claim::Json {
                path,
                ops: ops.into_iter().map(JsonOp::from).collect(),
            },
            ClaimWire::Whole { bytes, executable } => Claim::Whole {
                path,
                bytes,
                executable,
            },
        }
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
                    .map(|owned| OwnedWire {
                        engine: owned.engine.clone(),
                        kind: owned.kind,
                        claim: ClaimWire::from(&owned.claim),
                    })
                    .collect();
                let file = FileWire {
                    existed: file.existed,
                    original: file.original.clone(),
                    written: file.written.clone(),
                    claims,
                };
                (path.clone(), file)
            })
            .collect();
        let doc = Doc {
            schema_version: SCHEMA.to_string(),
            target: target.to_string(),
            files,
        };
        // A document of strings, numbers and maps with string keys cannot
        // fail to be written.
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        format!("{text}\n")
    }

    pub(crate) fn decode(file: &str, text: &str) -> Result<Record, SurfaceError> {
        let bad = |detail: &str| SurfaceError::Record {
            file: file.to_string(),
            detail: detail.to_string(),
        };
        let header: Header = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?;
        if header.schema_version.as_ref().and_then(Json::as_str) != Some(SCHEMA) {
            return Err(bad("its schema_version is not one this ctxloom reads"));
        }
        let doc: Doc = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?;
        let files = doc
            .files
            .into_iter()
            .map(|(path, file)| {
                let claims = file
                    .claims
                    .into_iter()
                    .map(|owned| Owned {
                        engine: owned.engine,
                        kind: owned.kind,
                        claim: owned.claim.into_claim(path.clone()),
                    })
                    .collect();
                let record = FileRecord {
                    existed: file.existed,
                    original: file.original,
                    written: file.written,
                    claims,
                };
                (path, record)
            })
            .collect();
        Ok(Record { files })
    }
}

#[cfg(test)]
#[path = "record.test.rs"]
mod tests;
