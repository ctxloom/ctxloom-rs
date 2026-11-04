//! The doctor over fakes of the ports. Go: `cli/doctor.feature` (release/0.7).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use ctxloom_ports::{ConfigFiles, Executables, Failure, LayerText, Put, Scope, Source, UserFiles};

use crate::{Doctor, Status};

#[derive(Default)]
struct Mem {
    files: Mutex<BTreeMap<String, String>>,
    project: bool,
}

impl ConfigFiles for Mem {
    fn source(&self) -> Source {
        if self.project {
            Source::Project
        } else {
            Source::Home
        }
    }
    fn app_dir(&self) -> String {
        "/work/.ctxloom".to_string()
    }
    fn layers(&self) -> Result<Vec<LayerText>, Failure> {
        Ok(Vec::new())
    }
    fn layer(&self, _: Scope) -> Result<Option<LayerText>, Failure> {
        Ok(self
            .files
            .lock()
            .unwrap()
            .get("/work/.ctxloom/config.yaml")
            .map(|text| LayerText {
                path: "/work/.ctxloom/config.yaml".to_string(),
                text: text.clone(),
            }))
    }
    fn layer_path(&self, _: Scope) -> String {
        "/work/.ctxloom/config.yaml".to_string()
    }
    fn write_layer(&self, _: Scope, _: &str) -> Result<(), Failure> {
        Ok(())
    }
}

impl UserFiles for Mem {
    fn exists(&self, path: &str) -> Result<bool, Failure> {
        Ok(self.files.lock().unwrap().contains_key(path))
    }
    fn is_dir(&self, _: &str) -> Result<bool, Failure> {
        Ok(false)
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Failure> {
        Ok(self.files.lock().unwrap()[path].clone().into_bytes())
    }
    fn read_tree(&self, _: &str) -> Result<Vec<Put>, Failure> {
        Ok(Vec::new())
    }
    fn write_file(&self, path: &str, bytes: &[u8], _: bool) -> Result<bool, Failure> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), String::from_utf8(bytes.to_vec()).unwrap());
        Ok(true)
    }
    fn write_tree(&self, _: &str, _: &[Put]) -> Result<(), Failure> {
        Ok(())
    }
}

struct Git(bool);

impl Executables for Git {
    fn find(&self, name: &str) -> Option<String> {
        (self.0 && name == "git").then(|| "/usr/bin/git".to_string())
    }
}

fn doctor(mem: &Arc<Mem>, git: bool) -> Doctor {
    Doctor::new(
        mem.clone(),
        mem.clone(),
        Arc::new(Git(git)),
        "/home/u",
        "0.1.0",
    )
}

fn project() -> Arc<Mem> {
    let mem = Arc::new(Mem {
        project: true,
        ..Mem::default()
    });
    mem.files.lock().unwrap().insert(
        "/work/.ctxloom/config.yaml".to_string(),
        "schema_version: 8\n".to_string(),
    );
    mem
}

fn status(report: &crate::Report, marker: &str) -> Status {
    report
        .checks
        .iter()
        .find(|c| c.marker == marker)
        .unwrap_or_else(|| panic!("no {marker}"))
        .status
}

// doctor.feature: "Doctor reports every check with the shared marker vocabulary".
#[test]
fn the_report_carries_every_marker_and_none_fails_on_a_plain_project() {
    let report = doctor(&project(), true).report(false);
    for marker in [
        "DOCTOR-CHECK-DEPS-a1",
        "DOCTOR-CHECK-AGENTS-b2",
        "DOCTOR-CHECK-VERSION-c3",
        "DOCTOR-CHECK-HOOKS-TRUST-d4",
        "DOCTOR-CHECK-SETUP-MARKER-e5",
        "DOCTOR-CHECK-GITIGNORE-f6",
        "DOCTOR-CHECK-MCP-INVOCATION-g7",
        "DOCTOR-CHECK-STALE-HOOKS-n5",
        "DOCTOR-CHECK-INGESTION-q7",
    ] {
        assert!(report.checks.iter().any(|c| c.marker == marker), "{marker}");
    }
    assert!(report.checks.iter().all(|c| !c.detail.is_empty()));
    assert!(!report.failed());
}

// doctor.feature: "The scoped report keeps the capability probes and drops
// the setup checks".
#[test]
fn deps_narrows_to_the_capability_probes() {
    let report = doctor(&project(), true).report(true);
    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].marker, "DOCTOR-CHECK-DEPS-a1");
}

// doctor.feature: "Doctor names a stale ctxloom hook entry and --fix removes
// exactly it" (the project's settings and the user's).
#[test]
fn fix_removes_the_stale_entries_of_the_project_and_the_user_and_then_the_row_is_ok() {
    let mem = project();
    let stale = r#"{"hooks":{"SessionStart":[{"hooks":[{"command":"ctxloom hook inject-context abc"},{"command":"ctxloom hook session-start"}]}]}}"#;
    mem.files
        .lock()
        .unwrap()
        .insert("/work/.claude/settings.json".to_string(), stale.to_string());
    mem.files.lock().unwrap().insert(
        "/home/u/.claude/settings.json".to_string(),
        stale.to_string(),
    );
    let doctor = doctor(&mem, true);
    assert_eq!(
        status(&doctor.report(false), "DOCTOR-CHECK-STALE-HOOKS-n5"),
        Status::Warn
    );
    let removed = doctor.fix().unwrap();
    assert_eq!(removed.len(), 2);
    assert_eq!(
        status(&doctor.report(false), "DOCTOR-CHECK-STALE-HOOKS-n5"),
        Status::Ok
    );
    let kept = mem.files.lock().unwrap()["/work/.claude/settings.json"].clone();
    assert!(kept.contains("ctxloom hook session-start") && !kept.contains("inject-context"));
}

#[test]
fn without_a_project_the_marker_warns_and_project_files_are_not_read() {
    let mem = Arc::new(Mem::default());
    let report = doctor(&mem, false).report(false);
    assert_eq!(
        status(&report, "DOCTOR-CHECK-SETUP-MARKER-e5"),
        Status::Warn
    );
    assert_eq!(status(&report, "DOCTOR-CHECK-DEPS-a1"), Status::Warn);
}

const LAUNCHES: &str = r#"{"mcpServers":{"ctxloom":{"command":"ctxloom","args":["mcp","serve"]}}}"#;

fn mcp_detail(mem: &Arc<Mem>) -> (Status, String) {
    let report = doctor(mem, true).report(false);
    let row = report
        .checks
        .iter()
        .find(|c| c.marker == "DOCTOR-CHECK-MCP-INVOCATION-g7")
        .unwrap();
    (row.status, row.detail.clone())
}

// The files that register MCP servers: the project's `.mcp.json` and the
// user's `~/.claude.json`, and only those.
#[test]
fn the_project_mcp_file_is_read() {
    let mem = project();
    mem.files
        .lock()
        .unwrap()
        .insert("/work/.mcp.json".to_string(), LAUNCHES.to_string());
    let (status, detail) = mcp_detail(&mem);
    assert_eq!(status, Status::Warn);
    assert!(detail.contains("from .mcp.json "), "{detail}");
}

#[test]
fn the_user_mcp_file_is_read() {
    let mem = project();
    mem.files
        .lock()
        .unwrap()
        .insert("/home/u/.claude.json".to_string(), LAUNCHES.to_string());
    let (status, detail) = mcp_detail(&mem);
    assert_eq!(status, Status::Warn);
    assert!(detail.contains("from ~/.claude.json "), "{detail}");
}

#[test]
fn without_a_project_only_the_user_mcp_file_is_read() {
    let mem = Arc::new(Mem::default());
    mem.files
        .lock()
        .unwrap()
        .insert("/work/.mcp.json".to_string(), LAUNCHES.to_string());
    assert_eq!(mcp_detail(&mem).0, Status::Ok);
}

#[test]
fn a_report_fails_only_when_a_row_fails() {
    let row = |status| crate::Check {
        marker: "M",
        name: "n",
        status,
        detail: String::new(),
        remedy: String::new(),
    };
    let report = |statuses: &[Status]| crate::Report {
        checks: statuses.iter().map(|s| row(*s)).collect(),
    };
    assert!(report(&[Status::Ok, Status::Fail]).failed());
    assert!(!report(&[Status::Ok, Status::Warn, Status::Info]).failed());
}
