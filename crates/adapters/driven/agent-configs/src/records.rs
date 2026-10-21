//! Go taskloom's memory of what it wrote into a configuration: its
//! application records (hew's §9.7 records, YAML) under
//! `~/.ctxloom/records/taskloom/`, one live record per file, named for the
//! file by Go's `paths.FlatName`. Read, never written, and only for one
//! fact: whether Go's last write made the `mcpServers` object itself. If it
//! did, Go's own uninstall takes the whole object back out, and a later Go
//! install expects it gone; an uninstall here does the same, so the two
//! binaries can take turns on one file.

use std::path::Path;

pub use file_lock::path::flat_name;
use serde::Deserialize;

/// The record directory's name under `~/.ctxloom/records`: taskloom's own,
/// apart from ctxloom's (Go's `recordsSubdir`).
pub const TASKLOOM_RECORDS: &str = "taskloom";

/// A record file's suffix.
const SUFFIX: &str = ".hew-record.yaml";

#[derive(Deserialize, Default)]
struct Record {
    #[serde(default)]
    applied_at: String,
    #[serde(default)]
    targets: Vec<Target>,
}

#[derive(Deserialize, Default)]
struct Target {
    #[serde(default)]
    target: String,
    #[serde(default)]
    transforms: Vec<Op>,
}

#[derive(Deserialize, Default)]
struct Op {
    #[serde(default)]
    op: String,
    #[serde(default)]
    path: String,
}

/// Whether Go's newest record for `target` (the path as Go names the
/// file) in `dir` says it added `pointer` whole. A record that cannot be
/// read says nothing.
pub fn added(dir: &Path, target: &str, pointer: &str) -> bool {
    let prefix = format!("{}__", flat_name(target));
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let newest = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (name.starts_with(&prefix) && name.ends_with(SUFFIX)).then_some(name)
        })
        .filter_map(|name| {
            let text = std::fs::read_to_string(dir.join(&name)).ok()?;
            let record: Record = serde_norway::from_str(&text).ok()?;
            Some((record.applied_at.clone(), name, record))
        })
        .max_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    let Some((_, _, record)) = newest else {
        return false;
    };
    record
        .targets
        .iter()
        .filter(|t| t.target == target)
        .flat_map(|t| &t.transforms)
        .any(|op| op.op == "add" && op.path == pointer)
}

#[cfg(test)]
#[path = "records.test.rs"]
mod tests;
