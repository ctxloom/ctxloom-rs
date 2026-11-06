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

use cucumber::{given, then};
use serde_json::Value;

use super::gherkin_common::{common_steps, Common, CommonWorld};
use super::library::{at, reports_as};
use crate::tests::files;

#[derive(cucumber::World)]
#[world(init = Self::new)]
struct World {
    common: Common,
    /// Fake companions, and the empty directory that hides the runtimes.
    bin: Option<file_system::Rooted>,
    recorded: Option<BTreeMap<String, Vec<u8>>>,
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
}

impl World {
    fn new() -> World {
        World {
            common: Common::new(),
            bin: None,
            recorded: None,
        }
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
            self.common
                .set_env("PATH", path.to_string_lossy().into_owned());
        }
        self.bin.as_ref().expect("just made").root()
    }

    /// A fake `ctxloom-companion-<name>` declaring `marker` as its typed
    /// `init.tooling` (Go: `toolingCompanion`).
    fn companion(&mut self, name: &str, marker: &str) {
        let bin = format!("ctxloom-companion-{name}");
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\n  loadout) printf 'init:\\n  tooling: \"%s\"\\n' '{marker}: install the tools this companion content needs.';;\n  version) printf '{{\"name\":\"{bin}\",\"version\":\"0.0.0-fixture\"}}\\n';;\nesac\n"
        );
        let path = self.bin().join(&bin);
        files::put_program(&path, &script);
    }
}

// ------------------------------------------------------------------ given

#[given(expr = "the mock LLM responds {string}")]
fn mock_responds(w: &mut World, _reply: String) {
    // The mock engine is the project's primary: a bare `container check`
    // resolves it.
    w.common.project.write(
        ".ctxloom/config.yaml",
        "schema_version: 9\nllm:\n  configs:\n    mock:\n      type: mock\n  defaults:\n    primary: mock\n",
    );
}

#[given("no container runtime is installed")]
fn no_runtime(w: &mut World) {
    // PATH holds neither docker nor podman, nor anything else.
    let empty = files::root();
    let path = empty.root().to_string_lossy().into_owned();
    w.bin = Some(empty);
    w.common.set_env("PATH", path);
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
    // Registered with the command that runs it: nothing is looked up by name.
    let program = w.bin().join(format!("ctxloom-companion-{name}"));
    w.common.run_lines(
        &format!(
            "ctxloom companion add {name} --command {}",
            program.display()
        ),
        "",
    );
    w.ran().succeeds();
}

#[given("I record the project tree")]
fn record_tree(w: &mut World) {
    w.recorded = Some(w.common.project.tree("."));
}

// ------------------------------------------------------------------- when

// ------------------------------------------------------------------- then

#[then(expr = "the output reports {string} as empty, saying {string}")]
fn reports_empty(w: &mut World, path: String, saying: String) {
    if w.common.text_asked() {
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
    if w.common.text_asked() {
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
    assert_eq!(&w.common.project.tree("."), before);
}

// ------------------------------------------------------------ the features

common_steps!();

/// Run one feature file with this World; whether it passed.
pub(super) fn run(file: &Path) -> bool {
    super::gherkin_common::run_feature::<World>(file)
}
