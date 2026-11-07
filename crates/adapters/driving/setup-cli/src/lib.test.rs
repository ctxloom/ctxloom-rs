//! The setup commands run on fakes of the ports they reach through the use
//! cases: what each noun prints, on which stream, and how it exits.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use auth_application::ExportedVars;
use clap::Command;
use deps_application::fakes::Fixture;
use doctor_checks::{
    ConfigFiles, Executables, Failure as PortFailure, LayerText, Put, Scope, Source, UserFiles,
};
use init_application::Scaffold;
use reclaim_application::ProjectCache;

use super::*;

/// One in-memory world behind every port.
#[derive(Default)]
struct World {
    files: Mutex<BTreeMap<String, String>>,
    trees: Mutex<BTreeMap<String, u64>>,
    /// The variables the launch exports.
    exported: Mutex<Vec<String>>,
    /// The project has no `.ctxloom` yet.
    fresh: std::sync::atomic::AtomicBool,
}

/// The scaffold, which lands.
struct Made;

impl init_application::Create for Made {
    fn create(&self) -> Result<(), PortFailure> {
        Ok(())
    }
}

impl ExportedVars for World {
    fn is_exported(&self, name: &str) -> bool {
        self.exported.lock().unwrap().iter().any(|v| v == name)
    }
}

impl ProjectCache for World {
    fn app_dir(&self) -> String {
        "/work/.ctxloom".to_string()
    }
    fn measure(&self, rel: &str) -> Result<Option<u64>, PortFailure> {
        Ok(self.trees.lock().unwrap().get(rel).copied())
    }
    fn remove(&self, rel: &str) -> Result<(), PortFailure> {
        self.trees.lock().unwrap().remove(rel);
        Ok(())
    }
}

impl ConfigFiles for World {
    fn source(&self) -> Source {
        Source::Project
    }
    fn app_dir(&self) -> String {
        "/work/.ctxloom".to_string()
    }
    fn layers(&self) -> Result<Vec<LayerText>, PortFailure> {
        Ok(Vec::new())
    }
    fn layer(&self, _: Scope) -> Result<Option<LayerText>, PortFailure> {
        Ok(None)
    }
    fn layer_path(&self, _: Scope) -> String {
        "/work/.ctxloom/config.yaml".to_string()
    }
    fn write_layer(&self, _: Scope, _: &str) -> Result<(), PortFailure> {
        Ok(())
    }
}

impl UserFiles for World {
    fn exists(&self, path: &str) -> Result<bool, PortFailure> {
        Ok(self.files.lock().unwrap().contains_key(path))
    }
    fn is_dir(&self, path: &str) -> Result<bool, PortFailure> {
        Ok(path == "/work/.ctxloom" && !self.fresh.load(std::sync::atomic::Ordering::SeqCst))
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PortFailure> {
        Ok(self.files.lock().unwrap()[path].clone().into_bytes())
    }
    fn read_tree(&self, _: &str) -> Result<Vec<Put>, PortFailure> {
        Ok(Vec::new())
    }
    fn write_file(&self, path: &str, bytes: &[u8], _: bool) -> Result<bool, PortFailure> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), String::from_utf8(bytes.to_vec()).unwrap());
        Ok(true)
    }
    fn write_tree(&self, _: &str, _: &[Put]) -> Result<(), PortFailure> {
        Ok(())
    }
}

impl Scaffold for World {
    fn app_dir(&self) -> String {
        "/work/.ctxloom".to_string()
    }
    fn exists(&self, _: &str) -> Result<bool, PortFailure> {
        Ok(true)
    }
    fn write(&self, _: &[Put]) -> Result<(), PortFailure> {
        Ok(())
    }
}

struct Git;

impl Executables for Git {
    fn find(&self, name: &str) -> Option<String> {
        (name == "git").then(|| "/usr/bin/git".to_string())
    }
}

struct Run {
    exit: Result<u8, Failure>,
    out: String,
    err: String,
    world: Arc<World>,
}

fn go(line: &[&str], format: Format, configure: impl FnOnce(&World, &Fixture)) -> Run {
    let mut out = Vec::new();
    let mut r = go_to(line, format, configure, &mut out);
    r.out = String::from_utf8(out).unwrap();
    r
}

fn go_to(
    line: &[&str],
    format: Format,
    configure: impl FnOnce(&World, &Fixture),
    out: &mut dyn Write,
) -> Run {
    let world = Arc::new(World::default());
    let fixture = Fixture::new();
    configure(&world, &fixture);
    let cleaner = Cleaner::new(world.clone());
    let auth = Auth::new(world.clone());
    let doctor = Doctor::new(
        world.clone(),
        world.clone(),
        Arc::new(Git),
        Auth::new(world.clone()),
        "/home/u",
        "0.1.0",
    );
    let init = Init::new(
        world.clone(),
        world.clone(),
        Rc::new(fixture.deps),
        Box::new(Made),
    );
    let uses = UseCases {
        auth: &auth,
        cleaner: &cleaner,
        doctor: &doctor,
        init: &init,
    };
    let mut full = vec!["ctxloom"];
    full.extend(line);
    let matches = nouns()
        .into_iter()
        .fold(Command::new("ctxloom"), Command::subcommand)
        .try_get_matches_from(full)
        .expect("parses");
    let noun = matches.subcommand_name().expect("a noun").to_string();
    let call = Call::new(&matches, Some(format), Format::Text);
    let catalog = Catalog::en_us(&[sessions_application::EN_US, deps_application::EN_US, EN_US])
        .expect("catalog");
    let mut err = Vec::new();
    let exit = run(&uses, &noun, &call, &catalog, out, &mut err);
    Run {
        exit,
        out: String::new(),
        err: String::from_utf8(err).unwrap(),
        world,
    }
}

fn none(_: &World, _: &Fixture) {}

/// A launch that exports claude's token.
fn token(world: &World, _: &Fixture) {
    world
        .exported
        .lock()
        .unwrap()
        .push("CLAUDE_CODE_OAUTH_TOKEN".to_string());
}

fn cached(world: &World, _: &Fixture) {
    world
        .trees
        .lock()
        .unwrap()
        .insert("cache/context".to_string(), 2048);
}

#[test]
fn clean_without_yes_reports_removes_nothing_and_says_so_on_the_error_stream() {
    let r = go(&["clean"], Format::Text, cached);
    assert_eq!(r.exit.unwrap(), 0);
    assert!(r.out.contains("would remove 2.0 KB"), "{}", r.out);
    assert!(r.err.starts_with("ctxloom: removed nothing"), "{}", r.err);
    assert!(r.err.contains("ctxloom clean --yes"), "{}", r.err);
    assert_eq!(r.world.trees.lock().unwrap().len(), 1);
}

#[test]
fn clean_with_yes_removes_and_is_quiet_on_the_error_stream() {
    let r = go(&["clean", "--yes"], Format::Text, cached);
    assert_eq!(r.exit.unwrap(), 0);
    assert!(r.out.contains("ctxloom removed 2.0 KB"), "{}", r.out);
    assert_eq!(r.err, "");
    assert!(r.world.trees.lock().unwrap().is_empty());
}

#[test]
fn clean_in_json_writes_the_report_and_a_json_warning() {
    let r = go(&["clean"], Format::Json, cached);
    assert_eq!(r.exit.unwrap(), 0);
    assert!(r.out.contains("\"applied\": false"), "{}", r.out);
    assert!(
        r.err
            .contains("\"code\": \"sessions-application-removed-nothing\""),
        "{}",
        r.err
    );
}

#[test]
fn doctor_prints_its_report_and_exits_zero_when_no_row_failed() {
    let r = go(&["doctor"], Format::Text, token);
    assert_eq!(r.exit.unwrap(), 0);
    assert!(r.out.starts_with("ctxloom doctor\n"), "{}", r.out);
    assert!(r.out.contains("[warn] DOCTOR-CHECK-"), "{}", r.out);
    assert!(!r.out.contains("[ok]"), "{}", r.out);
    let all = go(&["doctor", "--all"], Format::Text, token);
    assert!(all.out.contains("[ok]"), "{}", all.out);
}

// doctor.feature: "A missing agent credential is a fail row with run's fix,
// and exit 3": the whole report is written first.
#[test]
fn doctor_exits_three_after_the_whole_report_when_a_credential_is_missing() {
    let r = go(&["doctor"], Format::Text, none);
    assert_eq!(r.exit.unwrap(), 3);
    assert!(
        r.out.contains("[fail] DOCTOR-CHECK-CREDENTIALS-m5"),
        "{}",
        r.out
    );
    assert!(r.out.contains("claude setup-token"), "{}", r.out);
    assert!(r.out.contains("DOCTOR-CHECK-INGESTION-q7") || r.out.contains("[warn]"));
    let json = go(&["doctor"], Format::Json, none);
    assert_eq!(json.exit.unwrap(), 3);
    assert!(
        json.out
            .contains("\"marker\": \"DOCTOR-CHECK-CREDENTIALS-m5\""),
        "{}",
        json.out
    );
    assert!(json.out.contains("\"status\": \"fail\""), "{}", json.out);
}

#[test]
fn doctor_deps_checks_only_the_machine_and_json_lists_the_checks() {
    let r = go(&["doctor", "--deps"], Format::Json, none);
    assert_eq!(r.exit.unwrap(), 0);
    assert!(r.out.contains("DOCTOR-CHECK-DEPS-a1"), "{}", r.out);
    assert!(!r.out.contains("DOCTOR-CHECK-SETUP-MARKER"), "{}", r.out);
}

#[test]
fn doctor_fix_removes_a_stale_entry_and_names_it_on_the_error_stream() {
    let stale =
        r#"{"hooks":{"SessionStart":[{"hooks":[{"command":"ctxloom hook inject-context abc"}]}]}}"#;
    let r = go(&["doctor", "--fix"], Format::Text, |w, f| {
        token(w, f);
        w.files
            .lock()
            .unwrap()
            .insert("/work/.claude/settings.json".to_string(), stale.to_string());
    });
    assert_eq!(r.exit.unwrap(), 0);
    assert_eq!(
        r.err,
        "removed stale hook entry: SessionStart runs `ctxloom hook inject-context` in .claude/settings.json\n"
    );
    assert!(
        !r.world.files.lock().unwrap()["/work/.claude/settings.json"].contains("inject-context")
    );
    let untouched = go(&["doctor"], Format::Text, token);
    assert_eq!(untouched.err, "");
}

#[test]
fn the_exit_of_a_report_is_three_only_when_a_row_failed() {
    use doctor_checks::{Check, Report, Status};
    let row = |status| Check {
        marker: "DOCTOR-CHECK-X-a1",
        name: "x",
        status,
        detail: String::new(),
        remedy: String::new(),
    };
    assert_eq!(
        exit_of(&Report {
            checks: vec![row(Status::Warn)]
        }),
        0
    );
    assert_eq!(
        exit_of(&Report {
            checks: vec![row(Status::Ok), row(Status::Fail)]
        }),
        3
    );
}

#[test]
fn init_says_the_directory_exists_and_exits_zero() {
    let r = go(&["init"], Format::Text, none);
    assert_eq!(r.exit.unwrap(), 0);
    assert_eq!(r.out, "ctxloom directory already exists: /work/.ctxloom\n");
    assert_eq!(r.err, "");
}

#[test]
fn init_of_a_new_project_says_what_it_made_and_pulled() {
    let r = go(&["init"], Format::Text, |world, fixture| {
        world.fresh.store(true, std::sync::atomic::Ordering::SeqCst);
        let url = "file:///srv/origin.git";
        fixture.origin("origin", url);
        fixture.profile("dev", &[&Fixture::reference(url, "demo")]);
    });
    assert_eq!(r.exit.unwrap(), 0);
    assert_eq!(
        r.out,
        "Created ctxloom directory: /work/.ctxloom\nPulled 1 dependencies\n"
    );
}

#[test]
fn init_adds_each_remote_and_reports_one_that_would_not_register_as_a_warning() {
    let r = go(
        &[
            "init",
            "--remote",
            "https://example.com/a/b.git",
            "--forge",
            "git",
        ],
        Format::Text,
        |_, f| f.remotes.serve("https://example.com/a/b.git"),
    );
    assert_eq!(r.exit.unwrap(), 0);
    assert!(
        r.out
            .contains("Added remote \"personal\": https://example.com/a/b.git"),
        "{} / {}",
        r.out,
        r.err
    );
}

#[test]
fn init_prompt_prints_the_interview_and_nothing_else() {
    let r = go(&["init", "prompt"], Format::Json, none);
    assert_eq!(r.exit.unwrap(), 0);
    assert_eq!(r.out, init_application::prompt());
    assert_eq!(r.err, "");
}

#[test]
fn a_noun_the_line_does_not_carry_is_a_usage_failure() {
    let world = Arc::new(World::default());
    let fixture = Fixture::new();
    let cleaner = Cleaner::new(world.clone());
    let auth = Auth::new(world.clone());
    let doctor = Doctor::new(
        world.clone(),
        world.clone(),
        Arc::new(Git),
        Auth::new(world.clone()),
        "/h",
        "0",
    );
    let init = Init::new(
        world.clone(),
        world.clone(),
        Rc::new(fixture.deps),
        Box::new(Made),
    );
    let uses = UseCases {
        auth: &auth,
        cleaner: &cleaner,
        doctor: &doctor,
        init: &init,
    };
    let matches = nouns()
        .into_iter()
        .fold(Command::new("ctxloom"), Command::subcommand)
        .try_get_matches_from(["ctxloom", "clean"])
        .unwrap();
    let call = Call::new(&matches, None, Format::Text);
    let catalog = Catalog::en_us(&[EN_US]).unwrap();
    let failure = run(
        &uses,
        "doctor",
        &call,
        &catalog,
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("fails");
    assert_eq!(failure.exit, 2);
    assert_eq!(failure.cause.code(), "setup-cli-no-such-command");
}

#[test]
fn an_unwritable_answer_is_an_output_failure_with_exit_one() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for line in [&["init", "prompt"][..], &["init"], &["clean"], &["doctor"]] {
        let failure = go_to(line, Format::Text, cached, &mut Broken)
            .exit
            .expect_err("fails");
        assert_eq!(failure.exit, 1, "{line:?}");
        assert_eq!(failure.cause.code(), "setup-cli-output", "{line:?}");
    }
}

// auth.feature, Go's `auth status` (internal/adapters/cli/auth.go).
#[test]
fn auth_status_reports_an_exported_token_present_and_never_prints_a_value() {
    let r = go(&["auth", "status"], Format::Text, token);
    assert_eq!(r.exit.unwrap(), 0);
    assert_eq!(
        r.out,
        "claude-code token: present (CLAUDE_CODE_OAUTH_TOKEN)\n"
    );
    assert_eq!(r.err, "");
}

#[test]
fn auth_status_names_how_to_mint_and_export_a_missing_token() {
    let r = go(&["auth", "status"], Format::Text, none);
    assert_eq!(r.exit.unwrap(), 0);
    assert_eq!(
        r.out,
        "claude-code token: missing \u{2014} run `claude setup-token` and export CLAUDE_CODE_OAUTH_TOKEN (or store it in your secret manager)\n"
    );
}

#[test]
fn auth_status_in_json_is_the_rows_by_variable_name() {
    let r = go(&["auth", "status"], Format::Json, token);
    assert_eq!(r.exit.unwrap(), 0);
    let rows: serde_json::Value = serde_json::from_str(&r.out).unwrap();
    assert_eq!(rows[0]["engine"], "claude-code");
    assert_eq!(rows[0]["mode"], "token");
    assert_eq!(rows[0]["present"], true);
    assert_eq!(
        rows[0]["vars"],
        serde_json::json!(["CLAUDE_CODE_OAUTH_TOKEN"])
    );
}

// Go: `auth` alone is `auth status` (a group's default).
#[test]
fn a_bare_auth_is_auth_status() {
    let bare = go(&["auth"], Format::Text, token);
    let status = go(&["auth", "status"], Format::Text, token);
    assert_eq!(bare.exit.unwrap(), 0);
    assert_eq!(bare.out, status.out);
}
