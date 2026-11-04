//! A bundle's tree as it will be: the files it holds now, the changes made to
//! them so far, and the tree they leave. A command plans its writes here,
//! checks that the tree it leaves still reads, and only then hands the changes
//! to the port; so a refused write has changed nothing, and an edit that
//! changes nothing has nothing to hand.

use std::collections::BTreeSet;

use ctxloom_ports::{Change, Put};
use library::tree::Files;

/// A tree being changed.
pub(crate) struct Plan {
    files: Files,
    exec: BTreeSet<String>,
    changes: Vec<Change>,
}

impl Plan {
    /// The plan over a tree and the files of it that are executable.
    pub(crate) fn of(files: Files, exec: BTreeSet<String>) -> Plan {
        Plan {
            files,
            exec,
            changes: Vec::new(),
        }
    }

    /// The tree as it will be.
    pub(crate) fn files(&self) -> &Files {
        &self.files
    }

    /// The text of a file, if it is there.
    pub(crate) fn text(&self, path: &str) -> Option<String> {
        self.files
            .get(path)
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
    }

    /// Whether a file is there.
    pub(crate) fn has(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Whether the file is executable.
    pub(crate) fn is_executable(&self, path: &str) -> bool {
        self.exec.contains(path)
    }

    /// The paths below `dir` (a directory's name, without the slash).
    pub(crate) fn below(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{dir}/");
        self.files
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .cloned()
            .collect()
    }

    /// A plain file written whole.
    pub(crate) fn write(&mut self, path: &str, bytes: impl Into<Vec<u8>>) {
        self.put(path, bytes.into(), false);
    }

    /// A file written whole, executable or not. A write that leaves the file
    /// as it is no change.
    pub(crate) fn put(&mut self, path: &str, bytes: Vec<u8>, executable: bool) {
        let same = self.files.get(path) == Some(&bytes) && self.exec.contains(path) == executable;
        if same {
            return;
        }
        self.files.insert(path.to_string(), bytes.clone());
        if executable {
            self.exec.insert(path.to_string());
        } else {
            self.exec.remove(path);
        }
        self.changes.push(Change::Write(Put {
            path: path.to_string(),
            bytes,
            executable,
        }));
    }

    /// A file taken away; whether it was there (no change where it was not).
    pub(crate) fn remove(&mut self, path: &str) -> bool {
        if self.files.remove(path).is_none() {
            return false;
        }
        self.exec.remove(path);
        self.changes.push(Change::Remove {
            path: path.to_string(),
        });
        true
    }

    /// A directory taken away with everything in it; whether anything was
    /// there (no change where nothing was).
    pub(crate) fn remove_dir(&mut self, dir: &str) -> bool {
        let gone = self.below(dir);
        if gone.is_empty() {
            return false;
        }
        for path in &gone {
            self.files.remove(path);
            self.exec.remove(path);
        }
        self.changes.push(Change::RemoveDir {
            path: dir.to_string(),
        });
        true
    }

    /// Whether nothing was changed.
    pub(crate) fn is_unchanged(&self) -> bool {
        self.changes.is_empty()
    }

    /// The changes, in the order made.
    pub(crate) fn into_changes(self) -> Vec<Change> {
        self.changes
    }
}

#[cfg(test)]
#[path = "plan.test.rs"]
mod tests;
