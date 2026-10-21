//! Acceptance: `taskloom project`, the real binary in a temporary home,
//! finding and correcting a directory bound to the wrong project, so its
//! tasks no longer seem lost.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ports::TaskStores;
use serde_json::Value;
use task_store_sqlite::SqliteStores;
use tempfile::TempDir;

use super::{binary, Ran};

#[path = "../../../../adapters/driven/task-store-sqlite/src/damage.test.rs"]
mod damage;

/// A temporary home, and trees made in a temporary directory beside it.
struct World {
    home: TempDir,
    trees: TempDir,
}

impl World {
    fn new() -> World {
        World {
            home: tempfile::tempdir().unwrap(),
            trees: tempfile::tempdir().unwrap(),
        }
    }

    /// A new directory named `name`, canonical.
    fn tree(&self, name: &str) -> PathBuf {
        let dir = fs::canonicalize(self.trees.path()).unwrap().join(name);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// `taskloom args` in `cwd`, with only HOME and PATH set.
    fn run(&self, cwd: &Path, args: &[&str]) -> Ran {
        let output = Command::new(binary())
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("HOME", self.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .output()
            .expect("taskloom runs");
        Ran::of(output)
    }

    /// `taskloom args --json` in `cwd`, which must succeed: its document.
    fn json(&self, cwd: &Path, args: &[&str]) -> Value {
        let mut args = args.to_vec();
        args.push("--json");
        let ran = self.run(cwd, &args);
        assert!(ran.ok, "{args:?}: {}", ran.stderr);
        ran.json()
    }

    /// Add a task in `cwd`: the project it went to.
    fn add(&self, cwd: &Path, text: &str) -> String {
        let ran = self.run(cwd, &["add", text, "--json"]);
        assert!(ran.ok, "add: {}", ran.stderr);
        self.json(cwd, &["project", "show"])["project_id"]
            .as_str()
            .expect("a project")
            .to_string()
    }

    fn texts(&self, cwd: &Path) -> Vec<String> {
        self.json(cwd, &["list"])
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["text"].as_str().unwrap().to_string())
            .collect()
    }

    fn database(&self, id: &str) -> PathBuf {
        self.home.path().join(".ctxloom/projects").join(id)
    }
}

fn kinds(problems: &Value) -> Vec<&str> {
    problems
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap())
        .collect()
}

#[test]
fn test_a_second_clone_with_the_committed_marker_shares_the_project_by_design() {
    // Clones share by default (decided 2026-10-21): the committed
    // .project-id is the project, wherever it is checked out.
    let world = World::new();
    let first = world.tree("first");
    let second = world.tree("second");
    let id = world.add(&first, "the first clone's task");
    fs::copy(first.join(".project-id"), second.join(".project-id")).unwrap();

    // Before any heal, the second clone already reads and writes the store.
    assert_eq!(world.texts(&second), ["the first clone's task"]);
    let shown = world.json(&second, &["project", "show"]);
    assert_eq!(shown["project_id"], id.as_str());
    assert_eq!(
        shown["shared_with"],
        serde_json::json!([format!("{}", first.display())])
    );
    assert_eq!(
        kinds(&shown["problems"]),
        ["unregistered"],
        "only unregistered"
    );
    let text = world.run(&second, &["project", "show", "--format", "text"]);
    assert!(
        text.stdout.contains(&format!(
            "Shared:   {} (another tree holding the same project id: one project, one database)",
            first.display()
        )),
        "{}",
        text.stdout
    );

    // heal registers the second tree beside the first, and that is all.
    let healed = world.json(&second, &["project", "heal", "--yes"]);
    assert_eq!(healed["applied"], true);
    assert_eq!(
        healed["changes"],
        serde_json::json!([{"kind": "register", "project_id": id, "path": second.display().to_string()}])
    );
    assert_eq!(healed["unresolved"], serde_json::json!([]));
    let shown = world.json(&second, &["project", "show"]);
    assert_eq!(kinds(&shown["problems"]), [""; 0], "sharing is no problem");
    let from_first = world.json(&first, &["project", "show"]);
    assert_eq!(
        from_first["shared_with"],
        serde_json::json!([second.display().to_string()]),
        "and the first sees the second"
    );

    // An agent in either writes into the one list.
    world.add(&second, "the second clone's task");
    assert_eq!(
        world.texts(&first),
        ["the first clone's task", "the second clone's task"]
    );
}

#[test]
fn test_a_tree_that_forked_is_repointed_and_its_own_database_orphaned_then_removed() {
    let world = World::new();
    let original = world.tree("original");
    let copy = world.tree("copy");
    let kept = world.add(&original, "the original's task");
    let forked = world.add(&copy, "the copy's task");
    assert_ne!(kept, forked);
    assert_eq!(world.texts(&copy), ["the copy's task"], "it seems lost");

    let refused = world.run(&copy, &["project", "repoint", &kept, "--yes"]);
    assert!(!refused.ok);
    assert!(
        refused.stderr.contains("pass --share"),
        "{}",
        refused.stderr
    );
    let would = world.json(&copy, &["project", "repoint", &kept, "--share"]);
    assert_eq!(would["applied"], false);
    assert_eq!(world.texts(&copy), ["the copy's task"], "only reported");

    let made = world.json(&copy, &["project", "repoint", &kept, "--share", "--yes"]);
    assert_eq!(made["applied"], true);
    assert_eq!(made["previous"], forked.as_str());
    assert_eq!(world.texts(&copy), ["the original's task"], "found again");
    assert_eq!(
        fs::read_to_string(copy.join(".project-id")).unwrap(),
        format!("{kept}\n")
    );

    let shown = world.json(&copy, &["project", "show"]);
    assert_eq!(shown["project_id"], kept.as_str());
    assert_eq!(kinds(&shown["problems"]), ["orphan"]);
    assert_eq!(shown["problems"][0]["project_id"], forked.as_str());
    let orphans = world.json(&copy, &["project", "orphans"]);
    assert_eq!(orphans[0]["project_id"], forked.as_str());

    let not_orphaned = world.run(&copy, &["project", "orphans", "remove", &kept, "--yes"]);
    assert!(!not_orphaned.ok, "a live database is refused");
    let reported = world.json(&copy, &["project", "orphans", "remove", &forked]);
    assert_eq!(reported["applied"], false);
    assert!(world.database(&forked).exists(), "only reported");
    let removed = world.json(&copy, &["project", "orphans", "remove", &forked, "--yes"]);
    assert_eq!(removed["applied"], true);
    assert!(!world.database(&forked).exists());
    assert!(world.database(&kept).join("state.db").exists());
    assert_eq!(world.texts(&original), ["the original's task"]);
    assert_eq!(
        world.json(&copy, &["project", "orphans"]),
        Value::Array(vec![])
    );
}

#[test]
fn test_a_lost_marker_is_shown_and_healed_from_the_registry() {
    let world = World::new();
    let tree = world.tree("tree");
    let id = world.add(&tree, "a task");
    fs::remove_file(tree.join(".project-id")).unwrap();

    let shown = world.json(&tree, &["project", "show"]);
    assert_eq!(shown["source"], "registry");
    assert_eq!(kinds(&shown["problems"]), ["missing-marker"]);
    assert_eq!(shown["entries"][0]["project_id"], id.as_str());
    assert_eq!(
        shown["database"],
        world.database(&id).join("state.db").display().to_string()
    );

    let would = world.json(&tree, &["project", "heal"]);
    assert_eq!(would["changes"][0]["kind"], "write-marker");
    assert!(!tree.join(".project-id").exists(), "only reported");
    let text = world.run(&tree, &["project", "heal", "--yes", "--format", "text"]);
    assert!(text.ok, "{}", text.stderr);
    assert!(text.stdout.starts_with("Changed:\n"), "{}", text.stdout);
    assert_eq!(
        fs::read_to_string(tree.join(".project-id")).unwrap(),
        format!("{id}\n")
    );
    let healed = world.json(&tree, &["project", "show"]);
    assert_eq!(kinds(&healed["problems"]), Vec::<&str>::new());
    assert_eq!(
        world.json(&tree, &["project", "heal"])["changes"],
        Value::Array(vec![])
    );
}

#[test]
fn test_a_linked_worktree_shares_its_primary_checkouts_tasks() {
    let world = World::new();
    let primary = world.tree("primary");
    let git = |cwd: &Path, args: &[&str]| {
        let out = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(args)
            .current_dir(cwd)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&primary, &["init", "-q"]);
    git(&primary, &["commit", "-q", "--allow-empty", "-m", "first"]);
    let linked = fs::canonicalize(world.trees.path()).unwrap().join("linked");
    git(
        &primary,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            &linked.display().to_string(),
        ],
    );

    let id = world.add(&linked.join("."), "added in the worktree");
    assert!(
        !linked.join(".project-id").exists(),
        "nothing to merge back"
    );
    assert_eq!(world.texts(&primary), ["added in the worktree"]);
    let shown = world.json(&linked, &["project", "show"]);
    assert_eq!(shown["project_id"], id.as_str());
    assert_eq!(shown["root"], primary.display().to_string());
    assert_eq!(shown["work_root"], linked.display().to_string());
}

#[test]
fn test_every_project_command_answers_as_text_too() {
    let world = World::new();
    let tree = world.tree("tree");
    for args in [
        &["project", "show"][..],
        &["project", "heal"],
        &["project", "orphans"],
    ] {
        let ran = world.run(&tree, &[args, &["--format", "text"]].concat());
        assert!(ran.ok, "{args:?}: {}", ran.stderr);
        assert!(
            serde_json::from_str::<Value>(&ran.stdout).is_err(),
            "{}",
            ran.stdout
        );
    }
    assert!(
        !world
            .home
            .path()
            .join(".ctxloom/projects/index.yaml")
            .exists(),
        "a read binds nothing"
    );
}

/// Open the database at `path` in this process and close it, waiting for
/// the close: the last connection's close checkpoints the write-ahead log
/// a process left behind into the file and removes it, so the file is
/// whole. Its removal is the sign, waited for up to a deadline.
fn settled(path: &Path) {
    let project = ports::Project {
        id: "settling".to_string(),
        root: "/".to_string(),
        store: path.to_path_buf(),
    };
    let mut stores = SqliteStores::default();
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(async { stores.open(&project).await.map(drop) })
        .expect("the database opens to write");
    drop(stores);
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while Path::new(&wal).exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the database's log is still beside it"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn test_heal_reports_a_damaged_database_from_the_catalog_and_leaves_it_damaged() {
    let world = World::new();
    let tree = world.tree("tree");
    let id = world.add(&tree, "a task");
    let database = world.database(&id).join("state.db");
    settled(&database);
    // Damaged in a copy, then put in the database's place.
    let copy = world.trees.path().join("copy.db");
    fs::write(
        &copy,
        damage::damaged(&fs::read(&database).unwrap(), "events_by_command", |page| {
            *page.last_mut().unwrap() ^= 1;
        }),
    )
    .unwrap();
    fs::rename(&copy, &database).unwrap();
    let before = fs::read(&database).unwrap();

    let found = world.json(&tree, &["project", "heal", "--yes"]);
    assert_eq!(found["changes"], Value::Array(vec![]), "nothing to repair");
    assert_eq!(kinds(&found["unresolved"]), ["damaged"]);
    let damaged = &found["unresolved"][0];
    assert_eq!(damaged["project_id"], id.as_str());
    assert_eq!(damaged["database"], database.display().to_string());
    let damage = &damaged["damage"];
    assert_eq!(damage["code"], "annals-store-sql-damaged");
    let first = damage["args"]["first"].as_str().expect("the first problem");
    assert!(first.contains("events_by_command"), "{damage}");
    assert!(damage["args"]["more"].is_u64(), "{damage}");
    assert!(
        damage["message"]
            .as_str()
            .unwrap()
            .starts_with(&format!("the database file is damaged: {first}")),
        "{damage}"
    );

    let text = world.run(&tree, &["project", "heal", "--format", "text"]);
    assert!(text.ok, "{}", text.stderr);
    assert!(
        text.stdout.contains(&format!(
            "Not resolved:\n  - {}, the database of {id}: the database file is damaged: {first}",
            database.display()
        )),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout
            .contains("heal reports damage and does not repair it"),
        "{}",
        text.stdout
    );
    assert_eq!(fs::read(&database).unwrap(), before, "never repaired");
}

#[test]
fn test_heal_finds_no_damage_in_a_sound_database() {
    let world = World::new();
    let tree = world.tree("tree");
    world.add(&tree, "a task");
    let healed = world.json(&tree, &["project", "heal"]);
    assert_eq!(healed["unresolved"], Value::Array(vec![]));
    let text = world.run(&tree, &["project", "heal", "--format", "text"]);
    assert_eq!(text.stdout, "Nothing to change.\n");
}

#[test]
fn test_heal_of_a_project_with_no_database_is_no_damage_and_creates_none() {
    let world = World::new();
    let tree = world.tree("tree");
    let id = world.add(&tree, "a task");
    fs::remove_dir_all(world.database(&id)).unwrap();
    let healed = world.json(&tree, &["project", "heal", "--yes"]);
    assert_eq!(healed["unresolved"], Value::Array(vec![]));
    assert!(!world.database(&id).exists(), "the check created nothing");
}
