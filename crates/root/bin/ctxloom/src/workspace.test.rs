//! The workspace's own invariants: the lock holds one of each angzarr crate,
//! and no commit position is anywhere in the sources.

use std::path::Path;

/// angzarr crates a lock must hold exactly once, whatever pulls them in:
/// taskloom's `tasks-lib` brings its own graph, so the pins must agree.
const ONE_OF_EACH: [&str; 4] = [
    "angzarr-proto",
    "angzarr-ports",
    "angzarr-diagnostic",
    "angzarr-embedded",
];

#[test]
fn test_the_lock_holds_one_of_each_angzarr_crate() {
    let lock = include_str!("../../../../../Cargo.lock");
    let mut wrong = Vec::new();
    for name in ONE_OF_EACH {
        let held = lock
            .lines()
            .filter(|line| *line == format!("name = \"{name}\""))
            .count();
        if held != 1 {
            wrong.push(format!("{name}: {held} in the lock"));
        }
    }
    assert!(wrong.is_empty(), "one of each is required: {wrong:?}");
}

/// The four names no source may carry (the store has no commit position:
/// ctxloom reads by stream position). Spelled in pieces so this file does
/// not hold them itself, which keeps the scan free of any exclusion.
fn banned() -> [String; 4] {
    [
        concat!("commit", "_position"),
        concat!("Commit", "Position"),
        concat!("fork", "_point"),
        concat!("global", "_position"),
    ]
    .map(String::from)
}

/// Every file under `dir`, but build output and VCS state.
fn files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        if path.is_dir() {
            files(&path, found);
        } else {
            found.push(path);
        }
    }
}

/// Exclusions, stated: `target/` and `.git/` (build output and VCS state)
/// and files that are not UTF-8 (SQLite fixtures). Nothing else: the `crates`
/// and `proto` trees, every file in them, this one included.
#[test]
fn test_no_store_wide_position_is_anywhere_in_the_workspace_sources() {
    let root = super::repo_root();
    let mut all = Vec::new();
    for tree in ["crates", "proto"] {
        files(&root.join(tree), &mut all);
    }
    assert!(!all.is_empty(), "scanned nothing under {}", root.display());
    let mut hits = Vec::new();
    for path in all {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for term in banned() {
            if text.contains(&term) {
                hits.push(format!("{}: {term}", path.display()));
            }
        }
    }
    assert!(hits.is_empty(), "a commit position is back: {hits:#?}");
}

#[test]
fn test_annals_has_left_the_workspace() {
    let output = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--offline"])
        .arg("--manifest-path")
        .arg(super::repo_root().join("Cargo.toml"))
        .output()
        .expect("cargo runs");
    assert!(output.status.success(), "cargo metadata failed");
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    let annals: Vec<&str> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|p| p["name"].as_str())
        .filter(|name| name.starts_with("annals"))
        .collect();
    assert!(annals.is_empty(), "still in the graph: {annals:?}");
}
