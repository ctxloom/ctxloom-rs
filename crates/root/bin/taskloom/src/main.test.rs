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
    /// A fresh env, its project identity seeded by one `seed` command run
    /// before any concurrency, as Go's tests do: first sight of a project
    /// is not what these tests exercise.
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
        let output = Command::new(binary())
            .args(args)
            .current_dir(self.project.path())
            .env_clear()
            .env("HOME", self.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("CTXLOOM_SESSION_HARP", self.session)
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

#[test]
fn test_concurrent_adds_lose_no_task_and_duplicate_no_harp() {
    let env = Env::new("cross-proc-harp");
    let seed = env.run(&["list"]);
    assert!(seed.ok, "seed: {}", seed.stderr);

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
}

#[test]
fn test_readers_during_writes_see_whole_listings() {
    let env = Env::new("reader-writer-harp");
    let seed = env.run(&["list"]);
    assert!(seed.ok, "seed: {}", seed.stderr);

    let (writers, readers) = (20, 10);
    let ran = concurrently(&env, writers + readers, |i| {
        if i < writers {
            vec!["add".to_string(), format!("rw-task-{i:03}")]
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
    assert_eq!(env.stores().len(), 1);
}

#[test]
fn test_concurrent_tags_of_one_task_lose_no_tag() {
    let env = Env::new("cross-proc-tag-harp");
    let added = env.run(&["add", "tag-me", "--format", "json"]);
    assert!(added.ok, "seed: {}", added.stderr);
    let harp = added.json()["harp_id"]
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
