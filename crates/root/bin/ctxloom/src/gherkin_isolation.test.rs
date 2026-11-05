//! Gherkin features (S11, the standalone half), run with cucumber-rs over the
//! real binary: Go ctxloom's own `cli/container.feature` (revision
//! f36e8dfb21, release/0.7; the file's header names what was left out) with
//! new Rust step definitions.
//!
//! plan supersedes Go, where a step differs:
//! - "Available Commands:" is clap's "Commands:" heading (plan 8.2: the
//!   command line is clap's, not cobra's).
//! - "no container runtime is installed" hides docker and podman by giving
//!   the process a PATH with neither, as Go's mask does.
//!
//! The worktree scenarios (cli/session_worktrees.feature and the sweep) run
//! in gherkin.test.rs beside the session steps they share.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use cucumber::gherkin::Step;
use cucumber::{given, then, when, World as _};
use serde_json::Value;

use super::library::{at, reports_as, Project};
use super::Ran;
use crate::tests::files;

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct World {
    project: Project,
    env: Vec<(String, String)>,
    ran: Option<Ran>,
    /// The words of the last command line.
    line: String,
    /// Fake companions, and the empty directory that hides the runtimes.
    bin: Option<file_system::Rooted>,
    recorded: Option<BTreeMap<String, Vec<u8>>>,
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "World")
    }
}

impl World {
    fn new() -> World {
        World {
            project: Project::bare(),
            env: Vec::new(),
            ran: None,
            line: String::new(),
            bin: None,
            recorded: None,
        }
    }

    fn set_env(&mut self, name: &str, value: String) {
        self.env.retain(|(n, _)| n != name);
        self.env.push((name.to_string(), value));
    }

    /// The directory of fakes, put first on the PATH with the system's.
    fn bin(&mut self) -> &Path {
        if self.bin.is_none() {
            let dir = files::root();
            let mut paths = vec![dir.root().to_path_buf()];
            paths.extend(files::search_path(
                &crate::tests::host_var("PATH").unwrap_or_default(),
            ));
            let path = std::env::join_paths(paths).expect("a PATH");
            self.bin = Some(dir);
            self.set_env("PATH", path.to_string_lossy().into_owned());
        }
        self.bin.as_ref().expect("just made").root()
    }

    fn run(&mut self, line: &str) {
        let words: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(words.first(), Some(&"ctxloom"), "{line}");
        let env: Vec<(&str, &str)> = self
            .env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        self.line = line.to_string();
        self.ran = Some(self.project.run_with(&env, &words[1..]));
    }

    fn ran(&self) -> &Ran {
        self.ran.as_ref().expect("a command ran")
    }

    /// Whether the last command asked for the text form: an explicit
    /// `--format text`. Off a terminal the derived default is json.
    fn text_asked(&self) -> bool {
        let words: Vec<&str> = self.line.split_whitespace().collect();
        words
            .windows(2)
            .any(|pair| pair[0] == "--format" && pair[1] == "text")
    }

    /// A fake `ctxloom-companion-<name>` declaring `marker` as its typed
    /// `init.tooling` (Go: `toolingCompanion`).
    fn companion(&mut self, name: &str, marker: &str) {
        let bin = format!("ctxloom-companion-{name}");
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\n  loadout) printf 'init:\\n  tooling: \"%s\"\\n' '{marker}: install the tools this companion content needs.';;\n  version) printf '{{\"name\":\"{bin}\",\"version\":\"0.0.0-fixture\"}}\\n';;\nesac\n"
        );
        let path = self.bin().join(&bin);
        files::put_exec(&path, &script);
    }
}

fn doc(step: &Step) -> String {
    step.docstring
        .clone()
        .expect("a docstring")
        .trim()
        .to_string()
}

// ------------------------------------------------------------------ given

#[given("an initialized ctxloom project")]
fn initialized(w: &mut World) {
    w.project = Project::initialized();
}

#[given(expr = "the mock LLM responds {string}")]
fn mock_responds(w: &mut World, _reply: String) {
    // The mock engine is the project's primary: a bare `container check`
    // resolves it.
    w.project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\nllm:\n  configs:\n    mock:\n      type: mock\n  defaults:\n    primary: mock\n",
    );
}

#[given(expr = "the project already has the file {string}:")]
fn project_file(w: &mut World, step: &Step, rel: String) {
    w.project.write(&rel, &format!("{}\n", doc(step)));
}

#[given("no container runtime is installed")]
fn no_runtime(w: &mut World) {
    // PATH holds neither docker nor podman, nor anything else.
    let empty = files::root();
    let path = empty.root().to_string_lossy().into_owned();
    w.bin = Some(empty);
    w.set_env("PATH", path);
}

#[given(
    expr = "a companion {string} declaring container tooling {string}, on PATH but not registered"
)]
fn unregistered_companion(w: &mut World, name: String, marker: String) {
    w.companion(&name, &marker);
}

#[given(expr = "a companion {string} declaring container tooling {string}")]
fn registered_companion(w: &mut World, name: String, marker: String) {
    w.companion(&name, &marker);
    w.run(&format!("ctxloom companion add {name}"));
    w.ran().succeeds();
}

#[given("I record the project tree")]
fn record_tree(w: &mut World) {
    w.recorded = Some(w.project.tree("."));
}

// ------------------------------------------------------------------- when

#[when(expr = "I run {string}")]
fn run(w: &mut World, line: String) {
    w.run(&line);
}

#[when(regex = r"^Alice .*:$")]
fn alice_runs(w: &mut World, step: &Step) {
    w.run(&doc(step));
}

// ------------------------------------------------------------------- then

#[then("the command succeeds")]
fn succeeds(w: &mut World) {
    w.ran().succeeds();
}

#[then("the command fails")]
fn fails(w: &mut World) {
    w.ran().fails();
}

#[then(expr = "the command exits with code {int}")]
fn exits(w: &mut World, code: i32) {
    assert_eq!(w.ran().code, code, "{}", w.ran().said());
}

/// clap's heading for cobra's.
fn heading(text: &str) -> &str {
    if text == "Available Commands:" {
        "Commands:"
    } else {
        text
    }
}

#[then(expr = "the output contains {string}")]
fn output_contains(w: &mut World, text: String) {
    w.ran().contains(heading(&text));
}

#[then(expr = "the output does not contain {string}")]
fn output_lacks(w: &mut World, text: String) {
    w.ran().lacks(heading(&text));
}

#[then("the output is valid JSON")]
fn valid_json(w: &mut World) {
    let _ = w.ran().json();
}

#[then(expr = "the output reports {string} as empty, saying {string}")]
fn reports_empty(w: &mut World, path: String, saying: String) {
    if w.text_asked() {
        w.ran().contains(&saying);
        return;
    }
    let doc = w.ran().json();
    match at(&doc, &path) {
        None | Some(Value::Null) => {}
        Some(Value::Array(a)) => assert!(a.is_empty(), "{path}: {a:?}"),
        Some(other) => panic!("{path} is {other}, so the command did find something"),
    }
}

#[then(expr = "the output reports {string} matching {string}")]
fn reports_matching(w: &mut World, path: String, expected: String) {
    if w.text_asked() {
        // The rendered forms carry their prose: matched as a substring.
        w.ran().contains(&expected);
        return;
    }
    let doc = w.ran().json();
    // The hermetic rows give literal expectations; the pattern rows
    // (`^(true|false)$`, `\S`) are the excluded live Rule's.
    let got = reports_as(&doc, &path);
    assert!(
        got.contains(&expected),
        "{path} is {got:?}, not {expected:?}"
    );
}

#[then("the project tree is unchanged")]
fn tree_unchanged(w: &mut World) {
    let before = w.recorded.as_ref().expect("a recorded tree");
    assert_eq!(&w.project.tree("."), before);
}

#[then(expr = "the file {string} contains {string}")]
fn file_contains(w: &mut World, rel: String, text: String) {
    w.project.file_contains(&rel, &text);
}

#[then(expr = "the file {string} does not contain {string}")]
fn file_lacks(w: &mut World, rel: String, text: String) {
    w.project.file_lacks(&rel, &text);
}

#[then(expr = "the file {string} does not exist")]
fn file_absent(w: &mut World, rel: String) {
    w.project.file_is_absent(&rel);
}

// ------------------------------------------------------------ the features

#[test]
fn test_container_feature() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("features")
        .join("cli/container.feature");
    let writer = futures::executor::block_on(
        World::cucumber()
            // The test harness's own arguments (`--exact`) are not ours.
            .with_cli(cucumber::cli::Opts::<_, _, _, cucumber::cli::Empty>::default())
            .max_concurrent_scenarios(4)
            .fail_on_skipped()
            .run(file),
    );
    assert!(
        !cucumber::writer::Stats::execution_has_failed(&writer),
        "cli/container.feature failed"
    );
}
