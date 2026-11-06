//! Gherkin features (S12), run with cucumber-rs over the real binary: Go
//! ctxloom's own `.feature` text (revision f36e8dfb21, release/0.7; see each
//! file's header for what was left out) with new Rust step definitions.
//!
//! The steps build a project and a home the way Go's steps do (see
//! `library.test.rs`), seed sessions through `ctxloom test seed-session`
//! (session.test.rs), run the binary, and read what it printed.

use std::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use cucumber::gherkin::Step;
use cucumber::{given, then};
use serde_json::Value;

use super::gherkin_common::{common_steps, doc, Common, CommonWorld};
use super::library::{at, Project};
use crate::tests::files;

const DAY: u64 = 86_400;

/// The uncommitted work a reaper must never destroy (Go: `j001300WIPMarker`).
const WIP_MARKER: &str = "J001300-WIP-MARKER-5b8e21";

/// How a session is to be seeded, decided when the first command runs (a
/// step such as "every session has been idle for 120 days" comes after the
/// sessions it dates).
struct Planted {
    harp: String,
    /// `finished`, `running`, `unproven` or `oneshot`.
    kind: &'static str,
    /// Seconds since it ended, when the step says; else the kind's default.
    ended_ago: Option<u64>,
    keep: bool,
    summary: bool,
    no_lock: bool,
}

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct World {
    common: Common,
    planted: Vec<Planted>,
    seeded: bool,
    idle_days: Option<u64>,
    /// The locks of the sessions that are running, held until the scenario
    /// ends.
    held: Vec<File>,
    /// Scratch worktrees the steps left in sessions: the session, the
    /// checkout's suffix and whether it holds uncommitted work.
    scratch: Vec<(String, String, bool)>,
    /// A worktree of Alice's own, outside the sessions root: its branch and
    /// directory.
    foreign: Vec<(String, PathBuf)>,
    /// What `seed_scratch` planted, for the assertions.
    planted_scratch: Vec<(String, String, bool)>,
    planted_foreign: Vec<PathBuf>,
    /// The sessions planted with a free lock that is not held.
    lock_free_dead: Vec<String>,
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "World")
    }
}

impl CommonWorld for World {
    fn common(&self) -> &Common {
        &self.common
    }

    fn common_mut(&mut self) -> &mut Common {
        &mut self.common
    }

    fn before_run(&mut self) {
        self.seed();
        self.seed_scratch();
    }
}

impl World {
    fn new() -> World {
        World {
            common: Common::new(),
            planted: Vec::new(),
            seeded: false,
            idle_days: None,
            held: Vec::new(),
            scratch: Vec::new(),
            foreign: Vec::new(),
            planted_scratch: Vec::new(),
            planted_foreign: Vec::new(),
            lock_free_dead: Vec::new(),
        }
    }

    fn session_dir(&self, harp: &str) -> PathBuf {
        self.common
            .project
            .home()
            .join(".ctxloom/sessions")
            .join(harp)
    }

    fn output_dir(&self, harp: &str) -> PathBuf {
        let project = self.common.project.dir().file_name().expect("a name");
        self.common
            .project
            .home()
            .join("Documents/ctxloom")
            .join(project)
            .join(harp)
    }

    /// Plant the sessions the steps described, once.
    fn seed(&mut self) {
        if self.seeded {
            return;
        }
        self.seeded = true;
        let planted = std::mem::take(&mut self.planted);
        for session in &planted {
            let ended = session
                .ended_ago
                .or(self.idle_days.map(|d| d * DAY))
                .unwrap_or(match session.kind {
                    "oneshot" => 3_600,
                    _ => 60 * DAY,
                });
            let started = (ended + DAY).to_string();
            let ended = ended.to_string();
            let mut flags: Vec<&str> = vec!["test", "seed-session", &session.harp, "--transcript"];
            match session.kind {
                "running" => flags.extend(["--lock", "free", "--started-ago", "86400"]),
                _ => {
                    if !session.no_lock {
                        flags.extend(["--lock", "free"]);
                    }
                    flags.extend(["--started-ago", &started, "--ended-ago", &ended]);
                }
            }
            if session.kind == "oneshot" {
                flags.push("--oneshot");
            }
            if session.keep {
                flags.push("--keep");
            }
            let ran = self.common.project.run(&flags);
            assert_eq!(ran.code, 0, "seeding {}: {}", session.harp, ran.said());
            if session.kind != "running" && !session.no_lock {
                self.lock_free_dead.push(session.harp.clone());
            }
            if session.summary {
                let dir = self.output_dir(&session.harp);
                files::put(dir.join("essence.md"), "an older ctxloom's summary\n");
            }
            if session.kind == "running" {
                let lock = files::open_read(
                    self.common
                        .project
                        .home()
                        .join(".ctxloom/sessions")
                        .join(format!("{}.lock", session.harp)),
                )
                .expect("a lock file");
                lock.lock().expect("the lock is free to take");
                self.held.push(lock);
            }
        }
    }

    /// The project's git, with a clean identity and none of this process's
    /// environment.
    fn git(&self, dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Alice",
                "-c",
                "user.email=alice@example.com",
            ])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", crate::tests::host_var("PATH").unwrap_or_default())
            .env("HOME", self.common.project.home())
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Make the project a git repository, once, with one commit.
    fn git_backed(&self) {
        let project = self.common.project.dir();
        if files::exists(project.join(".git")) {
            return;
        }
        self.git(project, &["init", "-q"]);
        files::put(project.join("README.md"), "hello\n");
        self.git(project, &["add", "README.md"]);
        self.git(project, &["commit", "-q", "-m", "initial"]);
    }

    /// Whether `harp`'s owner is provably dead: it was planted with a free
    /// lock and is not running.
    fn reapable(&self, harp: &str) -> bool {
        self.lock_free_dead.iter().any(|h| h == harp)
    }

    fn scratch_dir(&self, harp: &str, name: &str) -> PathBuf {
        self.session_dir(harp)
            .join("work")
            .join(format!("ctxloom-wt-{name}"))
    }

    /// Plant the scratch worktrees the steps described: genuine `git
    /// worktree add` checkouts in the sessions' work directories, and the
    /// uncommitted work that no reaper may ever destroy.
    fn seed_scratch(&mut self) {
        if self.scratch.is_empty() && self.foreign.is_empty() {
            return;
        }
        self.git_backed();
        for (harp, name, dirty) in std::mem::take(&mut self.scratch) {
            let dir = self.scratch_dir(&harp, &name);
            let branch = format!("wt-{harp}-{name}");
            self.git(
                self.common.project.dir(),
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &branch,
                    &dir.display().to_string(),
                ],
            );
            if dirty {
                files::put(dir.join("in-flight.go"), format!("// {WIP_MARKER}\n"));
            }
            self.planted_scratch.push((harp, name, dirty));
        }
        for (branch, dir) in std::mem::take(&mut self.foreign) {
            files::mkdirs(dir.parent().expect("a parent"));
            self.git(
                self.common.project.dir(),
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &branch,
                    &dir.display().to_string(),
                ],
            );
            files::put(dir.join("feature.txt"), "unmerged work\n");
            self.git(&dir, &["add", "-A"]);
            self.git(&dir, &["commit", "-q", "-m", "unmerged feature work"]);
            files::put(dir.join("uncommitted.txt"), format!("{WIP_MARKER}\n"));
            self.planted_foreign.push(dir);
        }
    }

    fn plant(&mut self, harp: &str, kind: &'static str) -> &mut Planted {
        self.planted.push(Planted {
            harp: harp.to_string(),
            kind,
            ended_ago: None,
            keep: false,
            summary: false,
            no_lock: false,
        });
        self.planted.last_mut().expect("just pushed")
    }
}

// ------------------------------------------------------------------ given

#[given(expr = "the environment variable {string} is set to {string}")]
fn env_set(w: &mut World, name: String, value: String) {
    w.common.env.retain(|(n, _)| *n != name);
    w.common.env.push((name, value));
}

#[given(expr = "the home already has the file {string}:")]
fn home_file(w: &mut World, step: &Step, rel: String) {
    w.common
        .project
        .write_home(&rel, &format!("{}\n", doc(step)));
}

#[given("a malformed ctxloom config")]
fn malformed(w: &mut World) {
    w.common.project.write(
        ".ctxloom/config.yaml",
        "schema_version: [unclosed\n  - : :\n",
    );
}

#[given(expr = "a finished session {string} with a transcript")]
fn finished(w: &mut World, harp: String) {
    w.plant(&harp, "finished");
}

#[given(expr = "a finished session {string} with a transcript and a summary an older ctxloom left")]
fn finished_with_summary(w: &mut World, harp: String) {
    w.plant(&harp, "finished").summary = true;
}

#[given(expr = "a finished session {string} with a transcript but no session lock")]
fn finished_unlocked(w: &mut World, harp: String) {
    w.plant(&harp, "finished").no_lock = true;
}

#[given(expr = "a finished session {string} whose transcript retention has passed")]
fn finished_retention_passed(w: &mut World, harp: String) {
    w.plant(&harp, "finished");
}

#[given(expr = "a finished session {string} marked keep")]
fn finished_kept(w: &mut World, harp: String) {
    w.plant(&harp, "finished").keep = true;
}

#[given(expr = "a session {string} with a transcript that is running again")]
fn running_again(w: &mut World, harp: String) {
    // The index says ended; the lock says running.
    w.plant(&harp, "running");
}

#[given(expr = "a session {string} that is still running")]
fn still_running(w: &mut World, harp: String) {
    w.plant(&harp, "running");
}

#[given(expr = "a session {string} nothing can prove the liveness of")]
fn unproven(w: &mut World, harp: String) {
    w.plant(&harp, "unproven").no_lock = true;
}

#[given(expr = "a session {string} with a transcript that finished an hour ago")]
fn finished_an_hour_ago(w: &mut World, harp: String) {
    w.plant(&harp, "finished").ended_ago = Some(3_600);
}

#[given(expr = "an internal one-shot session {string} that finished an hour ago")]
fn oneshot(w: &mut World, harp: String) {
    w.plant(&harp, "oneshot");
}

#[given("the feature shipped on Friday and Alice is closing the workstream out")]
fn closing_out(w: &mut World) {
    w.common.project = Project::initialized();
}

#[given(expr = "session {string} left a clean scratch worktree")]
fn left_clean(w: &mut World, harp: String) {
    w.scratch.push((harp, "clean".to_string(), false));
}

#[given(expr = "session {string} left a scratch worktree holding uncommitted work")]
fn left_dirty(w: &mut World, harp: String) {
    w.scratch.push((harp, "wip".to_string(), true));
}

#[given(expr = "session {string} left a scratch worktree of its own")]
fn left_own(w: &mut World, harp: String) {
    w.scratch.push((harp, "own".to_string(), false));
}

#[given(
    expr = "a long-lived worktree {string} of her own, outside the sessions root, with unmerged work"
)]
fn long_lived(w: &mut World, branch: String) {
    let dir = w
        .common
        .project
        .home()
        .join("workspace/worktrees")
        .join(format!("proj--{branch}"));
    w.foreign.push((branch, dir));
}

#[given(expr = "every session has been idle for {int} days")]
fn idle(w: &mut World, days: u64) {
    w.idle_days = Some(days);
}

// ------------------------------------------------------------------- when

// ------------------------------------------------------------------- then

/// Go: "exactly N home files match" a glob below the home, one `*` per
/// path segment.
#[then(expr = "exactly {int} home files match {string}")]
fn home_files_match(w: &mut World, count: usize, pattern: String) {
    fn walk(dir: &Path, rel: &str, parts: &[&str], found: &mut usize) {
        let Some((first, rest)) = parts.split_first() else {
            *found += 1;
            return;
        };
        for path in files::children(dir) {
            let name = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            let ok = *first == "*" || *first == name;
            if ok {
                walk(&path, &format!("{rel}/{name}"), rest, found);
            }
        }
    }
    let parts: Vec<&str> = pattern.split('/').collect();
    let mut found = 0;
    walk(w.common.project.home(), "", &parts, &mut found);
    assert_eq!(found, count, "{pattern}");
}

#[then(expr = "the home file {string} exists")]
fn home_exists(w: &mut World, rel: String) {
    assert!(
        files::exists(w.common.project.home().join(&rel)),
        "{rel} should exist"
    );
}

#[then(expr = "the home file {string} does not exist")]
fn home_absent(w: &mut World, rel: String) {
    assert!(
        !files::exists(w.common.project.home().join(&rel)),
        "{rel} should not exist"
    );
}

#[then(expr = "the home file {string} does not contain {string}")]
fn home_lacks(w: &mut World, rel: String, text: String) {
    let held = w.common.project.read_home(&rel).expect("a home file");
    assert!(!held.contains(&text), "{rel}: {held}");
}

#[then(expr = "the output file {string} of session {string} exists")]
fn output_file_exists(w: &mut World, file: String, harp: String) {
    let path = w.output_dir(&harp).join(&file);
    assert!(files::exists(&path), "{} should exist", path.display());
}

fn array<'a>(doc: &'a Value, path: &str) -> &'a Vec<Value> {
    at(doc, path)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("no array at {path} in {doc}"))
}

#[then(expr = "the JSON output array {string} contains an object whose {string} is {string}")]
fn array_has(w: &mut World, path: String, key: String, value: String) {
    let doc = w.json();
    assert!(
        array(&doc, &path).iter().any(|o| is(o, &key, &value)),
        "no object with {key}={value}: {doc}"
    );
}

#[then(
    expr = "the JSON output array {string} contains an object whose {string} is {string} and whose {string} is {string}"
)]
fn array_has_two(
    w: &mut World,
    path: String,
    key: String,
    value: String,
    key2: String,
    value2: String,
) {
    let doc = w.json();
    assert!(
        array(&doc, &path)
            .iter()
            .any(|o| is(o, &key, &value) && is(o, &key2, &value2)),
        "no object with {key}={value} and {key2}={value2}: {doc}"
    );
}

/// Go's comparison: the value as the text of its JSON (a bool is `true`).
fn is(object: &Value, key: &str, want: &str) -> bool {
    match &object[key] {
        Value::String(s) => s == want,
        Value::Null => false,
        other => {
            let text = other.to_string();
            text == want
        }
    }
}

#[then(expr = "every object in the JSON output array {string} has a non-empty {string}")]
fn every_non_empty(w: &mut World, path: String, key: String) {
    let doc = w.json();
    for object in array(&doc, &path) {
        assert!(
            object[&key].as_str().is_some_and(|s| !s.is_empty()),
            "{object} lacks {key}"
        );
    }
}

fn hook_commands(w: &World, rel: &str, event: &str) -> Vec<String> {
    let text = w.common.project.read(rel).unwrap_or_default();
    let doc: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let mut commands = Vec::new();
    for group in doc["hooks"][event].as_array().into_iter().flatten() {
        for hook in group["hooks"].as_array().into_iter().flatten() {
            if let Some(command) = hook["command"].as_str() {
                commands.push(command.to_string());
            }
        }
    }
    commands
}

#[then(expr = "the file {string} registers no SessionStart hook whose command contains {string}")]
fn registers_no_hook(w: &mut World, rel: String, text: String) {
    let commands = hook_commands(w, &rel, "SessionStart");
    assert!(!commands.iter().any(|c| c.contains(&text)), "{commands:?}");
}

#[then(expr = "the file {string} registers a SessionStart hook whose command contains {string}")]
fn registers_hook(w: &mut World, rel: String, text: String) {
    let commands = hook_commands(w, &rel, "SessionStart");
    assert!(commands.iter().any(|c| c.contains(&text)), "{commands:?}");
}

// The sweep (j001300).

#[then(expr = "the sweep report names {string} and {string}")]
fn sweep_names(w: &mut World, a: String, b: String) {
    w.ran().succeeds().contains(&a).contains(&b);
}

#[then("every byte of every session is still on disk")]
fn bytes_on_disk(w: &mut World) {
    let sessions = w.common.project.home().join(".ctxloom/sessions");
    for path in files::children(&sessions) {
        if files::is_dir(&path) {
            assert!(
                files::exists(path.join("transcripts/transcript.jsonl")),
                "{} lost its transcript",
                path.display()
            );
        }
    }
}

#[then(expr = "the sweep skipped {string} and {string} without touching either")]
fn sweep_skipped(w: &mut World, running: String, unproven: String) {
    w.ran()
        .succeeds()
        .contains("it is running")
        .contains("its liveness cannot be proven")
        .contains(&format!("ctxloom session purge {unproven} --even-if-live"));
    for harp in [running, unproven] {
        assert!(
            files::exists(w.session_dir(&harp).join("transcripts/transcript.jsonl")),
            "{harp} was touched"
        );
    }
}

#[then(expr = "the machine-written bulk of {string} is gone")]
fn bulk_gone(w: &mut World, harp: String) {
    assert!(
        !files::exists(w.session_dir(&harp).join("transcripts/transcript.jsonl")),
        "{harp}'s transcript is still there: {}",
        w.ran().said()
    );
}

#[then(expr = "the machine-written bulk of {string} is still there")]
fn bulk_still_there(w: &mut World, harp: String) {
    assert!(
        files::exists(w.session_dir(&harp).join("transcripts/transcript.jsonl")),
        "{harp}'s transcript is gone: {}",
        w.ran().said()
    );
}

#[then(expr = "the sweep kept the transcript of {string} and named {string}")]
fn sweep_kept(w: &mut World, harp: String, named: String) {
    assert!(
        files::exists(w.session_dir(&harp).join("transcripts/transcript.jsonl")),
        "{harp}'s transcript was destroyed"
    );
    w.ran().contains(&harp).contains(&named);
}

// The scratch worktrees (j001300, S11).

#[then("the report names each scratch worktree with its harp, its owner and its verdict")]
fn worktrees_named(w: &mut World) {
    // plan supersedes Go: the owner is named by the state its lock proves,
    // not by the pid Go's lock file carried.
    let ran = w.ran();
    ran.succeeds()
        .contains("ctxloom-wt-clean")
        .contains("ctxloom-wt-wip")
        .contains("amber-quiet-heron")
        .contains("dead")
        .contains("reapable")
        .contains("spared");
}

#[then("only the clean, provably-orphaned worktree is gone from disk")]
fn only_clean_gone(w: &mut World) {
    let mut problems = Vec::new();
    for (harp, name, _) in &w.planted_scratch {
        let dir = w.scratch_dir(harp, name);
        let gone = !files::is_dir(&dir);
        let reapable = name == "clean" && w.reapable(harp);
        if reapable && !gone {
            problems.push(format!(
                "{} is clean, its owner dead, and still on disk",
                dir.display()
            ));
        } else if !reapable && gone {
            problems.push(format!(
                "{} was removed and nothing made it safe",
                dir.display()
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:?}: {}", w.ran().said());
}

#[then("the uncommitted work is still there, spared in place")]
fn wip_spared(w: &mut World) {
    let wip: Vec<_> = w
        .planted_scratch
        .iter()
        .filter(|(_, name, _)| name == "wip")
        .collect();
    assert!(!wip.is_empty(), "no uncommitted work was seeded");
    for (harp, name, _) in wip {
        let body = files::read(w.scratch_dir(harp, name).join("in-flight.go"))
            .unwrap_or_else(|e| panic!("the uncommitted work in {harp} is GONE: {e}"));
        assert!(body.contains(WIP_MARKER), "{body}");
    }
}

#[then("the report says why each spared worktree was left alone")]
fn spared_why(w: &mut World) {
    w.ran().contains("spared").contains("uncommitted changes");
}

#[then(expr = "no worktree of {string} is removed, and ctxloom says it could prove nothing safe")]
fn nothing_safe(w: &mut World, harp: String) {
    let own: Vec<_> = w
        .planted_scratch
        .iter()
        .filter(|(h, _, _)| *h == harp)
        .collect();
    assert!(!own.is_empty(), "no worktree was seeded under {harp}");
    for (h, name, _) in own {
        assert!(
            files::is_dir(w.scratch_dir(h, name)),
            "{h}'s {name} was REMOVED"
        );
    }
    // An action verb that changed nothing refuses.
    w.ran().fails().contains("skipped");
}

#[then("her own long-lived worktree is untouched and was never listed")]
fn foreign_untouched(w: &mut World) {
    assert!(
        !w.planted_foreign.is_empty(),
        "no foreign worktree was seeded"
    );
    for dir in &w.planted_foreign {
        assert!(files::is_dir(dir), "{} was removed", dir.display());
        w.ran().lacks(&dir.display().to_string());
    }
}

#[then("the scratch worktree is still registered with git")]
fn still_registered(w: &mut World) {
    let listed = w.git(w.common.project.dir(), &["worktree", "list", "--porcelain"]);
    for (harp, name, dirty) in &w.planted_scratch {
        if *dirty {
            assert!(
                listed.contains(&w.scratch_dir(harp, name).display().to_string()),
                "{listed}"
            );
        }
    }
}

// ------------------------------------------------------------ the features

common_steps!();

/// Run one feature file with this World; whether it passed.
pub(super) fn run(file: &std::path::Path) -> bool {
    super::gherkin_common::run_feature::<World>(file)
}
