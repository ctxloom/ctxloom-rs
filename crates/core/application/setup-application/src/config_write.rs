//! `util config-write`: merge a JSON patch into a FOREIGN config file, one a
//! user or another tool authored, that ctxloom edits and must hand back
//! intact (Go's `util_config_write.go`).
//!
//! - the file must be an absolute path (never `~`, never `$`);
//! - a missing file starts from an empty object; one that does not parse stops
//!   the command and is never overwritten;
//! - objects merge key by key, keeping every foreign key the patch does not
//!   mention; any other value is replaced;
//! - ctxloom records what it applied, and the NEXT write takes that back out
//!   before it applies the new one: a second write replaces ctxloom's entry
//!   instead of piling up beside it;
//! - after writing, the file is read back and the patch confirmed present.
//!
//! plan supersedes Go (8.2): JSON targets only. A TOML target is refused by
//! name until that format is ported. The document is read and written back
//! (digits and member order kept), not spliced into the file's bytes, so its
//! whitespace is normalized to two-space indentation.

use serde::Serialize;
use surface_ports::Json;

use crate::{Setup, SetupError};

/// What a write did.
#[derive(Clone, Debug, Serialize)]
pub struct ConfigWritten {
    pub file: String,
    pub filetype: String,
    pub created: bool,
    #[serde(rename = "mergedKeys")]
    pub merged_keys: Vec<String>,
    pub verified: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub record: String,
}

fn refused(detail: impl Into<String>) -> SetupError {
    SetupError::ConfigWrite {
        detail: detail.into(),
    }
}

/// One leaf ctxloom set: where, and what was there.
struct Applied {
    path: Vec<String>,
    previous: Option<Json>,
}

fn at<'a>(doc: &'a Json, path: &[String]) -> Option<&'a Json> {
    path.iter().try_fold(doc, |here, key| here.get(key))
}

fn parent_mut<'a>(doc: &'a mut Json, path: &[String]) -> Option<&'a mut Json> {
    path.iter().try_fold(doc, |here, key| here.get_mut(key))
}

/// Merge `patch` into `base`, recording each leaf set.
fn merge(base: &mut Json, patch: &Json, path: &mut Vec<String>, applied: &mut Vec<Applied>) {
    let Json::Object(members) = patch else {
        return;
    };
    for (key, value) in members {
        path.push(key.clone());
        let both_objects =
            matches!(value, Json::Object(_)) && matches!(base.get(key), Some(Json::Object(_)));
        if both_objects {
            if let Some(inner) = base.get_mut(key) {
                merge(inner, value, path, applied);
            }
        } else {
            applied.push(Applied {
                path: path.clone(),
                previous: base.get(key).cloned(),
            });
            base.set(key, value.clone());
        }
        path.pop();
    }
}

/// Take back what a record says was applied, last first; objects left empty
/// by it go too.
fn reverse(doc: &mut Json, applied: &[Applied]) {
    for step in applied.iter().rev() {
        let Some((last, parents)) = step.path.split_last() else {
            continue;
        };
        if let Some(parent) = parent_mut(doc, parents) {
            match &step.previous {
                Some(previous) => {
                    parent.set(last, previous.clone());
                }
                None => {
                    parent.remove(last);
                }
            }
        }
        for depth in (1..=parents.len()).rev() {
            let (key, above) = parents[..depth].split_last().expect("non-empty");
            if let Some(parent) = parent_mut(doc, above) {
                if matches!(parent.get(key), Some(Json::Object(m)) if m.is_empty()) {
                    parent.remove(key);
                }
            }
        }
    }
}

fn encode(applied: &[Applied]) -> String {
    let steps = applied
        .iter()
        .map(|a| {
            Json::object([
                ("path", Json::strings(&a.path)),
                ("previous", a.previous.clone().unwrap_or(Json::Null)),
                ("existed", Json::Bool(a.previous.is_some())),
            ])
        })
        .collect();
    format!(
        "{}\n",
        Json::object([("applied", Json::Array(steps))]).pretty()
    )
}

fn decode(text: &str) -> Option<Vec<Applied>> {
    let doc = Json::parse(text).ok()?;
    doc.get("applied")?
        .as_array()?
        .iter()
        .map(|step| {
            let path = step
                .get("path")?
                .as_array()?
                .iter()
                .map(|k| k.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()?;
            let existed = matches!(step.get("existed"), Some(Json::Bool(true)));
            Some(Applied {
                path,
                previous: existed.then(|| step.get("previous").cloned()).flatten(),
            })
        })
        .collect()
}

impl Setup {
    /// Merge `patch` (a JSON object) into the foreign config file `file`.
    pub fn config_write(
        &self,
        file: &str,
        filetype: Option<&str>,
        patch: &str,
    ) -> Result<ConfigWritten, SetupError> {
        let foreign = self
            .foreign
            .as_ref()
            .ok_or_else(|| refused("this build has no foreign files to write"))?;
        if file.is_empty() {
            return Err(refused("--file is required"));
        }
        if !file.starts_with('/') {
            return Err(refused(format!("--file must be an absolute, already-resolved path (got {file:?}): this command never expands $HOME or guesses a location")));
        }
        if file.contains(['~', '$']) {
            return Err(refused(format!(
                "--file must not contain an unexpanded ~ or $ reference (got {file:?})"
            )));
        }
        let kind = match filetype {
            Some(kind) => kind.to_string(),
            None => file
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_lowercase())
                .unwrap_or_default(),
        };
        if kind != "json" {
            return Err(refused(format!(
                "{file}: only json files can be written to (a {} target is not ported yet); pass --filetype json",
                if kind.is_empty() { "this" } else { &kind }
            )));
        }
        if patch.trim().is_empty() {
            return Err(refused("empty patch on stdin: refusing a no-op write (pass a JSON object with at least one key)"));
        }
        let patch_doc = Json::parse(patch)
            .ok()
            .filter(|p| matches!(p, Json::Object(_)))
            .ok_or_else(|| refused("the patch on stdin is not a JSON object"))?;
        if matches!(&patch_doc, Json::Object(m) if m.is_empty()) {
            return Err(refused(
                "the patch is an empty JSON object: refusing a no-op write",
            ));
        }

        let held = foreign.read(file).map_err(SetupError::cause)?;
        let created = held.is_none();
        let mut doc = match held.as_deref() {
            None => Json::Object(Vec::new()),
            Some(bytes) if bytes.iter().all(u8::is_ascii_whitespace) => Json::Object(Vec::new()),
            Some(bytes) => String::from_utf8(bytes.to_vec())
                .ok()
                .and_then(|t| Json::parse(&t).ok())
                .filter(|d| matches!(d, Json::Object(_)))
                .ok_or_else(|| {
                    refused(format!("{file} is not valid json: refusing to overwrite a file this command couldn't parse (fix or remove it by hand, then retry)"))
                })?,
        };
        let before = doc.clone();

        // Take back what the last write put here, then apply this one.
        if let Some(record) = foreign.record(file).map_err(SetupError::cause)? {
            let previous = decode(&record).ok_or_else(|| {
                refused(format!("the record of the last write to {file} cannot be read, so ctxloom cannot take its old entry back out; refusing to write"))
            })?;
            reverse(&mut doc, &previous);
        }
        let mut applied = Vec::new();
        merge(&mut doc, &patch_doc, &mut Vec::new(), &mut applied);

        if doc != before || created {
            foreign
                .write(file, format!("{}\n", doc.pretty()).as_bytes())
                .map_err(SetupError::cause)?;
        }
        // Payload verification, not just a clean write.
        let reread = foreign
            .read(file)
            .map_err(SetupError::cause)?
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|t| Json::parse(&t).ok());
        let verified = reread
            .as_ref()
            .is_some_and(|d| applied.iter().all(|a| at(d, &a.path) == at(&doc, &a.path)));
        if !verified {
            return Err(refused(format!("{file} does not contain the merged content after writing (verify failed): inspect it by hand")));
        }
        let record = foreign
            .write_record(file, &encode(&applied))
            .map_err(SetupError::cause)?;
        let mut keys: Vec<String> = match &patch_doc {
            Json::Object(m) => m.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        };
        keys.sort();
        Ok(ConfigWritten {
            file: file.to_string(),
            filetype: kind,
            created,
            merged_keys: keys,
            verified,
            record,
        })
    }
}

#[cfg(test)]
#[path = "config_write.test.rs"]
mod tests;
