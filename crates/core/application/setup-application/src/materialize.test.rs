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

mod pinned {
    use surface_ports::EngineSurfaces;

    use super::*;
    use crate::fakes::{world, world_of, Engine, Wire, World, CONFIG as CONFIG_DEV};

    fn mock_only() -> Vec<Box<dyn EngineSurfaces>> {
        vec![Box::new(Engine {
            name: "mock",
            kinds: vec![Kind::Context],
            wire: Wire,
            rides_launch: false,
            hook_losses: Vec::new(),
        })]
    }

    fn named(done: &MaterializeResult) -> Vec<String> {
        outcome(done, "mock")
            .not_carried
            .iter()
            .map(|n| n.surface.clone())
            .collect()
    }

    /// A project whose only bundle is bare, under `config`.
    fn bare(config: &'static str, profile: &'static str) -> World {
        world_of(
            config,
            vec![(
                "project",
                vec![
                    ("bundle.yaml", "schema_version: 4\nversion: 1.0.0\n"),
                    ("fragments/f.md", "BODY"),
                    ("profiles/dev.yaml", profile),
                ],
            )],
            mock_only(),
        )
    }

    const CFG: &str = "schema_version: 8\nagents:\n  default:\n    llm: mock\n    profiles: [dev]\ndefault_agent: default\nllm:\n  configs:\n    mock:\n      type: mock\n";
    const CFG_NO_HUD: &str = "schema_version: 8\nconfig:\n  statusline: false\nagents:\n  default:\n    llm: mock\n    profiles: [dev]\ndefault_agent: default\nllm:\n  configs:\n    mock:\n      type: mock\n";

    #[test]
    fn a_bare_project_names_only_what_it_carries_items_for() {
        // ctxloom's own hooks always ride; the HUD is on by default.
        let done = bare(CFG, "bundles: []\n")
            .setup
            .materialize(&request(&["mock"]))
            .unwrap()
            .value;
        assert_eq!(named(&done), ["settings", "hooks"]);
    }

    #[test]
    fn settings_are_named_for_either_the_hud_or_denied_tools_alone() {
        let neither = bare(CFG_NO_HUD, "bundles: []\n")
            .setup
            .materialize(&request(&["mock"]))
            .unwrap()
            .value;
        assert_eq!(named(&neither), ["hooks"]);
        let denied = bare(CFG_NO_HUD, "bundles: []\ndeny_tools: [Bash]\n")
            .setup
            .materialize(&request(&["mock"]))
            .unwrap()
            .value;
        assert_eq!(named(&denied), ["settings", "hooks"]);
    }

    #[test]
    fn mcp_commands_and_skills_are_named_when_the_project_has_them() {
        let done = world(CONFIG_DEV)
            .setup
            .materialize(&request(&["mock"]))
            .unwrap()
            .value;
        let names = named(&done);
        for kind in ["mcp", "settings", "hooks", "commands", "skills"] {
            assert!(names.contains(&kind.to_string()), "{kind}: {names:?}");
        }
    }

    fn riding() -> Vec<Box<dyn EngineSurfaces>> {
        vec![
            Box::new(Engine {
                name: "rides",
                kinds: Kind::ALL.to_vec(),
                wire: Wire,
                rides_launch: true,
                hook_losses: Vec::new(),
            }),
            Box::new(Engine::all("stays")),
        ]
    }

    fn wrote(done: &MaterializeResult, engine: &str) -> Vec<String> {
        outcome(done, engine).wrote
    }

    #[test]
    fn leaving_the_launch_context_drops_context_only_for_engines_that_ride_it() {
        let w = world_of(CONFIG_DEV, crate::fakes::bundles(), riding());
        let mut r = request(&["rides", "stays"]);
        r.leave_launch_context = true;
        let done = w.setup.materialize(&r).unwrap().value;
        assert!(!wrote(&done, "rides").contains(&"context".to_string()));
        assert!(wrote(&done, "rides").contains(&"hooks".to_string()));
        assert!(wrote(&done, "stays").contains(&"context".to_string()));
    }

    #[test]
    fn an_engine_that_rides_the_launch_still_gets_context_unless_asked_not_to() {
        let w = world_of(CONFIG_DEV, crate::fakes::bundles(), riding());
        let done = w.setup.materialize(&request(&["rides"])).unwrap().value;
        assert!(wrote(&done, "rides").contains(&"context".to_string()));
    }

    #[test]
    fn the_result_names_the_profiles_delivered_and_the_release_none() {
        let w = world(CONFIG_DEV);
        let done = w.setup.materialize(&request(&["mock"])).unwrap().value;
        assert_eq!(done.profiles, ["dev"]);
        let mut named = request(&["mock"]);
        named.profiles = vec!["dev".to_string()];
        assert_eq!(w.setup.materialize(&named).unwrap().value.profiles, ["dev"]);
        let mut release = request(&["mock"]);
        release.release = true;
        let released = w.setup.materialize(&release).unwrap().value;
        assert!(released.profiles.is_empty());
        assert_eq!(released.engines[0].engine, "mock");
    }

    #[test]
    fn only_a_delivery_to_a_missing_target_creates_it() {
        let w = world(CONFIG_DEV);
        w.store
            .state()
            .targets
            .insert("/p/out".into(), Inspected::Missing);
        let mut dry = request(&["mock"]);
        dry.dry_run = true;
        assert!(!w.setup.materialize(&dry).unwrap().value.created);
        assert!(
            w.setup
                .materialize(&request(&["mock"]))
                .unwrap()
                .value
                .created
        );
        let existing = world(CONFIG_DEV);
        assert!(
            !existing
                .setup
                .materialize(&request(&["mock"]))
                .unwrap()
                .value
                .created
        );
    }

    #[test]
    fn a_kind_is_written_when_delivered_and_in_a_dry_run_only_when_it_would_change() {
        let w = world(CONFIG_DEV);
        w.setup.materialize(&request(&["mock"])).unwrap();
        // Delivered again unchanged: still written.
        let again = w.setup.materialize(&request(&["mock"])).unwrap().value;
        assert_eq!(wrote(&again, "mock"), ["context"]);
        // A dry run of the same: nothing would change.
        let mut dry = request(&["mock"]);
        dry.dry_run = true;
        let planned = w.setup.materialize(&dry).unwrap().value;
        assert!(wrote(&planned, "mock").is_empty(), "{planned:?}");
    }

    #[test]
    fn a_refusal_over_a_finding_names_the_finding() {
        let w = world(CONFIG_DEV);
        let mut r = request(&["claude-code"]);
        r.profiles = vec!["broken".to_string()];
        let SetupError::Refused { findings } = w.setup.materialize(&r).unwrap_err() else {
            panic!("refused")
        };
        assert!(
            findings.contains("failed to load bundle \"absent\""),
            "{findings}"
        );
    }

    #[test]
    fn one_engine_failing_of_two_is_partial() {
        let w = world(CONFIG_DEV);
        w.store.state().refuse_path = Some("claude-code/settings".to_string());
        let done = w
            .setup
            .materialize(&request(&["claude-code", "mock"]))
            .unwrap()
            .value;
        assert_eq!(done.failures.len(), 1);
        assert_eq!(done.status, "partial");
    }
}

// materialize_loss_report_test.go: an engine that carries hooks but has no
// native form for an event the profile configures names that event and why;
// an event the profile does not configure costs nothing and is not named.
mod hook_losses {
    use surface_ports::EngineSurfaces;

    use super::*;
    use crate::fakes::{bundles, world_of, Engine, Wire};

    fn lossy(events: &[&str]) -> Box<dyn EngineSurfaces> {
        Box::new(Engine {
            name: "lossy",
            kinds: Kind::ALL.to_vec(),
            wire: Wire,
            rides_launch: false,
            hook_losses: events
                .iter()
                .map(|e| (e.to_string(), format!("lossy has no native {e} event")))
                .collect(),
        })
    }

    fn with_team_hook() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
        let mut all = bundles();
        let demo = &mut all[0].1;
        demo.push((
            "hooks/session_start/guard.yaml",
            "type: command\ncommand: echo guard\n",
        ));
        all
    }

    fn lost(done: &MaterializeResult) -> Vec<(String, String)> {
        outcome(done, "lossy")
            .not_carried
            .into_iter()
            .map(|n| (n.surface, n.reason))
            .collect()
    }

    #[test]
    fn a_configured_event_the_engine_cannot_fire_is_named_with_its_reason() {
        let w = world_of(
            CONFIG,
            with_team_hook(),
            vec![lossy(&["session_start", "session_end"])],
        );
        let done = w.setup.materialize(&request(&["lossy"])).unwrap().value;
        assert_eq!(
            lost(&done),
            [(
                "hooks".to_string(),
                "lossy has no native session_start event".to_string()
            )],
            "session_end is not configured, so it is not a loss"
        );
        assert!(outcome(&done, "lossy").wrote.contains(&"hooks".to_string()));
    }

    #[test]
    fn ctxloom_own_callbacks_are_not_the_teams_loss() {
        let w = world_of(CONFIG, bundles(), vec![lossy(&["session_start"])]);
        let done = w.setup.materialize(&request(&["lossy"])).unwrap().value;
        assert_eq!(lost(&done), []);
    }

    #[test]
    fn nothing_is_named_when_hooks_are_not_asked_for_or_are_released() {
        let w = world_of(CONFIG, with_team_hook(), vec![lossy(&["session_start"])]);
        let mut only_context = request(&["lossy"]);
        only_context.surfaces = vec![SurfaceSpec::parse("context").unwrap()];
        assert_eq!(lost(&w.setup.materialize(&only_context).unwrap().value), []);
        let mut release = request(&["lossy"]);
        release.release = true;
        assert_eq!(lost(&w.setup.materialize(&release).unwrap().value), []);
    }

    #[test]
    fn an_engine_with_no_hooks_at_all_is_reported_the_ordinary_way_only() {
        let w = world_of(
            CONFIG,
            with_team_hook(),
            vec![Box::new(Engine {
                name: "lossy",
                kinds: vec![Kind::Context],
                wire: Wire,
                rides_launch: false,
                hook_losses: vec![("session_start".into(), "x".into())],
            })],
        );
        let done = w.setup.materialize(&request(&["lossy"])).unwrap().value;
        let reasons: Vec<String> = outcome(&done, "lossy")
            .not_carried
            .into_iter()
            .filter(|n| n.surface == "hooks")
            .map(|n| n.reason)
            .collect();
        assert_eq!(reasons, ["lossy declares no approach for it"]);
    }
}
