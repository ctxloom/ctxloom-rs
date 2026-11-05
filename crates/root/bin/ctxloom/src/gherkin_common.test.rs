//! What the three cucumber Worlds (`gherkin.test.rs`, `gherkin_surfaces.test.rs`,
//! `gherkin_isolation.test.rs`) share, and the two things that run them all:
//! the feature discovery (`every_feature_runs`) and the parity lint
//! (`gherkin_lint.test.rs` uses the header reader here).
//!
//! A World embeds a [`Common`] (the project, the environment, the last
//! command) and implements [`CommonWorld`]; `common_steps!()` then defines
//! the steps every World has, in a module of its own, against that World.
//! Steps match per World, so a feature's steps all live in one World; the
//! feature's `# world:` header line says which (the convention is at the head
//! of `gherkin_lint.test.rs`).

use std::fmt;
use std::path::{Path, PathBuf};

use cucumber::gherkin::Step;
use serde_json::Value;

use super::library::{reports_as, Project};
use super::Ran;
use crate::tests::files;

/// The state every World has: the project and its home, the environment the
/// commands get, and the last command's outcome.
pub(crate) struct Common {
    pub(crate) project: Project,
    pub(crate) env: Vec<(String, String)>,
    pub(crate) ran: Option<Ran>,
    /// The last command line, as written.
    pub(crate) line: String,
}

impl Common {
    pub(crate) fn new() -> Common {
        Common {
            project: Project::bare(),
            env: Vec::new(),
            ran: None,
            line: String::new(),
        }
    }

    /// The commands of a docstring (one per line) or one line, run in turn
    /// (`$PROJECT_DIR` is the project's directory); the first to fail is the
    /// one the next step sees.
    pub(crate) fn run_lines(&mut self, lines: &str, input: &str) {
        let dir = self.project.dir().display().to_string();
        for line in lines.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let line = line.replace("$PROJECT_DIR", &dir);
            let words = split(&line);
            assert_eq!(words.first().map(String::as_str), Some("ctxloom"), "{line}");
            let args: Vec<&str> = words[1..].iter().map(String::as_str).collect();
            let env: Vec<(&str, &str)> = self
                .env
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let ran = if input.is_empty() {
                self.project.run_with(&env, &args)
            } else {
                self.project
                    .run_input(&args, &input.replace("$PROJECT_DIR", &dir), &env)
            };
            let failed = ran.code != 0;
            self.line = line;
            self.ran = Some(ran);
            if failed {
                return;
            }
        }
    }

    pub(crate) fn ran(&self) -> &Ran {
        self.ran.as_ref().expect("a command ran")
    }

    pub(crate) fn json(&self) -> Value {
        super::parse_one(&self.ran().stdout)
    }

    pub(crate) fn set_env(&mut self, name: &str, value: String) {
        self.env.retain(|(n, _)| n != name);
        self.env.push((name.to_string(), value));
    }

    /// Whether the last command asked for the text form: an explicit
    /// `--format text`. Off a terminal the derived default is json.
    pub(crate) fn text_asked(&self) -> bool {
        let words = split(&self.line);
        words
            .windows(2)
            .any(|pair| pair[0] == "--format" && pair[1] == "text")
    }

    /// Go: "the output contains", with the two words clap says differently
    /// from cobra (plan 8.2: the command line is clap's): its usage error
    /// says "unexpected argument" where cobra said "unknown flag", and its
    /// heading is "Commands:" where cobra's was "Available Commands:".
    pub(crate) fn clap_wording(text: &str) -> &str {
        match text {
            "unknown flag" => "unexpected argument",
            "Available Commands:" => "Commands:",
            other => other,
        }
    }

    pub(crate) fn output_reports(&self, path: &str, value: &str) {
        let ran = self.ran();
        match serde_json::from_str::<Value>(&ran.stdout) {
            Ok(doc) => assert_eq!(reports_as(&doc, path), value, "{}", ran.said()),
            Err(_) => {
                ran.contains(value);
            }
        }
    }
}

/// What a World gives the shared steps.
pub(crate) trait CommonWorld {
    fn common(&self) -> &Common;
    fn common_mut(&mut self) -> &mut Common;

    /// What a World does before each command (plants its sessions).
    fn before_run(&mut self) {}

    fn ran(&self) -> &Ran {
        self.common().ran()
    }

    fn json(&self) -> Value {
        self.common().json()
    }
}

/// A command line's words: whitespace separates, double quotes group.
pub(crate) fn split(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut any) = (false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    words.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            c => {
                word.push(c);
                any = true;
            }
        }
    }
    if any {
        words.push(word);
    }
    words
}

/// A step's docstring, trimmed.
pub(crate) fn doc(step: &Step) -> String {
    step.docstring
        .clone()
        .expect("a docstring")
        .trim()
        .to_string()
}

/// Define the steps every World has, in a module of its own, against the
/// `World` in scope (which implements [`CommonWorld`]).
macro_rules! common_steps {
    () => {
        mod common {
            use cucumber::gherkin::Step;
            use cucumber::{given, then, when};

            use super::World;
            use $crate::tests::gherkin_common::{doc, Common, CommonWorld};
            use $crate::tests::library::Project;

            #[given("an initialized ctxloom project")]
            fn initialized(w: &mut World) {
                w.common_mut().project = Project::initialized();
            }

            #[given("an empty project directory")]
            fn empty(w: &mut World) {
                w.common_mut().project = Project::bare();
            }

            #[given(expr = "the project already has the file {string}:")]
            fn project_file(w: &mut World, step: &Step, rel: String) {
                w.common().project.write(&rel, &format!("{}\n", doc(step)));
            }

            #[given(expr = "I run {string}")]
            #[when(expr = "I run {string}")]
            fn run(w: &mut World, line: String) {
                w.before_run();
                w.common_mut().run_lines(&line, "");
            }

            #[given(expr = "I run {string} with input:")]
            #[when(expr = "I run {string} with input:")]
            fn run_with_input(w: &mut World, step: &Step, line: String) {
                w.before_run();
                w.common_mut().run_lines(&line, &doc(step));
            }

            /// Every `Alice ...:` step carries its commands in a docstring.
            #[when(regex = r"^Alice .*:$")]
            fn alice_runs(w: &mut World, step: &Step) {
                w.before_run();
                w.common_mut().run_lines(&doc(step), "");
            }

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

            #[then(expr = "the output contains {string}")]
            fn output_contains(w: &mut World, text: String) {
                w.ran().contains(Common::clap_wording(&text));
            }

            #[then(expr = "the output does not contain {string}")]
            fn output_lacks(w: &mut World, text: String) {
                w.ran().lacks(Common::clap_wording(&text));
            }

            #[then("the output is valid JSON")]
            fn valid_json(w: &mut World) {
                let _ = w.json();
            }

            #[then(expr = "the output reports {string} as {string}")]
            fn output_reports(w: &mut World, path: String, value: String) {
                w.common().output_reports(&path, &value);
            }

            #[given(expr = "the file {string} contains {string}")]
            #[then(expr = "the file {string} contains {string}")]
            fn file_contains(w: &mut World, rel: String, text: String) {
                w.common().project.file_contains(&rel, &text);
            }

            #[then(expr = "the file {string} contains {string} exactly {int} times")]
            fn file_contains_times(w: &mut World, rel: String, text: String, times: usize) {
                w.common().project.file_counts(&rel, &text, times);
            }

            #[then(expr = "the file {string} does not contain {string}")]
            fn file_lacks(w: &mut World, rel: String, text: String) {
                w.common().project.file_lacks(&rel, &text);
            }

            #[then(expr = "the file {string} exists")]
            fn file_exists(w: &mut World, rel: String) {
                assert!(w.common().project.exists(&rel), "{rel} should exist");
            }

            #[then(expr = "the file {string} does not exist")]
            fn file_absent(w: &mut World, rel: String) {
                w.common().project.file_is_absent(&rel);
            }
        }
    };
}
pub(crate) use common_steps;

/// Run one feature file with the World `W`; whether it passed.
pub(crate) fn run_feature<W>(file: &Path) -> bool
where
    W: cucumber::World + cucumber::codegen::WorldInventory + fmt::Debug,
{
    let writer = futures::executor::block_on(
        W::cucumber()
            // The test harness's own arguments (`--exact`) are not ours.
            .with_cli(cucumber::cli::Opts::<_, _, _, cucumber::cli::Empty>::default())
            .max_concurrent_scenarios(4)
            .fail_on_skipped()
            .run(file),
    );
    !cucumber::writer::Stats::execution_has_failed(&writer)
}

/// The features directory.
pub(crate) fn features_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("features")
}

/// Every `.feature` file under `features/`, as a path relative to it with
/// `/` separators, sorted.
pub(crate) fn feature_files() -> Vec<String> {
    fn walk(dir: &Path, rel: &str, found: &mut Vec<String>) {
        let mut entries = files::children(dir);
        entries.sort();
        for path in entries {
            let name = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            let rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if files::is_dir(&path) {
                walk(&path, &rel, found);
            } else if name.ends_with(".feature") {
                found.push(rel);
            }
        }
    }
    let mut found = Vec::new();
    walk(&features_dir(), "", &mut found);
    found
}

/// The value of each header line that starts with `prefix` (`# world:`,
/// `# NOT ported:`, `# Ported from`) among the comment lines at the head of
/// a feature file, in order.
pub(crate) fn header_values<'a>(text: &'a str, prefix: &str) -> Vec<&'a str> {
    header_lines(text)
        .filter_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .collect()
}

/// A feature file's header: the leading run of comment lines, blank lines
/// allowed.
pub(crate) fn header_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(str::trim_end)
        .take_while(|l| l.is_empty() || l.starts_with('#'))
}

/// Which World runs a feature, from its `# world:` header line.
pub(crate) fn world_of(text: &str) -> Result<&str, String> {
    match header_values(text, "# world:").as_slice() {
        [world @ ("base" | "surfaces" | "isolation")] => Ok(world),
        [] => Err("no `# world: base|surfaces|isolation` header line".to_string()),
        other => Err(format!(
            "`# world:` must be one of base, surfaces, isolation, once: {other:?}"
        )),
    }
}
