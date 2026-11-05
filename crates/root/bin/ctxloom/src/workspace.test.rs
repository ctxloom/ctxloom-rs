//! The workspace's own invariants: the lock holds one of each angzarr crate,
//! and no commit position is anywhere in the sources.

use std::path::Path;

/// angzarr crates a lock must hold exactly once, whatever pulls them in:
/// taskloom's `tasks-lib` brings its own graph, so the pins must agree.
const ONE_OF_EACH: [&str; 5] = [
    "angzarr-proto",
    "angzarr-ports",
    "angzarr-diagnostic",
    "angzarr-embedded",
    // ctxloom-lib's: its own angzarr pins are ours, so it must not be held
    // at two commits (one from a transitive consumer).
    "cli-support",
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

/// The git-sourced crates the lock holds more than once, as of this pin. Each
/// is a duplicate to be gone when the pins agree (`angzarr-embedded-host-config`
/// comes in at two angzarr-embedded revs: ours, and the one taskloom's graph
/// was built on). The list names what is, so a NEW duplicate fails the test and
/// a duplicate that goes (so its entry is stale) fails it too.
const KNOWN_DUPLICATED: [&str; 1] = ["angzarr-embedded-host-config"];

/// Every package of the lock that comes from git, by name, each with the
/// source it comes from.
fn git_packages(lock: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for block in lock.split("\n\n") {
        let field = |key: &str| {
            block
                .lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(" = \""))
                .and_then(|v| v.strip_suffix('"'))
        };
        if let (Some(name), Some(source)) = (field("name"), field("source")) {
            if source.starts_with("git+") {
                found.push((name.to_string(), source.to_string()));
            }
        }
    }
    found
}

#[test]
fn test_every_git_pinned_crate_is_in_the_lock_once_but_the_known_duplicates() {
    let lock = include_str!("../../../../../Cargo.lock");
    let packages = git_packages(lock);
    assert!(
        packages.len() > 20,
        "the lock holds git crates: {}",
        packages.len()
    );
    let mut names: Vec<&str> = packages.iter().map(|(n, _)| n.as_str()).collect();
    names.sort_unstable();
    let mut twice: Vec<&str> = names
        .windows(2)
        .filter(|w| w[0] == w[1])
        .map(|w| w[0])
        .collect();
    twice.dedup();
    assert_eq!(
        twice, KNOWN_DUPLICATED,
        "git crates held more than once (a new one, or a known one gone)"
    );
    // a duplicate is never of one source twice: it is two revs
    for name in KNOWN_DUPLICATED {
        let mut sources: Vec<&str> = packages
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, s)| s.as_str())
            .collect();
        sources.sort_unstable();
        sources.dedup();
        assert!(sources.len() > 1, "{name}: {sources:?}");
    }
}

#[test]
fn test_the_git_pins_of_the_one_of_each_crates_are_the_pins_of_one_repository_each() {
    let lock = include_str!("../../../../../Cargo.lock");
    let packages = git_packages(lock);
    for name in ONE_OF_EACH {
        let sources: Vec<&str> = packages
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, s)| s.as_str())
            .collect();
        assert_eq!(sources.len(), 1, "{name}: {sources:?}");
        assert!(
            sources[0].contains("github.com/angzarr-io/")
                || sources[0].contains("github.com/ctxloom/ctxloom-lib"),
            "{name}: {}",
            sources[0]
        );
    }
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
