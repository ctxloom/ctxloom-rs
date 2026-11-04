//! `manage` over in-memory ports (cli/manage.feature, j000100).

use std::rc::Rc;

use library_application::Flags;
use surface_ports::Kind;

use super::*;
use crate::fakes::{world, CONFIG};

struct Pulls(Result<u64, String>);

impl crate::Puller for Pulls {
    fn pull(&self) -> Result<u64, String> {
        self.0.clone()
    }
}

fn bare() -> crate::fakes::World {
    world(CONFIG)
}

// manage.feature: Scenario: A misspelled engine is refused by name, and
// nothing is scaffolded
#[test]
fn test_a_misspelled_engine_is_refused_and_nothing_is_scaffolded() {
    let w = bare();
    let err = w
        .setup
        .install(&InstallRequest {
            engine: Some("totally-bogus-engine".into()),
            ..InstallRequest::default()
        })
        .unwrap_err();
    assert!(
        matches!(err, SetupError::UnknownEngine { ref known, .. } if known == "claude-code, mock")
    );
    assert!(
        w.mem.scaffold_paths().is_empty()
            || !w.mem.scaffold_paths().iter().any(|p| p == "config.yaml")
    );
}

// Go's checkInstallEngineApplies: --engine over a project that has its config
// is refused (the flag would silently do nothing).
#[test]
fn test_an_engine_asked_for_over_an_existing_project_is_refused() {
    let w = bare();
    w.mem
        .set_scaffold_for_test("config.yaml", "schema_version: 8\n");
    let err = w
        .setup
        .install(&InstallRequest {
            engine: Some("claude-code".into()),
            ..InstallRequest::default()
        })
        .unwrap_err();
    assert!(matches!(err, SetupError::EngineNotApplied { .. }));
}

// manage.feature: Scenario Outline: Installing scaffolds the project and
// writes no engine file
#[test]
fn test_install_scaffolds_the_config_and_the_gitignore_and_no_engine_file() {
    let w = bare();
    let done = w
        .setup
        .install(&InstallRequest {
            engine: Some("claude-code".into()),
            no_pull: true,
            ..InstallRequest::default()
        })
        .unwrap()
        .value;
    assert!(done.initialized);
    assert_eq!(done.gitignore_status, "updated");
    assert_eq!(done.pull.status, "skipped");
    assert!(w.mem.scaffold_text("config.yaml").is_some());
    let ignore = w.mem.scaffold_text(".gitignore").unwrap();
    assert_eq!(ignore.matches("ctxloom's private working state").count(), 1);
    for pattern in ["/cache\n", "/state\n", "*.lock\n"] {
        assert!(ignore.contains(pattern), "{ignore}");
    }
    assert!(w.store.state().claims.is_empty());
}

// manage.feature: Scenario: Repeated gitignore installs do not stack comment
// headers
#[test]
fn test_the_gitignore_is_written_wholesale_and_not_twice() {
    let w = bare();
    let first = w.setup.gitignore_install().unwrap();
    assert!(first.nested_written);
    // The store is where the file is read back from.
    let text = w.mem.scaffold_text(".gitignore").unwrap();
    w.store
        .state()
        .files
        .insert("/p/.ctxloom/.gitignore".to_string(), text.into_bytes());
    let again = w.setup.gitignore_install().unwrap();
    assert!(!again.nested_written);
    assert_eq!(again.status, "unchanged");
}

// manage.feature: install ends with the pull, which warns and stands when it
// fails (a pull that cannot complete is a warning naming what failed).
#[test]
fn test_the_pull_is_reported_and_a_failed_one_does_not_fail_the_install() {
    let w = bare();
    let setup = w.setup.with_puller(Rc::new(Pulls(Ok(2))));
    let pulled = setup.install(&InstallRequest::default()).unwrap().value;
    assert_eq!(
        (pulled.pull.status.as_str(), pulled.pull.installed),
        ("pulled", 2)
    );
    let w = bare();
    let setup = w.setup.with_puller(Rc::new(Pulls(Err("offline".into()))));
    let failed = setup.install(&InstallRequest::default()).unwrap().value;
    assert_eq!(
        (failed.pull.status.as_str(), failed.pull.error.as_str()),
        ("failed", "offline")
    );
    let skipped = setup
        .install(&InstallRequest {
            no_pull: true,
            ..InstallRequest::default()
        })
        .unwrap()
        .value;
    assert_eq!(skipped.pull.status, "skipped");
}

// manage.feature: Scenario: Uninstall strips the harness but keeps the
// project's own content
#[test]
fn test_uninstall_releases_every_kind_of_every_engine_and_leaves_ctxloom() {
    let w = bare();
    w.setup
        .materialize(&crate::MaterializeRequest {
            target: Some("/p".into()),
            engines: vec!["claude-code".into(), "mock".into()],
            ..crate::MaterializeRequest::default()
        })
        .unwrap();
    assert!(!w.store.state().claims.is_empty());
    let report = w.setup.uninstall().unwrap();
    assert_eq!(report.backends, vec!["claude-code", "mock"]);
    assert_eq!(report.status, "released");
    assert!(report.note.contains(".ctxloom directory"));
    assert!(w.store.state().claims.is_empty());
}

// manage.feature: Scenario Outline: Check reports which surfaces are actually
// wired
#[test]
fn test_check_reports_every_engine_and_what_ctxloom_has_materialized() {
    let w = bare();
    w.setup
        .materialize(&crate::MaterializeRequest {
            target: Some("/p".into()),
            engines: vec!["claude-code".into()],
            surfaces: vec![crate::SurfaceSpec::parse("context").unwrap()],
            ..crate::MaterializeRequest::default()
        })
        .unwrap();
    let report = w.setup.check(&Flags::default()).unwrap().value;
    assert_eq!(report.work_dir, "/p");
    assert!(report.manage_statusline);
    assert_eq!(report.backends.len(), 2);
    assert_eq!(report.backends[0].backend, "claude-code");
    assert_eq!(report.surfaces.len(), 1);
    assert_eq!(report.surfaces[0].route, Kind::Context.name());
}

// manage.feature: Scenario: An unknown event name is refused rather than
// answered empty
#[test]
fn test_an_unknown_event_is_refused_and_a_known_one_may_be_empty() {
    let w = bare();
    let err = w
        .setup
        .hooks_list(Some("pre_toll"), &[], &Flags::default())
        .unwrap_err();
    assert!(matches!(err, SetupError::UnknownEvent { ref event, .. } if event == "pre_toll"));
    let one = w
        .setup
        .hooks_list(Some("turn_end"), &[], &Flags::default())
        .unwrap()
        .value;
    assert_eq!(one.events.len(), 1);
    assert!(one.events[0].hooks.is_empty());
}

// manage.feature: Scenario: The hook list has a machine-readable form (every
// lifecycle event is listed, ctxloom's own callbacks among them)
#[test]
fn test_every_lifecycle_event_is_listed_with_ctxloom_own_callbacks() {
    let w = bare();
    let list = w
        .setup
        .hooks_list(None, &[], &Flags::default())
        .unwrap()
        .value;
    let names: Vec<&str> = list.events.iter().map(|e| e.event.as_str()).collect();
    assert!(names.contains(&"post_file_edit") && names.contains(&"session_start"));
    let start = list
        .events
        .iter()
        .find(|e| e.event == "session_start")
        .unwrap();
    assert_eq!(start.hooks[0].source_kind, "context");
    assert_eq!(start.hooks[0].args, vec!["hook", "session-start"]);
}

// manage.feature: Scenario: Statusline can be disabled and re-enabled
#[test]
fn test_the_statusline_choice_is_written_to_the_projects_config() {
    let w = bare();
    assert!(
        !w.setup
            .set_statusline(false, &Flags::default())
            .unwrap()
            .value
    );
    let config = w.mem.config(ctxloom_ports::Scope::Project).unwrap();
    assert!(config.contains("statusline: false"), "{config}");
}

// manage.go's `--print`: the steps install would take, without taking them.
#[test]
fn test_the_install_plan_names_the_engine_only_when_it_would_scaffold() {
    let w = bare();
    let plan = w.setup.install_plan(Some("mock")).unwrap();
    assert!(!plan.ctxloom_dir_exists);
    assert_eq!(plan.engine, "mock");
    assert_eq!(plan.gitignore, "/p/.gitignore");
    assert!(w.mem.scaffold_text("config.yaml").is_none());
    w.mem
        .set_scaffold_for_test("config.yaml", "schema_version: 8\n");
    let plan = w.setup.install_plan(Some("mock")).unwrap();
    assert!(plan.ctxloom_dir_exists && plan.engine.is_empty());
}
