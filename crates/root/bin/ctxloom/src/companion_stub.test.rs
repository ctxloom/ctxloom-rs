//! The stub companion for the companion protocol's tests: the compiled
//! `ctxloom-companion-stub` (the `stub-companion` crate), a real program the
//! binary under test execs, answering `version` and `loadout --format yaml`
//! from files this module writes. One built binary stands in for any
//! companion: a test registers `[stub, --files, <dir>/<name>]` as the
//! command and the files beside that prefix say what it prints. Nothing is
//! written executable and nothing is looked up on a `PATH`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use crate::tests::files;

/// The compiled stub, built once per test run.
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
                "stub-companion",
                "--bin",
                "ctxloom-companion-stub",
            ])
            .arg("--manifest-path")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .env("CARGO_TARGET_DIR", target_dir);
        if profile_dir.file_name().is_some_and(|p| p == "release") {
            build.arg("--release");
        }
        let status = build.status().expect("cargo runs");
        assert!(status.success(), "building the stub companion failed");
        profile_dir.join(format!(
            "ctxloom-companion-stub{}",
            std::env::consts::EXE_SUFFIX
        ))
    })
}

/// How the stub misbehaves, where it does.
#[derive(Clone, Copy)]
pub(crate) enum Misbehaving {
    /// It exits with this status instead of printing a loadout.
    Exits(i32),
    /// It never answers: the caller's time limit ends it.
    Hangs,
}

/// A stub companion standing in for `name`, whose files are under `dir`.
pub(crate) struct Stub {
    prefix: PathBuf,
}

impl Stub {
    /// A stub that prints `loadout` for `loadout --format yaml`.
    pub(crate) fn install(dir: &Path, name: &str, loadout: &str) -> Stub {
        let prefix = dir.join(name);
        files::put(format!("{}.loadout", prefix.display()), loadout);
        Stub { prefix }
    }

    /// The same, with a way of misbehaving.
    pub(crate) fn misbehaving(self, how: Misbehaving) -> Stub {
        match how {
            Misbehaving::Exits(code) => {
                files::put(
                    format!("{}.exit", self.prefix.display()),
                    format!("{code}\n"),
                );
            }
            Misbehaving::Hangs => {
                files::put(format!("{}.hang", self.prefix.display()), "");
            }
        }
        self
    }

    /// The command that runs it: the program, then its arguments.
    pub(crate) fn command(&self) -> Vec<String> {
        vec![
            binary().display().to_string(),
            "--files".to_string(),
            self.prefix.display().to_string(),
        ]
    }

    /// `companion add <name> --command ...` as the arguments `ctxloom` takes.
    pub(crate) fn add_args(&self, name: &str) -> Vec<String> {
        let mut args: Vec<String> = ["companion", "add", name, "--command"]
            .iter()
            .map(ToString::to_string)
            .collect();
        args.extend(self.command());
        args
    }
}

// The stub answers what the protocol asks, through a real exec of the
// compiled program (the crate's own tests cover its answers; these hold that
// the binary a test registers is the one that was built and answers).
#[test]
fn the_compiled_stub_answers_version_and_loadout_through_a_real_exec() {
    let dir = files::root();
    let stub = Stub::install(dir.root(), "acme", "run:\n  version: 1.0.0\n");
    let command = stub.command();
    let version = Command::new(&command[0])
        .args(&command[1..])
        .arg("version")
        .output()
        .expect("the stub runs");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        "{\"name\":\"acme\",\"version\":\"0.0.0-stub\"}\n"
    );
    let loadout = Command::new(&command[0])
        .args(&command[1..])
        .args(["loadout", "--format", "yaml"])
        .output()
        .expect("the stub runs");
    assert_eq!(
        String::from_utf8_lossy(&loadout.stdout),
        "run:\n  version: 1.0.0\n"
    );
    let failing = Stub::install(dir.root(), "bad", "x").misbehaving(Misbehaving::Exits(7));
    let command = failing.command();
    let out = Command::new(&command[0])
        .args(&command[1..])
        .args(["loadout", "--format", "yaml"])
        .output()
        .expect("the stub runs");
    assert_eq!(out.status.code(), Some(7));
}
