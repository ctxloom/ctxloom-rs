//! Acceptance: `taskloom import` agrees with Go's fold, task by task.
//!
//! Go's fold is the oracle, dumped as JSON by `scripts/go-fold`'s harness
//! (`scripts/compare-go-import.sh`) from Go's own `eventLog.fold`. The
//! fixtures (`fixtures/go-logs`) are synthetic logs, one per kind of record
//! the importer meets, and `fixtures/go-fold.json` is Go's fold of them.
//! The same comparison runs over real logs, given by path, in an ignored
//! test: the logs are copied into a temporary home first, so it never
//! touches a real one.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use annals::next::{AsyncEventStore, Stream, Timestamp};
use ports::{Project, TaskStores};
use serde_json::Value;
use task_store_sqlite::SqliteStores;
use tasks::{TaskState, Tasks};
use tempfile::TempDir;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::{binary, Ran};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// A temporary home holding a copy of every log in `logs`.
fn home_with(logs: &Path) -> TempDir {
    let home = TempDir::new().unwrap();
    let tasks = home.path().join(".ctxloom/tasks");
    fs::create_dir_all(&tasks).unwrap();
    for entry in fs::read_dir(logs).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            fs::copy(&path, tasks.join(path.file_name().unwrap())).unwrap();
        }
    }
    home
}

/// `taskloom import --json` in `home`, from a directory that is no
/// project.
fn import(home: &Path) -> (Ran, Value) {
    let cwd = TempDir::new().unwrap();
    let output = Command::new(binary())
        .args(["import", "--json"])
        .current_dir(cwd.path())
        .env_clear()
        .env("HOME", home)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .output()
        .expect("taskloom runs");
    let ran = Ran::of(output);
    let report = ran.json();
    (ran, report)
}

/// Every task in the project's store, by harp; `None` when it has none.
fn rust_tasks(home: &Path, project: &str) -> Option<BTreeMap<String, TaskState>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut stores = SqliteStores::default();
        let project = Project {
            id: project.to_string(),
            root: String::new(),
            store: home
                .join(".ctxloom/projects")
                .join(project)
                .join("state.db"),
        };
        let store = stores.open_existing(&project).await.unwrap()?;
        let registry = tasks_application::registry();
        let mut tasks = BTreeMap::new();
        for cover in store.covers("tasks").await.unwrap() {
            let slice = store.read_from(&cover, 0).await.unwrap().unwrap();
            let stream = Stream::<Tasks>::replay(cover, &slice.pages, &registry).unwrap();
            let state = stream.state().clone();
            tasks.insert(state.harp_id.clone(), state);
        }
        Some(tasks)
    })
}

fn instant(value: &Value) -> Option<Timestamp> {
    let text = value.as_str()?;
    let at = OffsetDateTime::parse(text, &Rfc3339).expect("Go writes RFC 3339");
    Some(Timestamp {
        seconds: at.unix_timestamp(),
        nanos: i32::try_from(at.nanosecond()).unwrap(),
    })
}

/// How one project's import compares with Go's fold of it.
#[derive(Debug, Default)]
struct Compared {
    /// Tasks Go's fold holds, removed ones included.
    go_tasks: usize,
    /// Tasks that agree on every field.
    agreeing: usize,
    /// The tags of the live tasks in Go's fold, each task's counted.
    go_tags: usize,
    /// Those of them that are not tags in tagma's grammar: the import
    /// reports each and leaves it out.
    unparseable_tags: usize,
    /// `<harp>: <field>` for every field that disagrees: values are left
    /// out, since a real log's are private.
    mismatches: Vec<String>,
}

/// Compare Go's fold of one project with the store the import left.
fn compare(go: &Value, rust: &BTreeMap<String, TaskState>) -> Compared {
    let mut compared = Compared::default();
    let mut seen = BTreeSet::new();
    for task in go["tasks"].as_array().unwrap() {
        compared.go_tasks += 1;
        let harp = task["harp_id"].as_str().unwrap();
        seen.insert(harp.to_string());
        let Some(state) = rust.get(harp) else {
            compared.mismatches.push(format!("{harp}: absent"));
            continue;
        };
        let mut fields = Vec::new();
        if task["removed"].as_bool().unwrap() {
            if !state.removed {
                fields.push("removed");
            }
        } else {
            // Go's tags as tagma parses them, each in its canonical
            // spelling; one that does not parse is left out, as the import
            // leaves it out.
            let go_tags = task["tags"].as_array().unwrap();
            compared.go_tags += go_tags.len();
            let tags: BTreeSet<String> = go_tags
                .iter()
                .filter_map(|t| tagma_core::Tag::parse(t.as_str().unwrap()).ok())
                .map(|t| t.to_string())
                .collect();
            compared.unparseable_tags += go_tags
                .iter()
                .filter(|t| tagma_core::Tag::parse(t.as_str().unwrap()).is_err())
                .count();
            let tags: Vec<String> = tags.into_iter().collect();
            let ours: Vec<String> = state.tags.iter().map(ToString::to_string).collect();
            let checks = [
                ("removed", !state.removed),
                ("text", task["text"] == state.text),
                ("status", task["status"] == state.status),
                ("trigger", task["trigger"] == state.trigger),
                (
                    "origin_session",
                    task["origin_session"] == state.origin_session,
                ),
                ("tags", tags == ours),
                (
                    "created_at",
                    instant(&task["created_at"]) == Some(state.created_at),
                ),
                (
                    "status_set_at",
                    instant(&task["status_set_at"]) == Some(state.status_set_at),
                ),
                (
                    "deferred_since",
                    instant(&task["deferred_since"]) == state.deferred_since,
                ),
            ];
            fields.extend(checks.iter().filter(|(_, ok)| !ok).map(|(f, _)| *f));
        }
        if fields.is_empty() {
            compared.agreeing += 1;
        }
        compared
            .mismatches
            .extend(fields.iter().map(|f| format!("{harp}: {f}")));
    }
    for harp in rust.keys().filter(|h| !seen.contains(*h)) {
        compared
            .mismatches
            .push(format!("{harp}: not in Go's fold"));
    }
    compared
}

/// The import's report of `project`.
fn reported<'r>(report: &'r Value, project: &str) -> &'r Value {
    report["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["project_id"] == project)
        .unwrap_or_else(|| panic!("{project} reported"))
}

/// `(line, harp, why's code)` of every record the import left behind.
fn lost(project: &Value) -> Vec<(u64, String, String)> {
    project["lost"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["line"].as_u64().unwrap(),
                l["harp_id"].as_str().unwrap().to_string(),
                l["why"]["code"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// Compare every project of Go's fold `go` with what importing its logs
/// into `home` left, checking on the way that the import reported what
/// Go's fold could not give it. Mismatches by project, and the comparisons.
fn check(go: &Value, home: &Path, report: &Value) -> BTreeMap<String, Compared> {
    let mut all = BTreeMap::new();
    for (project, fold) in go.as_object().unwrap() {
        let ours = reported(report, project);
        let rust = rust_tasks(home, project).unwrap_or_default();
        if let Some(error) = fold.get("error") {
            assert!(
                !ours["failed"].is_null(),
                "{project}: Go cannot fold it ({error}); the import must stop too"
            );
            assert!(rust.is_empty(), "{project}: nothing imported");
            continue;
        }
        assert!(ours["failed"].is_null(), "{project}: {}", ours["failed"]);
        // Every anomaly Go's fold met is accounted for: repaired, or
        // reported displaced.
        let anomalies = fold["anomalies"].as_array().unwrap();
        let repaired = anomalies.iter().filter(|a| a["repaired"] == true).count();
        assert_eq!(ours["repaired"], repaired, "{project}: repaired");
        let displaced: Vec<&str> = anomalies
            .iter()
            .filter(|a| a["repaired"] == false)
            .map(|a| a["harp_id"].as_str().unwrap())
            .collect();
        let reported_displaced: Vec<String> = lost(ours)
            .into_iter()
            .filter(|(_, _, why)| why == "tasks-application-import-displaced")
            .map(|(_, harp, _)| harp)
            .collect();
        assert_eq!(reported_displaced, displaced, "{project}: displaced adds");
        all.insert(project.clone(), compare(fold, &rust));
    }
    all
}

/// Importing again changes nothing and reports everything already there.
fn assert_idempotent(home: &Path, first: &Value) {
    let (_, again) = import(home);
    for (before, after) in first["projects"]
        .as_array()
        .unwrap()
        .iter()
        .zip(again["projects"].as_array().unwrap())
    {
        let project = &after["project_id"];
        assert_eq!(after["applied"], 0, "{project}: nothing applied again");
        assert_eq!(after["tasks"], 0, "{project}: no task added again");
        assert_eq!(
            after["already"], before["applied"],
            "{project}: every record found"
        );
        assert_eq!(after["lost"], before["lost"], "{project}: the same left");
    }
}

#[test]
fn test_the_import_agrees_with_gos_fold_of_every_fixture() {
    let go: Value =
        serde_json::from_str(&fs::read_to_string(fixtures().join("go-fold.json")).unwrap())
            .unwrap();
    let home = home_with(&fixtures().join("go-logs"));
    let (ran, report) = import(home.path());
    assert!(!ran.ok, "the fixtures leave records behind, so it fails");
    assert_eq!(report["complete"], false);
    let compared = check(&go, home.path(), &report);

    // Where Rust and Go disagree, the import said why: each record the
    // tasks domain refuses, and every later record to its task.
    let refusals = reported(&report, "domain-refusals");
    let code = |c: &str| c.to_string();
    let harp = |h: &str| h.to_string();
    assert_eq!(
        lost(refusals),
        [
            (1, harp("brisk-heron"), code("tasks-text-required")),
            (
                2,
                harp("brisk-heron"),
                code("tasks-application-import-task-not-imported")
            ),
            (3, harp("quiet-maple"), code("tasks-trigger-required")),
            (5, harp("tidy-otter"), code("tasks-status-reserved")),
            (6, harp("tidy-otter"), code("tasks-trigger-required")),
            (7, harp("tidy-otter"), code("tasks-text-required")),
            (10, harp(""), code("tasks-harp-id-required")),
        ]
    );
    assert_eq!(
        compared["domain-refusals"].mismatches,
        [
            ": absent",
            "brisk-heron: absent",
            "quiet-maple: absent",
            // A status Go kept untrimmed; Rust trims it. Not reported.
            "swift-lark: status",
            "tidy-otter: text",
            "tidy-otter: status",
            "tidy-otter: status_set_at",
            "tidy-otter: deferred_since",
        ]
    );
    let collisions = reported(&report, "harp-collision");
    assert_eq!(
        lost(collisions),
        [
            (
                2,
                harp("brisk-heron"),
                code("tasks-application-import-displaced")
            ),
            (
                6,
                harp("quiet-maple"),
                code("tasks-application-import-displaced")
            ),
        ]
    );
    // A Go tag outside tagma's grammar is reported, record by record, and
    // left out; the rest of the record imports, and `k="v"` is `k=v`.
    let unparseable = reported(&report, "unparseable-tags");
    let tag = code("tasks-application-tag-unparseable");
    let heron = || harp("brisk-heron");
    assert_eq!(
        lost(unparseable),
        [
            (1, heron(), tag.clone()),
            (1, heron(), tag.clone()),
            (2, heron(), tag.clone()),
            (3, heron(), tag.clone()),
            (4, heron(), tag.clone()),
        ]
    );
    assert_eq!(
        unparseable["lost"][1]["why"]["args"]["tag"], "area/cli",
        "each names its tag"
    );
    assert_eq!(compared["unparseable-tags"].go_tags, 5);
    assert_eq!(compared["unparseable-tags"].unparseable_tags, 3);
    for (project, compared) in &compared {
        if project != "domain-refusals" {
            assert_eq!(compared.mismatches, Vec::<String>::new(), "{project}");
            assert_eq!(compared.agreeing, compared.go_tasks, "{project}");
        }
    }
    assert!(compared["plain-history"].go_tasks >= 6);

    let counts = |p: &str, field: &str| reported(&report, p)[field].as_u64().unwrap();
    assert_eq!(counts("union-merged", "repeated"), 2);
    assert_eq!(counts("ignored-records", "ignored"), 4);
    assert_eq!(counts("ignored-records", "unchanged"), 3);
    assert_eq!(counts("repaired-collision", "tasks"), 2);
    assert_eq!(counts("empty", "records"), 0);
    assert!(
        rust_tasks(home.path(), "empty").is_none(),
        "no store for it"
    );
    let failed = |p: &str| reported(&report, p)["failed"]["code"].clone();
    assert_eq!(failed("malformed"), "go-task-log-malformed");
    assert_eq!(failed("unknown-op"), "go-task-log-unknown-op");
    assert!(
        !home.path().join(".ctxloom/projects/index.yaml").exists(),
        "an import binds no directory to a project"
    );

    assert_idempotent(home.path(), &report);
}

/// Over real logs: TASKLOOM_IMPORT_LOGS names a directory of Go logs
/// (copied again here, so it is only read), TASKLOOM_GO_FOLD Go's fold of
/// them. `scripts/compare-go-import.sh` sets both up from copies.
#[test]
#[ignore = "reads real logs, given by TASKLOOM_IMPORT_LOGS and TASKLOOM_GO_FOLD"]
fn test_the_import_agrees_with_gos_fold_of_real_logs() {
    let logs =
        PathBuf::from(std::env::var_os("TASKLOOM_IMPORT_LOGS").expect("TASKLOOM_IMPORT_LOGS"));
    let fold = PathBuf::from(std::env::var_os("TASKLOOM_GO_FOLD").expect("TASKLOOM_GO_FOLD"));
    let go: Value = serde_json::from_str(&fs::read_to_string(fold).unwrap()).unwrap();
    let home = home_with(&logs);
    let (ran, report) = import(home.path());
    let compared = check(&go, home.path(), &report);
    let mut mismatched = 0;
    let (mut tags, mut unparseable) = (0, 0);
    for (project, compared) in &compared {
        tags += compared.go_tags;
        unparseable += compared.unparseable_tags;
        let ours = reported(&report, project);
        println!(
            "{project}: {} Go tasks, {} agree; {} records, {} imported, {} repeated, {} unchanged, {} ignored, {} repaired, {} not imported",
            compared.go_tasks,
            compared.agreeing,
            ours["records"],
            ours["applied"],
            ours["repeated"],
            ours["unchanged"],
            ours["ignored"],
            ours["repaired"],
            ours["lost"].as_array().unwrap().len(),
        );
        for (line, harp, why) in lost(ours) {
            println!("  not imported: line {line}, {harp}: {why}");
        }
        for mismatch in &compared.mismatches {
            println!("  MISMATCH {mismatch}");
        }
        mismatched += compared.mismatches.len();
    }
    println!(
        "tags of live tasks: {tags}, {} parse, {unparseable} do not",
        tags - unparseable
    );
    assert_idempotent(home.path(), &report);
    assert_eq!(mismatched, 0, "every task agrees");
    // What is left behind is only what tagma cannot parse, if anything.
    let all_lost: Vec<String> = report["projects"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| lost(p).into_iter().map(|(_, _, why)| why))
        .collect();
    assert!(
        all_lost
            .iter()
            .all(|why| why == "tasks-application-tag-unparseable"),
        "everything else imported: {}",
        ran.stderr
    );
    assert_eq!(ran.ok, all_lost.is_empty());
}
