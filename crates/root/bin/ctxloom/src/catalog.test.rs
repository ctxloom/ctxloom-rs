//! The catalog test (CLAUDE.md, "Errors are localizable"): every diagnostic
//! code is prefixed with its crate's name, has an en-US message in that
//! crate's `locales/en-US/*.ftl`, and the message uses exactly the
//! diagnostic's args.
//!
//! Taskloom's form of this test calls each crate's `diagnostic_samples()`
//! and the binary's `catalog()`. S0 has not added the pins that give those
//! types (angzarr-diagnostic, angzarr-presentation), so this reads the same
//! facts from outside: the binary's `test diagnostics` (every sample, as the
//! envelope the binary renders from its catalog), `cargo metadata` for the
//! crates, and their catalogs' text.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

use super::{repo_root, run};
use crate::tests::files;

/// A crate of the dependency graph: its name and its directory.
struct Package {
    name: String,
    dir: PathBuf,
}

fn packages() -> Vec<Package> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--offline"])
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .output()
        .expect("cargo runs");
    assert!(output.status.success(), "cargo metadata failed");
    let meta: Value = serde_json::from_slice(&output.stdout).expect("metadata is JSON");
    meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .map(|p| Package {
            name: p["name"].as_str().expect("a name").to_string(),
            dir: PathBuf::from(p["manifest_path"].as_str().expect("a path"))
                .parent()
                .expect("a directory")
                .to_path_buf(),
        })
        .collect()
}

/// The crate `code` is prefixed with: the longest crate name `<name>-` begins it.
fn owner<'a>(code: &str, packages: &'a [Package]) -> Option<&'a Package> {
    packages
        .iter()
        .filter(|p| code.starts_with(&format!("{}-", p.name)))
        .max_by_key(|p| p.name.len())
}

/// The `$variables` of `code`'s message in the crate's en-US catalogs: the
/// message's own lines, up to the next message. None when no catalog has it.
/// A crate whose build prefixes its catalog with its name (ctxloom-lib's, as
/// taskloom's) writes the id without that prefix.
fn message_vars(package: &Package, code: &str) -> Option<BTreeSet<String>> {
    let bare = code.strip_prefix(&format!("{}-", package.name));
    let wanted: Vec<&str> = std::iter::once(code).chain(bare).collect();
    let catalogs = files::children(package.dir.join("locales/en-US"));
    for entry in catalogs {
        if entry.extension().is_none_or(|e| e != "ftl") {
            continue;
        }
        let text = files::read(&entry).ok()?;
        let mut lines = text
            .lines()
            .skip_while(|l| !wanted.iter().any(|id| l.starts_with(&format!("{id} ="))));
        let Some(first) = lines.next() else { continue };
        let rest = lines.take_while(|l| l.starts_with(' ') || l.is_empty());
        let block: Vec<&str> = std::iter::once(first).chain(rest).collect();
        return Some(vars(&block.join("\n")));
    }
    None
}

/// The names after each `$` in `text`.
fn vars(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        let name: String = rest[at + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if !name.is_empty() {
            found.insert(name);
        }
        rest = &rest[at + 1..];
    }
    found
}

fn diagnostics() -> Vec<Value> {
    let ran = run(&["test", "diagnostics"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    let all = ran.json();
    let all = all.as_array().expect("an array of envelopes").clone();
    assert!(!all.is_empty(), "the binary reports no diagnostic at all");
    all
}

#[test]
fn test_every_diagnostic_code_is_prefixed_with_its_crate_s_name() {
    let packages = packages();
    let unprefixed: Vec<String> = diagnostics()
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .filter(|code| owner(code, &packages).is_none())
        .collect();
    assert!(unprefixed.is_empty(), "no crate owns: {unprefixed:#?}");
}

#[test]
fn test_every_en_us_message_uses_exactly_its_args() {
    let packages = packages();
    let mut wrong = Vec::new();
    for d in diagnostics() {
        let code = d["code"].as_str().expect("a code");
        let args: BTreeSet<String> = d["args"]
            .as_object()
            .unwrap_or_else(|| panic!("{code}: args is not an object"))
            .keys()
            .cloned()
            .collect();
        let used = owner(code, &packages).and_then(|p| message_vars(p, code));
        if used.as_ref() != Some(&args) {
            wrong.push(format!("{code}: args {args:?}, message uses {used:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn test_every_diagnostic_renders_from_the_catalog_not_its_key() {
    for d in diagnostics() {
        let code = d["code"].as_str().expect("a code");
        let message = d["message"].as_str().expect("a message");
        assert!(
            !message.is_empty() && message != code,
            "{code}: {message:?}"
        );
        assert!(
            !message.contains('$') && !message.contains('{'),
            "{code}: an unfilled placeable in {message:?}"
        );
    }
}

#[test]
fn test_the_codes_the_binary_raises_are_among_its_diagnostics() {
    let known: BTreeSet<String> = diagnostics()
        .iter()
        .map(|d| d["code"].as_str().expect("a code").to_string())
        .collect();
    for args in [
        &["--format", "json", "test", "refuse"][..],
        &["--format", "json", "frobnicate"],
        &["--format", "json", "version", "--format", "yaml"],
    ] {
        let ran = run(args);
        let code = ran.err_json()["code"].as_str().expect("a code").to_string();
        assert!(known.contains(&code), "{args:?}: {code} has no sample");
    }
}

#[test]
fn test_the_binary_reports_the_diagnostics_of_every_crate_composed_in() {
    let packages = packages();
    let owners: BTreeSet<String> = diagnostics()
        .iter()
        .filter_map(|d| owner(d["code"].as_str()?, &packages).map(|p| p.name.clone()))
        .collect();
    // A crate with diagnostics joins the binary's samples (CLAUDE.md), and
    // this list: a crate left out of `diagnostic_samples()` is missed here.
    // "angzarr-ports" is the store's own error the opener carries
    // (`OpenError::Store`), sampled with the opener's.
    let composed = [
        "angzarr-ports",
        "cache-files",
        "config-files",
        "container-cli",
        "content-fs",
        "ctxloom-cli",
        "ctxloom-wiring",
        "deps",
        "deps-application",
        "duration-text",
        "editor-process",
        "hook-cli",
        "isolation",
        "isolation-application",
        "library",
        "library-application",
        "project-dir",
        "reclaim-application",
        "scheduler",
        "scheduler-loop",
        "session-cli",
        "session-files",
        "sessions",
        "sessions-application",
        "setup-application",
        "setup-cli",
        "sqlite-store-opener",
        "surface-ports",
        "tasks-ports",
        "worktree-git",
    ];
    assert_eq!(
        owners.iter().map(String::as_str).collect::<Vec<_>>(),
        composed
    );
}
