//! Acceptance: the real `taskloom` binary, run as separate processes on one
//! project's SQLite database, loses no write and duplicates no task id.
//! A port of Go's `tests/taskloom/crossprocess_test.go`.
//!
//! The tests live beside the code (no `tests/` directory), where cargo
//! does not build the binary for them; so the first test to need it builds
//! it, once, with the cargo running these tests, into the same target
//! directory and profile.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use serde_json::Value;
use tempfile::TempDir;

/// The `taskloom` binary, built once per test run.
fn binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        // This test executable is <target>/<profile>/deps/taskloom-<hash>.
        let exe = std::env::current_exe().expect("the test executable");
        let profile_dir = exe
            .parent()
            .and_then(Path::parent)
            .expect("<target>/<profile>");
        let target_dir = profile_dir.parent().expect("<target>");
        let mut build = Command::new(env!("CARGO"));
        build
            .args([
                "build",
                "--quiet",
                "--package",
                "taskloom",
                "--bin",
                "taskloom",
            ])
            .arg("--manifest-path")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .env("CARGO_TARGET_DIR", target_dir);
        if profile_dir.file_name().is_some_and(|p| p == "release") {
            build.arg("--release");
        }
        let status = build.status().expect("cargo runs");
        assert!(status.success(), "building taskloom failed");
        profile_dir.join("taskloom")
    })
}

/// An isolated home and project directory.
struct Env {
    home: TempDir,
    project: TempDir,
    session: &'static str,
}

/// One finished process.
struct Ran {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Ran {
    fn of(output: Output) -> Ran {
        Ran {
            ok: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {:?}", self.stdout))
    }
}

impl Env {
    /// A fresh home and project directory: no registry, no marker, no
    /// store. A test that is not about first sight seeds the project with a
    /// real `add` before any concurrency, as Go's tests do.
    fn new(session: &'static str) -> Env {
        Env {
            home: tempfile::tempdir().unwrap(),
            project: tempfile::tempdir().unwrap(),
            session,
        }
    }

    /// Run `taskloom args` in the project directory, with only HOME, PATH
    /// and the session in its environment. Safe to call concurrently.
    fn run(&self, args: &[&str]) -> Ran {
        self.run_with(args, &[])
    }

    /// [`Env::run`], with `vars` in its environment too.
    fn run_with(&self, args: &[&str], vars: &[(&str, &str)]) -> Ran {
        self.run_in(self.project.path(), args, vars)
    }

    /// [`Env::run_with`], in `dir`.
    fn run_in(&self, dir: &Path, args: &[&str], vars: &[(&str, &str)]) -> Ran {
        let output = Command::new(binary())
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("HOME", self.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("CTXLOOM_SESSION_HARP", self.session)
            .envs(vars.iter().copied())
            .output()
            .expect("taskloom runs");
        Ran::of(output)
    }

    /// The final listing, read by one process alone.
    fn list(&self) -> Vec<Value> {
        let ran = self.run(&["list", "--json"]);
        assert!(ran.ok, "list failed: {}", ran.stderr);
        ran.json().as_array().expect("an array").clone()
    }

    /// Add `text` alone, before any concurrency: the project is minted, its
    /// store created and migrated. The task's JSON.
    fn seed(&self, text: &str) -> Value {
        let added = self.run(&["add", text, "--format", "json"]);
        assert!(added.ok, "seed: {}", added.stderr);
        added.json()
    }

    /// The registry's entries, as `project_id` lines.
    fn registered(&self) -> Vec<String> {
        let index = self.home.path().join(".ctxloom/projects/index.yaml");
        std::fs::read_to_string(index)
            .expect("the registry")
            .lines()
            .filter(|l| {
                l.trim_start()
                    .trim_start_matches("- ")
                    .starts_with("project_id:")
            })
            .map(str::to_string)
            .collect()
    }

    /// The project stores under the home: one, or identities forked.
    fn stores(&self) -> Vec<PathBuf> {
        let projects = self.home.path().join(".ctxloom/projects");
        let mut stores: Vec<PathBuf> = std::fs::read_dir(projects)
            .expect("the projects directory")
            .filter_map(|e| Some(e.ok()?.path().join("state.db")))
            .filter(|p| p.exists())
            .collect();
        stores.sort();
        stores
    }
}

/// Run `n` processes at once, the `i`th with `args(i)`.
fn concurrently(env: &Env, n: usize, args: impl Fn(usize) -> Vec<String> + Sync) -> Vec<Ran> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..n)
            .map(|i| {
                let args = &args;
                scope.spawn(move || {
                    let args = args(i);
                    let args: Vec<&str> = args.iter().map(String::as_str).collect();
                    env.run(&args)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

fn texts(tasks: &[Value], field: &str) -> Vec<String> {
    tasks
        .iter()
        .map(|t| t[field].as_str().expect(field).to_string())
        .collect()
}

/// A first-sight race: the project is unseeded, so the 40 adds race to
/// mint and bind its id (under the registry's lock) and to create and
/// migrate its database (under SQLite's write lock), as well as to add.
#[test]
fn test_concurrent_adds_at_first_sight_bind_one_project_and_lose_no_task() {
    let env = Env::new("cross-proc-harp");

    let n = 40;
    let ran = concurrently(&env, n, |i| {
        vec!["add".to_string(), format!("concurrent-task-{i:03}")]
    });
    for (i, r) in ran.iter().enumerate() {
        assert!(r.ok, "process {i} failed: {}", r.stderr);
    }

    let tasks = env.list();
    assert_eq!(tasks.len(), n, "every add landed");
    let mut harps = texts(&tasks, "harp_id");
    harps.sort();
    harps.dedup();
    assert_eq!(harps.len(), n, "no harp id twice");
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for text in texts(&tasks, "text") {
        *seen.entry(text).or_default() += 1;
    }
    for i in 0..n {
        let text = format!("concurrent-task-{i:03}");
        assert_eq!(seen.get(&text), Some(&1), "{text} exactly once");
    }
    assert!(
        tasks
            .iter()
            .all(|t| t["origin_session"] == "cross-proc-harp"),
        "each stamped with its session"
    );
    assert_eq!(env.stores().len(), 1, "one project, one store");
    assert_eq!(
        env.registered().len(),
        1,
        "one id bound: {:?}",
        env.registered()
    );
}

#[test]
fn test_readers_during_writes_see_whole_listings() {
    let env = Env::new("reader-writer-harp");
    env.seed("rw-seed");

    let (writers, readers) = (20, 10);
    let ran = concurrently(&env, writers + readers, |i| {
        if i < writers {
            vec!["add".to_string(), format!("rw-task-{i:03}")]
        } else {
            vec!["list".to_string(), "--json".to_string()]
        }
    });
    let last: BTreeMap<String, Value> = env
        .list()
        .into_iter()
        .map(|t| (t["harp_id"].as_str().expect("harp_id").to_string(), t))
        .collect();
    assert_eq!(last.len(), writers + 1, "every add, and the seed");
    for (i, r) in ran.iter().enumerate() {
        assert!(r.ok, "process {i} failed: {}", r.stderr);
        if i < writers {
            continue;
        }
        assert!(
            !r.stderr.contains("no project here"),
            "reader {i} found the seeded project: {}",
            r.stderr
        );
        let rows = r.json();
        let rows = rows.as_array().expect("an array");
        assert!(!rows.is_empty(), "reader {i} saw at least the seed");
        for row in rows {
            let harp = row["harp_id"].as_str().expect("harp_id");
            assert_eq!(
                Some(row),
                last.get(harp),
                "reader {i}: a task of the final listing, every field whole"
            );
        }
    }
    assert_eq!(env.stores().len(), 1);
}

/// A first-sight race for readers: listings run while the first adds
/// mint the project and create its database. A reader may find no project
/// yet, or a database not yet migrated or in WAL, and must still answer.
#[test]
fn test_readers_at_first_sight_answer_while_the_store_is_made() {
    let env = Env::new("first-sight-reader-harp");
    let before = env.run(&["list"]);
    assert!(
        before.ok && before.stderr.contains("no project here"),
        "{}",
        before.stderr
    );
    let (writers, readers) = (20, 10);
    let ran = concurrently(&env, writers + readers, |i| {
        if i < writers {
            vec!["add".to_string(), format!("fs-task-{i:03}")]
        } else {
            vec!["list".to_string(), "--json".to_string()]
        }
    });
    for (i, r) in ran.iter().enumerate() {
        assert!(r.ok, "process {i} failed: {}", r.stderr);
        if i >= writers {
            assert!(r.json().is_array(), "reader {i}: {}", r.stdout);
        }
    }
    assert_eq!(env.list().len(), writers);
    assert_eq!(env.registered().len(), 1);
}

#[test]
fn test_concurrent_tags_of_one_task_lose_no_tag() {
    let env = Env::new("cross-proc-tag-harp");
    let harp = env.seed("tag-me")["harp_id"]
        .as_str()
        .expect("a harp id")
        .to_string();

    let n = 40;
    let ran = concurrently(&env, n, |i| {
        vec![
            "tag".to_string(),
            harp.clone(),
            "--add".to_string(),
            format!("tag-{i:03}"),
        ]
    });
    for (i, r) in ran.iter().enumerate() {
        assert!(r.ok, "tag process {i} failed: {}", r.stderr);
    }

    let tasks = env.list();
    assert_eq!(tasks.len(), 1, "the one seeded task");
    let tags: Vec<String> = tasks[0]["tags"]
        .as_array()
        .expect("tags")
        .iter()
        .map(|t| t.as_str().unwrap().to_string())
        .collect();
    let expected: Vec<String> = (0..n).map(|i| format!("tag-{i:03}")).collect();
    assert_eq!(tags, expected, "every tag, sorted");
}

#[test]
fn test_a_task_added_is_listed_in_text_with_its_project() {
    let env = Env::new("smoke-harp");
    let added = env.run(&["add", "Ship", "it", "--tag", "release", "--format", "text"]);
    assert!(added.ok, "{}", added.stderr);
    let fields: Vec<&str> = added.stdout.trim_end().split('\t').collect();
    assert_eq!(fields[1..], ["To Do", "Ship it"]);
    assert!(
        added.stderr.starts_with("taskloom: project "),
        "{}",
        added.stderr
    );
    let listed = env.run(&["list", "--format", "text"]);
    assert!(listed.stdout.starts_with("Project: "), "{}", listed.stdout);
    assert!(
        listed.stdout.contains("Ship it  [release]"),
        "{}",
        listed.stdout
    );
    let json = env.list();
    let created = json[0]["created_at"].as_str().expect("created_at");
    assert!(
        created.starts_with("20"),
        "recorded at the clock's time: {created}"
    );
    let marker = std::fs::read_to_string(env.project.path().join(".project-id")).unwrap();
    assert_eq!(
        marker.trim().split('-').count(),
        3,
        "a three-word id: {marker}"
    );
}

#[test]
fn test_a_list_of_a_project_never_written_creates_nothing() {
    let env = Env::new("tpyo-harp");
    let listed = env.run(&["list", "--project", "tpyo", "--json"]);
    assert!(listed.ok, "{}", listed.stderr);
    assert_eq!(listed.json(), serde_json::json!([]));
    assert!(
        !env.home.path().join(".ctxloom").exists(),
        "no registry, no project directory, no database"
    );
    assert!(!env.project.path().join(".project-id").exists());
}

#[test]
fn test_a_tag_where_there_is_no_project_fails_and_binds_none() {
    let env = Env::new("no-project-harp");
    let tagged = env.run(&["tag", "swift-amber", "--add", "x"]);
    assert!(!tagged.ok);
    assert!(
        tagged.stderr.contains("no project here yet"),
        "{}",
        tagged.stderr
    );
    assert!(!env.home.path().join(".ctxloom").exists());
    assert!(!env.project.path().join(".project-id").exists());
}

#[test]
fn test_a_read_spans_every_project_when_asked_or_outside_any_work_root() {
    let env = Env::new("global-harp");
    env.seed("in the first");
    let (repo, stray, empty_repo) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    for dir in [&repo, &empty_repo] {
        std::fs::create_dir(dir.path().join(".git")).unwrap();
    }
    let added = env.run_in(repo.path(), &["add", "in the repo", "--json"], &[]);
    assert!(added.ok, "{}", added.stderr);

    let here = env.run(&["list", "--json"]);
    assert_eq!(
        texts(&here.json().as_array().unwrap().clone(), "text"),
        ["in the first"]
    );
    let every = env.run(&["list", "--global", "--json"]);
    assert!(every.ok, "{}", every.stderr);
    assert_eq!(every.stderr, "", "asked for: nothing to explain");
    let rows = every.json().as_array().unwrap().clone();
    let mut seen = texts(&rows, "text");
    seen.sort();
    assert_eq!(seen, ["in the first", "in the repo"]);
    assert!(rows.iter().all(|r| r["project_id"].is_string()), "{rows:?}");

    let fallback = env.run_in(stray.path(), &["list", "--json"], &[]);
    assert!(fallback.ok, "{}", fallback.stderr);
    assert_eq!(fallback.json().as_array().unwrap().len(), 2);
    assert!(
        fallback.stderr.contains("so this reads every project"),
        "{}",
        fallback.stderr
    );
    let text = env.run_in(stray.path(), &["list", "--format", "text"], &[]);
    assert!(
        text.stdout.starts_with("Projects: 2 (--global)\n"),
        "{}",
        text.stdout
    );

    let in_repo = env.run_in(empty_repo.path(), &["list", "--json"], &[]);
    assert_eq!(in_repo.json(), serde_json::json!([]));
    assert!(
        in_repo.stderr.contains("no project here yet"),
        "{}",
        in_repo.stderr
    );
    assert!(!stray.path().join(".project-id").exists());
    assert!(!empty_repo.path().join(".project-id").exists());
}

#[test]
fn test_every_diagnostic_of_every_crate_renders_from_the_binarys_catalog() {
    let catalog = super::catalog().expect("every catalog loads, no message twice");
    let samples: Vec<Box<dyn annals_diagnostic::Diagnostic>> = [
        annals::diagnostic_samples(),
        annals_store_sql::diagnostic_samples(),
        tasks::diagnostic_samples(),
        tasks_application::diagnostic_samples(),
        task_priority::diagnostic_samples(),
        project_registry::diagnostic_samples(),
        task_store_sqlite::diagnostic_samples(),
        tasks_cli::diagnostic_samples(),
        project_application::diagnostic_samples(),
        project_cli::diagnostic_samples(),
        go_task_log::diagnostic_samples(),
        tasks_config::diagnostic_samples(),
        presentation::diagnostic_samples(),
        tasks_mcp::diagnostic_samples(),
        tasks_run::diagnostic_samples(),
        registration_application::diagnostic_samples(),
        registration_cli::diagnostic_samples(),
        agent_configs::diagnostic_samples(),
        file_lock::diagnostic_samples(),
        tasks_watch::diagnostic_samples(),
    ]
    .into_iter()
    .flatten()
    .collect();
    for sample in &samples {
        assert!(
            catalog.render(&**sample).is_ok(),
            "{}: {:?}",
            sample.code(),
            catalog.render(&**sample)
        );
    }
}

/// stdout as exactly one JSON document, as jq reads it.
fn one_document(text: &str) -> Value {
    let mut documents = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let first = documents
        .next()
        .unwrap_or_else(|| panic!("no JSON: {text:?}"))
        .unwrap_or_else(|e| panic!("not JSON ({e}): {text:?}"));
    assert!(
        documents.next().is_none(),
        "more than one document: {text:?}"
    );
    first
}

#[test]
fn test_in_json_each_command_writes_one_document_and_errors_an_envelope() {
    let env = Env::new("jq-harp");
    let added = env.run(&["add", "Ship", "it", "--json"]);
    assert!(added.ok, "{}", added.stderr);
    let task = one_document(&added.stdout);
    let harp = task["harp_id"].as_str().expect("harp_id").to_string();
    assert!(
        added.stderr.starts_with("taskloom: project "),
        "notices on stderr: {}",
        added.stderr
    );
    let tagged = env.run(&["tag", &harp, "--add", "x", "--json"]);
    assert!(tagged.ok, "{}", tagged.stderr);
    assert_eq!(
        one_document(&tagged.stdout)["tags"],
        serde_json::json!(["x"])
    );
    let listed = env.run(&["list", "--json"]);
    assert!(listed.ok, "{}", listed.stderr);
    assert_eq!(one_document(&listed.stdout)[0]["harp_id"], harp.as_str());
    let queried = env.run(&["list", "--tag-query", "x", "--json"]);
    assert!(queried.ok, "{}", queried.stderr);
    assert_eq!(one_document(&queried.stdout)[0]["harp_id"], harp.as_str());
    let unread = env.run(&["list", "--tag-query", "x/and", "--json"]);
    assert!(!unread.ok);
    assert_eq!(unread.stdout, "", "an error leaves stdout empty");
    assert_eq!(
        one_document(&unread.stderr)["code"],
        "tasks-application-tag-query"
    );

    let failed = env.run(&["tag", "no-such-task", "--add", "x", "--json"]);
    assert!(!failed.ok);
    assert_eq!(failed.stdout, "", "an error leaves stdout empty");
    let envelope = one_document(&failed.stderr);
    assert!(envelope["code"].is_string(), "{envelope}");
    assert!(envelope["args"].is_object(), "{envelope}");
    assert!(envelope["message"].is_string(), "{envelope}");

    let usage = env.run(&["frobnicate", "--json"]);
    assert!(!usage.ok);
    assert_eq!(one_document(&usage.stderr)["code"], "tasks-cli-usage");

    let logs = env.home.path().join(".ctxloom/tasks");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::write(
        logs.join("jolly-mute-magma.jsonl"),
        "{\"op\":\"add\",\"task\":\"brisk-heron\",\"text\":\"Go's\",\"ts\":\"2026-03-01T10:01:00Z\"}\n",
    )
    .unwrap();
    let imported = env.run(&["import", "--json"]);
    assert!(imported.ok, "{}", imported.stderr);
    assert_eq!(one_document(&imported.stdout)["complete"], true);

    let incomplete = env.run(&["import", "--project", "no-such-project", "--json"]);
    assert!(!incomplete.ok);
    let report = one_document(&incomplete.stdout);
    assert_eq!(
        report["projects"][0]["failed"]["code"],
        "tasks-application-no-go-log"
    );
    assert_eq!(
        one_document(&incomplete.stderr)["code"],
        "tasks-cli-import-incomplete"
    );
}

/// A project with one task added, for a command's JSON to be checked.
struct Jq {
    env: Env,
    harp: String,
}

impl Jq {
    fn new(session: &'static str) -> Jq {
        let env = Env::new(session);
        let harp = env.seed("Ship it")["harp_id"]
            .as_str()
            .expect("harp_id")
            .to_string();
        Jq { env, harp }
    }

    /// `args`' answer: it succeeded, and wrote one JSON document.
    fn ok(&self, args: &[&str]) -> Value {
        let ran = self.env.run(args);
        assert!(ran.ok, "{args:?}: {}", ran.stderr);
        one_document(&ran.stdout)
    }

    /// `args` failed with `code`: an envelope on stderr, stdout empty.
    fn refused(&self, args: &[&str], code: &str) {
        let ran = self.env.run(args);
        assert!(!ran.ok, "{args:?}");
        assert_eq!(ran.stdout, "", "{args:?}: an error leaves stdout empty");
        assert_eq!(one_document(&ran.stderr)["code"], code, "{args:?}");
    }
}

#[test]
fn test_in_json_each_change_writes_one_document_and_errors_an_envelope() {
    let jq = Jq::new("jq-change-harp");
    let harp = jq.harp.as_str();
    let task = jq.ok(&["status", harp, "Deferred", "--trigger", "later", "--json"]);
    assert_eq!(
        (&task["status"], &task["trigger"]),
        (&"Deferred".into(), &"later".into())
    );
    let back = jq.ok(&["status", harp, "To Do", "--json"]);
    assert_eq!(back["trigger"], "later", "kept");
    jq.refused(
        &["status", harp, "@open", "--json"],
        "tasks-status-reserved",
    );
    jq.refused(
        &["status", "no-such-task", "Done", "--json"],
        "tasks-no-task",
    );
    let task = jq.ok(&["edit", harp, "Ship", "it", "today", "--json"]);
    assert_eq!(
        (&task["text"], &task["status"]),
        (&"Ship it today".into(), &"To Do".into())
    );
    jq.refused(&["edit", harp, " ", "--json"], "tasks-text-required");
    jq.refused(&["tag", harp, "--json"], "tasks-tag-required");
    let tagged = jq.ok(&[
        "tag",
        harp,
        "--add",
        "triage:level=3",
        "--add",
        "x",
        "--json",
    ]);
    assert_eq!(tagged["tags"], serde_json::json!(["triage:level=3", "x"]));
    let collapsed = jq.ok(&[
        "tag",
        harp,
        "--add",
        "triage:level=1",
        "--remove",
        "x",
        "--add",
        "x",
        "--json",
    ]);
    assert_eq!(
        collapsed["tags"],
        serde_json::json!(["triage:level=1"]),
        "the scalar collapsed, the add then the remove"
    );
}

#[test]
fn test_in_json_each_read_writes_one_document_and_errors_an_envelope() {
    let jq = Jq::new("jq-read-harp");
    let harp = jq.harp.as_str();
    jq.ok(&["tag", harp, "--add", "release", "--json"]);
    let narrowed = jq.ok(&[
        "list",
        "--status",
        "@open",
        "--term",
        "SHIP",
        "--compact",
        "--limit",
        "1",
        "--json",
    ]);
    assert_eq!(narrowed[0]["headline"], "Ship it");
    assert_eq!(jq.ok(&["list", "--global", "--json"])[0]["harp_id"], harp);
    jq.refused(
        &["list", "--status", "@done", "--json"],
        "tasks-application-unknown-status-class",
    );

    assert_eq!(jq.ok(&["show", harp, "--json"])["text"], "Ship it");
    let twice = jq.ok(&["show", harp, harp, "--global", "--json"]);
    assert_eq!(twice.as_array().map(Vec::len), Some(2));
    jq.refused(
        &["show", harp, "no-such-task", "--json"],
        "tasks-application-not-found",
    );

    for args in [
        &["summary", "--json"][..],
        &["summary", "--global", "--json"],
    ] {
        let summary = jq.ok(args);
        assert_eq!(summary["counts"]["To Do"], 1, "{args:?}");
        assert_eq!(summary["in_progress"], serde_json::json!([]), "{args:?}");
    }

    let statuses = jq.ok(&["statuses", "--json"]);
    let names: Vec<&Value> = statuses
        .as_array()
        .expect("an array")
        .iter()
        .map(|s| &s["name"])
        .collect();
    assert_eq!(
        names,
        ["In Progress", "To Do", "Deferred", "Done", "Archived"]
    );

    for args in [
        &["tags", "--json"][..],
        &["tags", "--global", "--status", "@open", "--json"],
    ] {
        assert_eq!(
            jq.ok(args),
            serde_json::json!([{"tag": "release", "active": 1, "total": 1}]),
            "{args:?}"
        );
    }
    jq.refused(
        &["tags", "--tag-query", "x/and", "--json"],
        "tasks-application-tag-query",
    );

    let linted = jq.ok(&["lint", "--json"]);
    assert_eq!(linted["violations"], serde_json::json!([]));
    assert!(linted["checked_targets"].as_u64() > Some(0), "{linted}");
    let declared = r#"tag_schema=tagma.enum:"release"="beta""#;
    let report = jq.ok(&["lint", "--config-set", declared, "--json"]);
    assert_eq!(
        report["violations"],
        serde_json::json!([]),
        "release has no value"
    );
    let tagged = jq.ok(&["tag", harp, "--add", "area:x=c", "--json"]);
    assert_eq!(tagged["tags"].as_array().map(Vec::len), Some(2));
    let declared = r#"tag_schema=tagma.enum:"area:x"="a,b""#;
    let ran = jq.env.run(&["lint", "--config-set", declared, "--json"]);
    assert!(!ran.ok, "a violation exits 1");
    let report = one_document(&ran.stdout);
    assert_eq!(report["violations"][0]["harp_id"], harp);
    assert_eq!(
        one_document(&ran.stderr)["code"],
        "tasks-cli-lint-violations"
    );
}

#[test]
fn test_a_list_sorted_by_priority_ranks_the_projects_tasks_through_its_store() {
    let jq = Jq::new("jq-priority-harp");
    let critical = jq.ok(&["add", "Fix it", "--tag", "triage:level=1", "--json"]);
    let ranked = jq.ok(&["list", "--sort", "priority", "--json"]);
    let order: Vec<(&Value, &Value)> = ranked
        .as_array()
        .expect("an array")
        .iter()
        .map(|t| (&t["harp_id"], &t["derived_priority"]))
        .collect();
    assert_eq!(
        order,
        [
            (&critical["harp_id"], &serde_json::json!(5)),
            (&serde_json::json!(jq.harp), &serde_json::json!(2.5)),
        ],
        "level 1 first; the untriaged task floors below it"
    );
    assert_eq!(
        jq.ok(&["list", "--json"])[0]["derived_priority"],
        Value::Null
    );
}

#[test]
fn test_in_json_a_stream_writes_one_document_a_line() {
    let env = Env::new("jq-watch-harp");
    let added = env.run(&["add", "Ship", "it", "--json"]);
    assert!(added.ok, "{}", added.stderr);
    let watching = watch::Watching::start(&env, &["--json"]);
    assert_eq!(one_document(&watching.line())["event"], "changed");
    let harp = one_document(&added.stdout)["harp_id"].to_string();
    let tagged = env.run(&["tag", harp.trim_matches('"'), "--add", "x"]);
    assert!(tagged.ok, "{}", tagged.stderr);
    assert_eq!(one_document(&watching.line())["kind"], "tasks");
    assert!(watching.stop("INT").success());
}

#[test]
fn test_in_json_each_project_command_writes_one_document_and_errors_an_envelope() {
    let env = Env::new("jq-project-harp");
    let added = env.run(&["add", "Ship", "it", "--json"]);
    assert!(added.ok, "{}", added.stderr);
    let shown = env.run(&["project", "show", "--json"]);
    assert!(shown.ok, "{}", shown.stderr);
    let project = one_document(&shown.stdout)["project_id"]
        .as_str()
        .expect("the project added to")
        .to_string();
    for args in [
        &["project", "heal", "--json"][..],
        &["project", "orphans", "--json"],
        &["project", "repoint", &project, "--json"],
    ] {
        let ran = env.run(args);
        assert!(ran.ok, "{args:?}: {}", ran.stderr);
        one_document(&ran.stdout);
    }
    let refused = env.run(&["project", "repoint", "no-such-project", "--json"]);
    assert!(!refused.ok);
    assert_eq!(refused.stdout, "", "an error leaves stdout empty");
    assert_eq!(
        one_document(&refused.stderr)["code"],
        "project-application-unknown-project"
    );
}

#[path = "companion.test.rs"]
mod companion;

#[path = "import.test.rs"]
mod import;

#[path = "manage.test.rs"]
mod manage;

#[path = "mcp.test.rs"]
mod mcp;

#[path = "project.test.rs"]
mod project;

#[path = "run.test.rs"]
mod run;

#[path = "tags.test.rs"]
mod tags;

#[path = "prune.test.rs"]
mod prune;

#[path = "snapshot.test.rs"]
mod snapshot;

#[path = "watch.test.rs"]
mod watch;
