//! The steps of j000100 (adopting ctxloom, and backing out), as a child of
//! the base World: Alice runs `ctxloom init` on an empty project directory,
//! looks at what it did, and leaves by deleting `.ctxloom`.
//!
//! Hermetic: ctxloom's published default content (`ctxloom-default`, which
//! the seeded default profile inherits from) is a bare repository in the
//! scenario's temporary directory, and git reaches it through the home's own
//! git configuration (`url.<fixture>.insteadOf`), so no ctxloom flag or
//! variable is needed and nothing reaches the network. The fixture is laid
//! out as `deps pull` reads a remote today: the bundle `ai-developer` with a
//! `developer` profile, whose fragment carries [`PUBLISHED_MARKER`].
//!
//! Claude Code is a stub first on `PATH`, so the engine init finds is the
//! same on every machine. It records what it was launched with: its
//! arguments, every file an argument names, its standard input, and every
//! file of the configuration directory it was pointed at (a session home). A
//! session "was handed" the published default when that record carries the
//! marker; the record's shape is to be confirmed when `run` launches an
//! engine. Alice's own assistant configuration, in her home, is a settings
//! file and an instructions file of hers, so "exactly as it was" compares
//! something she wrote.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cucumber::{given, then, when};

use super::super::deps::git_in;
use super::super::gherkin_common::CommonWorld;
use super::World;
use crate::tests::files;

/// The address the seeded default profile's parent names.
const PUBLISHED_DEFAULT: &str = "https://github.com/ctxloom/ctxloom-default";

/// What the published default's fragment says.
const PUBLISHED_MARKER: &str = "J000100-PUBLISHED-DEFAULT-MARKER";

/// The stub engine: records its launch beside itself, and replies.
const CLAUDE_STUB: &str = r#"rec="$(dirname "$0")/claude-launch"
for a in "$@"; do
  printf '%s\n' "$a" >> "$rec"
  [ -f "$a" ] && cat "$a" >> "$rec"
done
if [ -n "${CLAUDE_CONFIG_DIR:-}" ] && [ -d "$CLAUDE_CONFIG_DIR" ]; then
  find "$CLAUDE_CONFIG_DIR" -type f -exec cat {} + >> "$rec" 2>/dev/null
fi
[ -t 0 ] || cat >> "$rec"
echo J000100-ASSISTANT-REPLY
"#;

/// Where Alice's own assistant keeps its configuration, in her home.
const ASSISTANT_HOME: &[&str] = &[".claude", ".claude.json"];

/// A project as it stands: every file (path and bytes), and the names of
/// its top-level entries, sorted.
type Snapshot = (BTreeMap<String, Vec<u8>>, Vec<String>);

/// What the j000100 scenarios keep between steps.
#[derive(Default)]
pub(super) struct State {
    /// Every file of the project, and its top-level entries, before the
    /// first ctxloom command ran.
    before: Option<Snapshot>,
    /// Her assistant's own configuration in her home, then.
    assistant_before: Option<BTreeMap<String, Vec<u8>>>,
}

/// Every file below `path` (or `path` itself, a file), by its path.
fn files_below(path: &std::path::Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if files::is_dir(path) {
        for child in files::children(path) {
            files_below(&child, out);
        }
    } else if let Ok(bytes) = files::read_bytes(path) {
        out.insert(path.display().to_string(), bytes);
    }
}

impl World {
    /// Record the project as it is, once, before ctxloom first runs in it.
    pub(super) fn adopt_record(&mut self) {
        if self.adopt.before.is_none() {
            self.adopt.before = Some(self.project_now());
            self.adopt.assistant_before = Some(self.assistant_now());
        }
    }

    fn project_now(&self) -> Snapshot {
        let project = &self.common.project;
        let mut top: Vec<String> = files::children(project.dir())
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        top.sort();
        (project.tree("."), top)
    }

    fn assistant_now(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for rel in ASSISTANT_HOME {
            files_below(&self.common.project.home().join(rel), &mut out);
        }
        out
    }

    fn launch_record(&self) -> String {
        files::read(self.adopt_scratch().join("bin/claude-launch")).unwrap_or_default()
    }

    /// The scenario's temporary directory, beside the project and the home.
    fn adopt_scratch(&self) -> PathBuf {
        self.common
            .project
            .dir()
            .parent()
            .expect("the project is in the scenario's directory")
            .to_path_buf()
    }
}

fn outside_ctxloom<V: Clone>(tree: &BTreeMap<String, V>) -> BTreeMap<String, V> {
    tree.iter()
        .filter(|(path, _)| !path.starts_with(".ctxloom/"))
        .map(|(path, v)| (path.clone(), v.clone()))
        .collect()
}

#[given("Alice's assistant is Claude Code")]
fn claude_code(w: &mut World) {
    let bin = w.adopt_scratch().join("bin");
    files::put_program(bin.join("claude"), CLAUDE_STUB);
    let project = &w.common.project;
    project.write_home(".claude/settings.json", "{\n  \"theme\": \"dark\"\n}\n");
    project.write_home(".claude/CLAUDE.md", "Alice's own instructions: be brief.\n");
    let host = crate::tests::host_var("PATH").unwrap_or_default();
    let path = format!("{}:{}", bin.display(), host.to_string_lossy());
    w.common.set_env("PATH", path);
}

#[given("ctxloom's published default content is within reach")]
fn published_default(w: &mut World) {
    let root = w.adopt_scratch().join("published");
    let (bare, work) = (root.join("ctxloom-default"), root.join("work"));
    files::mkdirs(&root);
    git_in(
        &root,
        &["init", "-q", "--bare", "-b", "main", "ctxloom-default"],
    );
    git_in(&root, &["init", "-q", "-b", "main", "work"]);
    let bundle = work.join(".ctxloom/content/bundles/v2/ai-developer");
    files::put(
        bundle.join("bundle.yaml"),
        "version: \"1.0.0\"\ndescription: \"ctxloom's published developer content\"\n",
    );
    files::put(
        bundle.join("fragments/published-default.md"),
        format!("{PUBLISHED_MARKER}\n"),
    );
    files::put(
        bundle.join("profiles/developer.yaml"),
        "bundles:\n  - ai-developer\n",
    );
    git_in(&work, &["add", "-A"]);
    git_in(&work, &["commit", "-q", "-m", "published"]);
    git_in(
        &work,
        &["push", "-q", bare.to_str().expect("utf-8"), "main"],
    );
    // Longest match wins, so the `.git` spelling maps to the same repository.
    let to = format!("file://{}", bare.display());
    w.common.project.write_home(
        ".gitconfig",
        &format!(
            "[url \"{to}\"]\n\tinsteadOf = {PUBLISHED_DEFAULT}.git\n\tinsteadOf = {PUBLISHED_DEFAULT}\n"
        ),
    );
}

/// The token every agent authenticates with: a fixture, so init's check
/// for it passes and no real credential is read.
#[given("Alice has exported her assistant's token")]
fn token(w: &mut World) {
    w.common.set_env(
        "CLAUDE_CODE_OAUTH_TOKEN",
        "sk-ant-oat01-acceptance-fixture-not-real".to_string(), // gitleaks:allow
    );
}

#[given("Alice has set ctxloom up in her project")]
fn set_up(w: &mut World) {
    w.before_run();
    w.common.run_lines("ctxloom init", "");
    w.ran().succeeds();
}

#[given("Alice has started her assistant through ctxloom")]
fn started_through_ctxloom(w: &mut World) {
    w.before_run();
    w.common.run_lines(
        "ctxloom run --one-shot \"what does this project expect of me?\"",
        "",
    );
    w.ran().succeeds();
}

#[then("her assistant was handed ctxloom's published default content")]
fn handed_published_default(w: &mut World) {
    let record = w.launch_record();
    assert!(
        record.contains(PUBLISHED_MARKER),
        "the session's launch carried no published default content: {record}"
    );
}

/// Her assistant's own configuration, in her home and in her project, as it
/// was before ctxloom first ran.
#[then("her assistant, started on its own, is exactly as it was before she set ctxloom up")]
fn assistant_as_it_was(w: &mut World) {
    let before = w.adopt.assistant_before.clone().expect("a command ran");
    assert!(
        w.assistant_now() == before,
        "her assistant's own configuration changed: now {:?}, before {:?}",
        w.assistant_now().keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>()
    );
    let in_project = |tree: BTreeMap<String, Vec<u8>>| -> BTreeMap<String, Vec<u8>> {
        tree.into_iter()
            .filter(|(p, _)| p.starts_with(".claude/") || p == ".mcp.json" || p == "CLAUDE.md")
            .collect()
    };
    let was = in_project(w.adopt.before.clone().expect("a command ran").0);
    assert_eq!(
        in_project(w.project_now().0),
        was,
        "her project's assistant files changed"
    );
}

/// Asserted before the removal, so the scenario starts from a project that
/// demonstrably has something to remove.
#[given("her project has a .ctxloom directory")]
fn has_ctxloom(w: &mut World) {
    assert!(
        files::is_dir(w.common.project.dir().join(".ctxloom")),
        "no .ctxloom in the project"
    );
}

#[when("Alice takes ctxloom back out by deleting its directory")]
fn delete_ctxloom(w: &mut World) {
    files::remove_tree(w.common.project.dir().join(".ctxloom"));
}

/// Asserted on the whole tree, not on a list of well-known files: any file
/// init writes outside `.ctxloom`, under any name, is one Alice would have
/// to find and delete by hand, and her exit is deleting `.ctxloom` alone.
/// (What init keeps under `~/.ctxloom` is a cache shared across projects,
/// and is left when she goes.)
#[then("nothing in her project outside .ctxloom was written")]
fn kept_to_its_directory(w: &mut World) {
    let (before_files, before_top) = w.adopt.before.clone().expect("a command ran");
    let (now_files, now_top) = w.project_now();
    let top = |names: Vec<String>| -> Vec<String> {
        names.into_iter().filter(|n| n != ".ctxloom").collect()
    };
    assert_eq!(
        top(now_top),
        top(before_top),
        "init left entries in the project outside .ctxloom"
    );
    let (now, before) = (outside_ctxloom(&now_files), outside_ctxloom(&before_files));
    let changed: Vec<&String> = now
        .keys()
        .chain(before.keys())
        .filter(|k| now.get(*k) != before.get(*k))
        .collect();
    assert!(
        changed.is_empty(),
        "init wrote outside .ctxloom: {changed:?}"
    );
}

#[then("her project is exactly as it was before she set ctxloom up")]
fn as_it_was(w: &mut World) {
    let before = w.adopt.before.clone().expect("a command ran");
    assert!(
        w.project_now() == before,
        "the project is not as it was before init: now {:?}, before {:?}",
        w.project_now().1,
        before.1
    );
}

/// Go's `steps_j000100_adopt.go`: doctor names a check for each part of the
/// harness. doctor needs no network, engine or credentials, which is what
/// makes it the check Alice can run before she trusts anything else.
#[then("she is shown a health check for every part of the harness")]
fn health_check(w: &mut World) {
    for marker in [
        "DOCTOR-CHECK-DEPS-a1",
        "DOCTOR-CHECK-AGENTS-b2",
        "DOCTOR-CHECK-HOOKS-TRUST-d4",
    ] {
        w.ran().contains(marker);
    }
}
