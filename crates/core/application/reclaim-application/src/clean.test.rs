//! The clean's plan and its application. Go: `CleanCache` (release/0.7).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use ctxloom_ports::{Failure, ProjectCache};

use crate::Cleaner;

#[derive(Default)]
struct Mem {
    trees: Mutex<BTreeMap<String, u64>>,
}

impl ProjectCache for Mem {
    fn app_dir(&self) -> String {
        "/work/.ctxloom".to_string()
    }

    fn measure(&self, rel: &str) -> Result<Option<u64>, Failure> {
        Ok(self.trees.lock().unwrap().get(rel).copied())
    }

    fn remove(&self, rel: &str) -> Result<(), Failure> {
        self.trees.lock().unwrap().remove(rel);
        Ok(())
    }
}

fn cleaner(trees: &[(&str, u64)]) -> (Arc<Mem>, Cleaner) {
    let mem = Arc::new(Mem::default());
    for (rel, bytes) in trees {
        mem.trees.lock().unwrap().insert(rel.to_string(), *bytes);
    }
    (mem.clone(), Cleaner::new(mem))
}

#[test]
fn a_report_removes_nothing_and_names_each_rebuild() {
    let (mem, cleaner) = cleaner(&[("cache/context", 10), ("cache/repos", 5)]);
    let report = cleaner.clean(false).unwrap();
    assert!(!report.applied);
    assert_eq!(report.bytes, 15);
    assert_eq!(mem.trees.lock().unwrap().len(), 2);
    let context = &report.targets[2];
    assert_eq!(context.rel, ".ctxloom/cache/context");
    assert!(context.present && !context.removed);
    assert_eq!(context.rebuild, "ctxloom run (a launch rewrites it)");
}

#[test]
fn an_absent_target_is_reported_absent() {
    let (_, cleaner) = cleaner(&[]);
    let report = cleaner.clean(true).unwrap();
    assert_eq!(report.targets.len(), 3);
    assert!(report.targets.iter().all(|t| !t.present && !t.removed));
    assert_eq!(report.bytes, 0);
}

#[test]
fn yes_removes_what_it_reported() {
    let (mem, cleaner) = cleaner(&[("cache/context", 10), ("cache/bundles", 1)]);
    let report = cleaner.clean(true).unwrap();
    assert!(report.applied);
    assert!(mem.trees.lock().unwrap().is_empty());
    assert_eq!(report.targets.iter().filter(|t| t.removed).count(), 2);
}
