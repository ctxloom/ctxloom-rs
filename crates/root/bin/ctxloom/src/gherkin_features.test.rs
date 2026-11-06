//! Feature discovery: every `.feature` file under `features/` is run, with the
//! World its `# world:` header line names (`gherkin_common::world_of`). Adding
//! a feature needs no hand-written test; `gherkin_lint.test.rs` has the
//! convention and the parity lint. A `@pending` scenario (Go's, not yet
//! ported: tagged on itself, its Rule or its Feature) is left out of the run;
//! a feature tagged `@pending` as a whole has no World and is not run at all.
//!
//! The features run concurrently, a thread each up to the machine's cores
//! (each cucumber run also runs 4 scenarios at once). `GHERKIN_ONLY=<text>`
//! runs only the features whose path contains it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::gherkin_common::{feature_files, features_dir, is_pending_feature, world_of};
use super::host_var;
use crate::tests::files;

/// Run one feature with its World; whether it passed. A pending feature is
/// skipped; a pending scenario is left out by the World's run
/// (`gherkin_common::run_feature`).
fn run_one(rel: &str) -> Result<(), String> {
    let path = features_dir().join(rel);
    let text = files::read(&path).map_err(|e| format!("{rel}: {e}"))?;
    if is_pending_feature(&text) {
        return Ok(());
    }
    let passed = match world_of(&text).map_err(|e| format!("{rel}: {e}"))? {
        "base" => super::gherkin::run(&path),
        "surfaces" => super::gherkin_surfaces::run(&path),
        "isolation" => super::gherkin_isolation::run(&path),
        other => return Err(format!("{rel}: unknown world {other}")),
    };
    if passed {
        Ok(())
    } else {
        Err(format!("{rel}: a scenario failed (its output is above)"))
    }
}

#[test]
fn every_feature_runs() {
    let only = host_var("GHERKIN_ONLY")
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    let features: Vec<String> = feature_files()
        .into_iter()
        .filter(|f| f.contains(&only))
        .collect();
    assert!(!features.is_empty(), "no feature matches {only:?}");
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4)
        .min(features.len());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(rel) = features.get(i) else { break };
                if let Err(e) = run_one(rel) {
                    failures.lock().expect("not poisoned").push(e);
                }
            });
        }
    });
    let mut failures = failures.into_inner().expect("not poisoned");
    failures.sort();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A feature whose other scenarios run: its `@pending` scenarios, and a
/// `@pending` Rule's, are left out, though no World has their steps.
#[test]
fn a_pending_scenario_is_left_out_and_its_neighbours_run() {
    let project = super::library::Project::bare();
    let feature = |tag: &str| {
        format!(
            "Feature: f\n  Scenario: runs\n    Given an empty project directory\n\n  {tag}\n  Scenario: left out\n    Given a step no World has\n\n  {tag}\n  Rule: r\n    Scenario: left out too\n      Given a step no World has\n"
        )
    };
    let path = project.dir().join("f.feature");
    files::put(&path, feature("@pending"));
    assert!(super::gherkin::run(&path), "the pending scenarios ran");
    files::put(&path, feature("@other"));
    assert!(
        !super::gherkin::run(&path),
        "a step no World has passed: the check above proves nothing"
    );
}
