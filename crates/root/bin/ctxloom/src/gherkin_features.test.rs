//! Feature discovery: every `.feature` file under `features/` is run, with the
//! World its `# world:` header line names (`gherkin_common::world_of`). Adding
//! a feature needs no hand-written test; `gherkin_lint.test.rs` has the
//! convention and the parity lint. A `@pending` feature (Go's scenarios not
//! yet ported, `<name>.pending.feature`) has no World and is not run.
//!
//! The features run concurrently, a thread each up to the machine's cores
//! (each cucumber run also runs 4 scenarios at once). `GHERKIN_ONLY=<text>`
//! runs only the features whose path contains it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::gherkin_common::{feature_files, features_dir, is_pending, world_of};
use super::host_var;
use crate::tests::files;

/// Run one feature with its World; whether it passed. A pending feature is
/// skipped.
fn run_one(rel: &str) -> Result<(), String> {
    let path = features_dir().join(rel);
    let text = files::read(&path).map_err(|e| format!("{rel}: {e}"))?;
    if is_pending(&text) {
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
