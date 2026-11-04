//! Acceptance: the real `ctxloom` binary, run as a separate process.
//!
//! S0's red tests (`port/foundation`). The tests live beside the code (no
//! `tests/` directory), where cargo does not build the binary for them; so
//! the first test to need it builds it, once, with the cargo running these
//! tests, into the same target directory and profile.
//!
//! THE TEST-ONLY SURFACE this file proposes (built only with the binary's
//! `test-support` feature; S0 has no real command that does these):
//!
//! - `ctxloom test refuse`: a refusal (exit 2) that carries a remedy.
//! - `ctxloom test findings`: fatal findings (exit 3).
//! - `ctxloom test sample [--empty]`: a sample list document. Two rows,
//!   `{"name": "alpha", "count": 1}` and `{"name": "beta", "count": 2}`;
//!   with `--empty`, no rows.
//! - `ctxloom test diagnostics`: one JSON array of the `{code, args,
//!   message}` envelope of every diagnostic the binary can raise (every
//!   crate's samples), rendered from the binary's catalog.
//! - `ctxloom test session-id`: a minted session harp, on a line.
//! - `ctxloom test run-id <session-uuid> <incarnation>`: the run id of that
//!   incarnation of that session, on a line.
//! - `ctxloom test open-store <project-id>`: opens the project's store
//!   (`Stores::open`) under the process's home and prints its path.
//!
//! These tests cannot use angzarr-presentation, angzarr-diagnostic or the
//! file-system seam yet: S0 has not added their pins. So they read the
//! binary's output, the lock file, the manifests and the catalogs' text
//! instead of the crates' types.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use serde_json::Value;

/// The `ctxloom` binary, built once per test run with its test-only
/// commands.
pub(crate) fn binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        // This test executable is <target>/<profile>/deps/ctxloom-<hash>.
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
                "ctxloom",
                "--bin",
                "ctxloom",
                "--features",
                "test-support",
            ])
            .arg("--manifest-path")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .env("CARGO_TARGET_DIR", target_dir);
        if profile_dir.file_name().is_some_and(|p| p == "release") {
            build.arg("--release");
        }
        let status = build.status().expect("cargo runs");
        assert!(status.success(), "building ctxloom failed");
        profile_dir.join(format!("ctxloom{}", std::env::consts::EXE_SUFFIX))
    })
}

/// One finished process.
pub(crate) struct Ran {
    /// The exit code; -1 when a signal ended the process.
    pub(crate) code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Ran {
    pub(crate) fn of(output: Output) -> Ran {
        Ran {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// stdout as one JSON document.
    pub(crate) fn json(&self) -> Value {
        parse_one(&self.stdout)
    }

    /// stderr as one JSON document.
    pub(crate) fn err_json(&self) -> Value {
        parse_one(&self.stderr)
    }

    /// A description of the run, for an assertion message.
    pub(crate) fn said(&self) -> String {
        format!(
            "exit {}; stdout {:?}; stderr {:?}",
            self.code, self.stdout, self.stderr
        )
    }
}

/// `text` as exactly one JSON document, as jq reads it.
pub(crate) fn parse_one(text: &str) -> Value {
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

/// An isolated home: a process sees nothing of this one's environment but
/// what a process needs to start, and this directory as its home and its
/// working directory. Never the real home.
pub(crate) struct Env {
    home: tempfile::TempDir,
}

impl Env {
    pub(crate) fn new() -> Env {
        Env {
            home: tempfile::tempdir().expect("a temporary home"),
        }
    }

    pub(crate) fn home(&self) -> &Path {
        self.home.path()
    }

    /// Run `ctxloom args`.
    pub(crate) fn run(&self, args: &[&str]) -> Ran {
        let mut command = Command::new(binary());
        command
            .args(args)
            .env_clear()
            .env("HOME", self.home())
            .env("USERPROFILE", self.home())
            .current_dir(self.home());
        for name in ["PATH", "SystemRoot"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        Ran::of(command.output().expect("ctxloom runs"))
    }
}

/// Run `ctxloom args` in a fresh home.
pub(crate) fn run(args: &[&str]) -> Ran {
    Env::new().run(args)
}

/// The repository's root: this crate is crates/root/bin/ctxloom.
pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

#[path = "catalog.test.rs"]
mod catalog;

#[path = "deps.test.rs"]
mod deps;

#[path = "errors.test.rs"]
mod errors;

#[path = "exit.test.rs"]
mod exit;

#[path = "format.test.rs"]
mod format;

#[path = "gherkin.test.rs"]
mod gherkin;

#[path = "hook.test.rs"]
mod hook;

#[path = "ids.test.rs"]
mod ids;

#[path = "library.test.rs"]
mod library;

#[path = "library_cli.test.rs"]
mod library_cli;

#[path = "library_config.test.rs"]
mod library_config;

#[path = "library_parity.test.rs"]
mod library_parity;

#[path = "library_write_agent.test.rs"]
mod library_write_agent;

#[path = "library_write_bundle.test.rs"]
mod library_write_bundle;

#[path = "library_write_companion.test.rs"]
mod library_write_companion;

#[path = "library_write_config.test.rs"]
mod library_write_config;

#[path = "library_write_items.test.rs"]
mod library_write_items;

#[path = "library_write_llm.test.rs"]
mod library_write_llm;

#[path = "library_write_loadout.test.rs"]
mod library_write_loadout;

#[path = "library_write_mcp.test.rs"]
mod library_write_mcp;

#[path = "library_write_profile.test.rs"]
mod library_write_profile;

#[path = "library_write_skill.test.rs"]
mod library_write_skill;

#[path = "session.test.rs"]
mod session;

#[path = "gherkin_surfaces.test.rs"]
mod gherkin_surfaces;

#[path = "manage.test.rs"]
mod manage;

#[path = "materialize.test.rs"]
mod materialize;

#[path = "post_tool_hooks.test.rs"]
mod post_tool_hooks;

#[path = "session_start.test.rs"]
mod session_start;

#[path = "surfaces.test.rs"]
mod surfaces;

#[path = "util.test.rs"]
mod util;

#[path = "usage.test.rs"]
mod usage;

#[path = "version.test.rs"]
mod version;

#[path = "workspace.test.rs"]
mod workspace;
