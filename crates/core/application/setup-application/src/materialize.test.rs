//! `materialize` over in-memory ports. Each test names the Go scenario it
//! holds the use case to (cli/materialize.feature, cli/manage.feature).

use library_application::Flags;
use surface_ports::{Inspected, Kind};

use super::*;
use crate::fakes::{world, CONFIG};

fn request(engines: &[&str]) -> MaterializeRequest {
    MaterializeRequest {
        target: Some("out".to_string()),
        engines: engines.iter().map(ToString::to_string).collect(),
        ..MaterializeRequest::default()
    }
}

fn outcome(result: &MaterializeResult, engine: &str) -> EngineOutcome {
    result
        .engines
        .iter()
        .find(|e| e.engine == engine)
        .cloned()
        .unwrap_or_else(|| panic!("no {engine} in {result:?}"))
}

// materialize.feature: Scenario: Re-materializing only the context keeps the
// commands and skills
#[test]
fn test_a_subset_delivery_touches_only_the_kinds_it_names() {
    let w = world(CONFIG);
    let all = w
        .setup
        .materialize(&request(&["claude-code"]))
        .unwrap()
        .value;
    assert_eq!(all.status, "applied");
    let wrote = outcome(&all, "claude-code").wrote;
    assert_eq!(
        wrote,
        vec!["context", "mcp", "settings", "hooks", "commands", "skills"]
    );
    let mut again = request(&["claude-code"]);
    again.surfaces = vec![SurfaceSpec::parse("context").unwrap()];
    w.setup.materialize(&again).unwrap();
    let held = w
        .store
        .state()
        .claims
        .keys()
        .filter(|(e, _)| e == "claude-code")
        .count();
    assert!(held >= 4, "the other kinds stand: {held}");
    let mut release = request(&["claude-code"]);
    release.release = true;
    release.surfaces = vec![SurfaceSpec::parse("commands").unwrap()];
    let released = w.setup.materialize(&release).unwrap().value;
    assert_eq!(released.status, "released");
    assert!(!w
        .store
        .state()
        .claims
        .contains_key(&("claude-code".to_string(), Kind::Commands)));
    assert!(w
        .store
        .state()
        .claims
        .contains_key(&("claude-code".to_string(), Kind::Context)));
}

// materialize.feature: Scenario: Releasing one engine leaves the other's files
#[test]
fn test_releasing_one_engine_leaves_the_others_claims() {
    let w = world(CONFIG);
    w.setup
        .materialize(&request(&["claude-code", "mock"]))
        .unwrap();
    let mut release = request(&["mock"]);
    release.release = true;
    w.setup.materialize(&release).unwrap();
    let state = w.store.state();
    assert!(state.claims.keys().any(|(e, _)| e == "claude-code"));
    assert!(!state.claims.keys().any(|(e, _)| e == "mock"));
}

// manage.feature: "No engine gets ctxloom registered as an MCP server at
// rest": the server the session serves is not registered, and the tool
// bundle's is.
#[test]
fn test_ctxloom_own_server_is_never_registered_at_rest() {
    let w = world(CONFIG);
    w.setup.materialize(&request(&["claude-code"])).unwrap();
    let state = w.store.state();
    let Some(claims) = state.claims.get(&("claude-code".to_string(), Kind::Mcp)) else {
        panic!("the tool server is registered")
    };
    assert_eq!(claims.len(), 1);
}

// materialize.feature: Scenario: With no target, materialize warns and refuses
// unless --yes is given
#[test]
fn test_the_project_directory_is_written_only_with_yes() {
    let w = world(CONFIG);
    let mut bare = MaterializeRequest {
        engines: vec!["claude-code".to_string()],
        ..MaterializeRequest::default()
    };
    let err = w.setup.materialize(&bare).unwrap_err();
    assert!(matches!(err, SetupError::NoTarget { ref dir, .. } if dir == "/p"));
    assert_eq!(err.exit_code(), 2);
    assert!(w.store.state().log.is_empty());
    bare.yes = true;
    let done = w.setup.materialize(&bare).unwrap().value;
    assert_eq!(done.target, "/p");
}

// materialize.feature: engines default to the configured ones (the labels'
// types), else the registry's first.
#[test]
fn test_engines_default_to_the_configured_ones() {
    let w = world(CONFIG);
    let done = w.setup.materialize(&request(&[])).unwrap().value;
    assert_eq!(done.engines.len(), 1);
    assert_eq!(
        done.engines[0].engine, "mock",
        "the label of type mock is not registered here"
    );
}

#[test]
fn test_an_unregistered_engine_is_refused_by_name_with_the_known_ones() {
    let w = world(CONFIG);
    let err = w.setup.materialize(&request(&["nosuch"])).unwrap_err();
    let SetupError::UnknownEngine { engine, known } = err else {
        panic!()
    };
    assert_eq!(
        (engine.as_str(), known.as_str()),
        ("nosuch", "claude-code, mock")
    );
}

// j000400_multi_engine.feature: a surface the engine does not carry is named
// with a reason, and not written.
#[test]
fn test_what_an_engine_does_not_carry_is_named_not_dropped() {
    let w = world(CONFIG);
    let done = w.setup.materialize(&request(&["mock"])).unwrap().value;
    let mock_engine = outcome(&done, "mock");
    let named: Vec<&str> = mock_engine
        .not_carried
        .iter()
        .map(|n| n.surface.as_str())
        .collect();
    assert!(named.contains(&"mcp") && named.contains(&"hooks") && named.contains(&"commands"));
    assert!(mock_engine
        .not_carried
        .iter()
        .all(|n| n.reason.contains("mock declares no approach")));
    assert_eq!(mock_engine.wrote, vec!["context"]);
}

// manage.feature: "A statusline ctxloom cannot read refuses the write": the
// refused kind fails alone, and so does any later kind in the same file.
#[test]
fn test_a_refused_kind_fails_the_kinds_that_share_its_file() {
    let w = world(CONFIG);
    w.store.state().refuse_path = Some("claude-code/settings".to_string());
    let done = w
        .setup
        .materialize(&request(&["claude-code"]))
        .unwrap()
        .value;
    assert_eq!(done.failures.len(), 1);
    assert_eq!(done.status, "failed");
    let alpha = outcome(&done, "claude-code");
    assert!(alpha.wrote.contains(&"context".to_string()));
    assert!(!alpha.wrote.contains(&"settings".to_string()));
}

// materialize.feature: a dry run plans and reports and writes nothing.
#[test]
fn test_a_dry_run_writes_nothing_and_reports_the_kinds_that_would_change() {
    let w = world(CONFIG);
    let mut dry = request(&["claude-code"]);
    dry.dry_run = true;
    let done = w.setup.materialize(&dry).unwrap().value;
    assert_eq!(done.status, "planned");
    assert!(!done.created);
    assert!(w.store.state().claims.is_empty());
    assert!(outcome(&done, "claude-code")
        .wrote
        .contains(&"context".to_string()));
}

// materialize.feature: a target that does not exist is created by a delivery
// and holds nothing to release.
#[test]
fn test_a_missing_target_is_created_and_has_nothing_to_release() {
    let w = world(CONFIG);
    w.store
        .state()
        .targets
        .insert("/p/out".to_string(), Inspected::Missing);
    let mut release = request(&["claude-code"]);
    release.release = true;
    let done = w.setup.materialize(&release).unwrap().value;
    assert_eq!(done.status, "nothing");
    assert!(done.warnings[0].contains("nothing to release"));
    let applied = w
        .setup
        .materialize(&request(&["claude-code"]))
        .unwrap()
        .value;
    assert!(applied.created);
}

#[test]
fn test_a_target_that_is_a_file_is_refused() {
    let w = world(CONFIG);
    w.store
        .state()
        .targets
        .insert("/p/out".to_string(), Inspected::Other);
    assert!(matches!(
        w.setup.materialize(&request(&["claude-code"])),
        Err(SetupError::TargetNotDirectory { .. })
    ));
}

// materialize.feature (the request's own rules): a release takes no profiles
// or destination; a destination is context's alone; one kind is not named twice
// differently.
#[test]
fn test_an_incoherent_request_is_refused_before_anything_is_written() {
    let w = world(CONFIG);
    let bad = |surfaces: &[&str], release: bool, profiles: &[&str]| {
        let mut r = request(&["claude-code"]);
        r.release = release;
        r.profiles = profiles.iter().map(ToString::to_string).collect();
        r.surfaces = surfaces
            .iter()
            .map(|s| SurfaceSpec::parse(s).unwrap())
            .collect();
        let err = w.setup.materialize(&r).unwrap_err();
        assert!(matches!(err, SetupError::Request { .. }), "{err:?}");
    };
    bad(&[], true, &["dev"]);
    bad(&["commands:x.md"], false, &[]);
    bad(&["context=file:x.md"], true, &[]);
    bad(&["context=link"], false, &[]);
    bad(&["context", "context:x.md"], false, &[]);
    assert!(w.store.state().log.is_empty());
}

#[test]
fn test_a_surface_spec_is_kind_mechanism_and_destination() {
    let spec = SurfaceSpec::parse("context=file:docs/AGENTS.md").unwrap();
    assert_eq!(
        (spec.kind, spec.mechanism.as_str(), spec.dest.as_str()),
        (Kind::Context, "file", "docs/AGENTS.md")
    );
    assert!(matches!(
        SurfaceSpec::parse("nothing"),
        Err(SetupError::Request { .. })
    ));
}

// materialize.feature: "Context written into the user's own file": the
// destination is what the engine is asked to write the context to.
#[test]
fn test_the_context_goes_to_the_destination_asked_for() {
    let w = world(CONFIG);
    let mut r = request(&["claude-code"]);
    r.surfaces = vec![SurfaceSpec::parse("context=file:docs/AGENTS.md").unwrap()];
    let done = w.setup.materialize(&r).unwrap().value;
    assert_eq!(
        outcome(&done, "claude-code").context_file,
        "/p/out/docs/AGENTS.md"
    );
}

// manage.feature (the empty-context rule of Go's materializePackage): nothing
// assembled, nothing to write - but ctxloom's own fragment is always there.
#[test]
fn test_ctxloom_own_fragment_keeps_an_empty_project_from_assembling_to_nothing() {
    let w = world("schema_version: 8\n");
    let mut r = request(&["claude-code"]);
    r.surfaces = vec![SurfaceSpec::parse("context").unwrap()];
    let done = w.setup.materialize(&r).unwrap().value;
    assert_eq!(outcome(&done, "claude-code").wrote, vec!["context"]);
    let state = w.store.state();
    let Some(claims) = state
        .claims
        .get(&("claude-code".to_string(), Kind::Context))
    else {
        panic!()
    };
    let surface_ports::Claim::Section { text, .. } = &claims[0] else {
        panic!()
    };
    assert!(text.contains("Isolation: specify both axes"));
}

// manage.feature: a strict materialize refuses over a fatal finding before
// writing anything; --degraded waives it.
#[test]
fn test_a_fatal_finding_refuses_the_delivery_unless_degraded() {
    let w = world(CONFIG);
    let mut r = request(&["claude-code"]);
    r.profiles = vec!["broken".to_string()];
    let err = w.setup.materialize(&r).unwrap_err();
    assert!(matches!(err, SetupError::Refused { .. }), "{err:?}");
    assert_eq!(err.exit_code(), 3);
    assert!(w.store.state().log.is_empty(), "nothing was written");
    r.flags = Flags {
        degraded: true,
        ..Flags::default()
    };
    assert_eq!(w.setup.materialize(&r).unwrap().value.status, "applied");
}
