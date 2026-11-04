//! `agent.feature`'s write scenarios at the application port.

use std::sync::Arc;

use ctxloom_ports::Scope;

use crate::fakes::{plain, Mem, Setup, CONFIG, ENVELOPE};
use crate::write::requests::AgentRequest;
use crate::write::views::{DefaultAgent, Removal};
use crate::Library;

fn world_with(config: &'static str) -> (Library, Arc<Mem>) {
    Setup {
        bundles: &[(
            "project",
            &[
                ENVELOPE,
                ("profiles/dev.yaml", "bundles: []\nparents: [x]\n"),
            ],
        )],
        layers: &[config],
        ..Setup::default()
    }
    .fixture()
}

fn world() -> (Library, Arc<Mem>) {
    world_with(CONFIG)
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

fn write(
    library: &Library,
    name: &str,
    request: &AgentRequest,
    must_exist: bool,
) -> crate::Outcome<crate::AgentRow> {
    library.agent_write(&plain(), name, request, must_exist)
}

fn dev() -> AgentRequest {
    AgentRequest {
        llm: Some("claude-code".to_string()),
        profiles: Some(vec!["dev".to_string()]),
        ..AgentRequest::default()
    }
}

// agent.feature: Creating a binding records every axis
#[test]
fn test_create_records_every_axis_the_config_sorted_and_answers_the_binding() {
    let (library, mem) = world();
    let row = write(
        &library,
        "dev2",
        &AgentRequest {
            runtime: Some("container-rootless".to_string()),
            permissions: Some("plan".to_string()),
            surfaces: Some(vec![("context".to_string(), "system-prompt".to_string())]),
            roots: Some(vec![("mcp".to_string(), "project-root".to_string())]),
            engine_home: Some("session".to_string()),
            env_host: Some(false),
            env: Some(vec!["A".to_string(), "B".to_string()]),
            ..dev()
        },
        false,
    )
    .result
    .unwrap();
    assert_eq!(row.name, "dev2");
    assert_eq!(row.llm, "claude-code");
    assert_eq!(row.profiles, ["dev"]);
    assert_eq!(row.runtime, "container-rootless");
    assert_eq!(row.home_mode, "session");
    assert_eq!(row.env_host, Some(false));
    assert_eq!(row.env, ["A", "B"]);
    assert_eq!(
        row.permissions,
        serde_json::json!({"claude-code": {"mode": "plan"}})
    );
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 8\nagents:\n  dev2:\n    engine_home: session\n    env:\n      - A\n      - B\n    env_host: false\n    llm: claude-code\n    permissions:\n      claude-code:\n        mode: plan\n    profiles:\n      - dev\n    roots:\n      mcp: project-root\n    runtime: container-rootless\n    surfaces:\n      context: system-prompt\n"
    );
}

// operations.SetAgent: a mode the engine takes is written in its canonical spelling
#[test]
fn test_a_permission_mode_is_written_into_the_engines_block_in_its_spelling() {
    let (library, mem) = world();
    write(
        &library,
        "a",
        &AgentRequest {
            permissions: Some("ACCEPTEDITS".to_string()),
            ..dev()
        },
        false,
    )
    .result
    .unwrap();
    assert!(mem
        .config(Scope::Project)
        .unwrap()
        .contains("        mode: acceptEdits\n"));
    // The block is named for the backend the label maps to.
    let (library, mem) = Setup {
        layers: &["schema_version: 8\nllm:\n  configs:\n    big:\n      type: mock\n"],
        ..Setup::default()
    }
    .fixture();
    write(
        &library,
        "m",
        &AgentRequest {
            llm: Some("big".to_string()),
            permissions: Some("bypass".to_string()),
            ..AgentRequest::default()
        },
        false,
    )
    .result
    .unwrap();
    assert!(mem
        .config(Scope::Project)
        .unwrap()
        .contains("    permissions:\n      mock:\n        mode: bypass\n"));
}

// agent.feature: Create refuses an existing name, and leaves the live binding untouched
#[test]
fn test_create_refuses_a_name_that_is_there_and_edit_a_name_that_is_not() {
    let (library, mem) = world_with("schema_version: 8\nagents:\n  dev:\n    llm: claude-code\n");
    assert!(err(write(&library, "dev", &dev(), false)).contains("AgentExists"));
    assert!(err(write(&library, "nosuchagent", &dev(), true)).contains("AgentMissing"));
    assert_eq!(mem.writes(), 0);
}

// agent.feature: An edit naming only --runtime leaves every other axis intact
#[test]
fn test_an_edit_changes_only_the_axes_it_names() {
    let (library, mem) = world();
    write(
        &library,
        "developer",
        &AgentRequest {
            runtime: Some("host".to_string()),
            permissions: Some("plan".to_string()),
            profiles: Some(vec!["dev".to_string(), "ops".to_string()]),
            ..dev()
        },
        false,
    )
    .result
    .unwrap();
    let row = write(
        &library,
        "developer",
        &AgentRequest {
            runtime: Some("container-rootless".to_string()),
            ..AgentRequest::default()
        },
        true,
    )
    .result
    .unwrap();
    assert_eq!(row.runtime, "container-rootless");
    assert_eq!(row.llm, "claude-code");
    assert_eq!(row.profiles, ["dev", "ops"]);
    assert_eq!(
        row.permissions,
        serde_json::json!({"claude-code": {"mode": "plan"}})
    );
    let text = mem.config(Scope::Project).unwrap();
    assert!(text.contains("runtime: container-rootless"));
}

// agent.feature: An edit passing an empty --env clears the names and keeps env_host
#[test]
fn test_a_flag_named_empty_clears_its_axis_and_only_that_one() {
    let (library, _mem) = world();
    write(
        &library,
        "d",
        &AgentRequest {
            env_host: Some(false),
            env: Some(vec!["ALPHA".to_string()]),
            runtime: Some("host".to_string()),
            ..dev()
        },
        false,
    )
    .result
    .unwrap();
    let row = write(
        &library,
        "d",
        &AgentRequest {
            env: Some(vec![]),
            ..AgentRequest::default()
        },
        true,
    )
    .result
    .unwrap();
    assert!(row.env.is_empty());
    assert_eq!(row.env_host, Some(false));
    assert_eq!(row.runtime, "host");
    let row = write(
        &library,
        "d",
        &AgentRequest {
            runtime: Some(String::new()),
            ..AgentRequest::default()
        },
        true,
    )
    .result
    .unwrap();
    assert_eq!(row.runtime, "");
    let row = write(
        &library,
        "d",
        &AgentRequest {
            permissions: Some(String::new()),
            ..AgentRequest::default()
        },
        true,
    )
    .result
    .unwrap();
    assert_eq!(row.permissions, serde_json::json!({}));
    let row = write(
        &library,
        "d",
        &AgentRequest {
            surfaces: Some(vec![]),
            roots: Some(vec![]),
            ..AgentRequest::default()
        },
        true,
    )
    .result
    .unwrap();
    assert_eq!(row.llm, "claude-code");
}

/// Each request is refused with the named error, and the file is as it was.
fn refuses_all(cases: Vec<(AgentRequest, bool, &str)>) {
    let (library, mem) = world_with("schema_version: 8\nagents:\n  d:\n    llm: claude-code\n");
    let before = mem.config(Scope::Project);
    for (request, edit, said) in cases {
        let name = if edit { "d" } else { "n" };
        let got = err(write(&library, name, &request, edit));
        assert!(got.contains(said), "{request:?}: {got}");
    }
    assert_eq!(mem.config(Scope::Project), before);
    assert_eq!(mem.writes(), 0);
}

fn text(s: &str) -> Option<String> {
    Some(s.to_string())
}

fn pairs(kind: &str, value: &str) -> Option<Vec<(String, String)>> {
    Some(vec![(kind.to_string(), value.to_string())])
}

// operations.validateAgentAxes: each is refused, and the file is as it was
#[test]
fn test_every_scalar_axis_is_checked_before_anything_is_written() {
    let r = AgentRequest::default;
    refuses_all(vec![
        (
            AgentRequest {
                llm: text("nosuch"),
                ..r()
            },
            true,
            "UnknownEngine",
        ),
        (
            AgentRequest {
                runtime: text("weird"),
                ..r()
            },
            true,
            "UnknownRuntime",
        ),
        (
            AgentRequest {
                engine_home: text("weird"),
                ..r()
            },
            true,
            "UnknownEngineHome",
        ),
        (
            AgentRequest {
                permissions: text("weird"),
                ..r()
            },
            true,
            "Permissions",
        ),
        (
            AgentRequest {
                permissions: text("bypass"),
                ..r()
            },
            true,
            "declared as mode: bypassPermissions",
        ),
        (
            AgentRequest {
                llm: text(""),
                ..r()
            },
            true,
            "AgentWithoutEngine",
        ),
        (r(), false, "AgentWithoutEngine"),
    ]);
}

#[test]
fn test_env_roots_and_surfaces_are_checked_before_anything_is_written() {
    let r = AgentRequest::default;
    refuses_all(vec![
        (
            AgentRequest {
                env: Some(vec!["A=1".to_string()]),
                env_host: Some(false),
                ..r()
            },
            true,
            "BadEnv",
        ),
        (
            AgentRequest {
                env: Some(vec!["A".to_string()]),
                ..r()
            },
            true,
            "BadEnv",
        ),
        (
            AgentRequest {
                roots: pairs("context", "nowhere"),
                ..r()
            },
            true,
            "RootRefused",
        ),
        (
            AgentRequest {
                roots: pairs("bogus", "work-dir"),
                ..r()
            },
            true,
            "RootRefused",
        ),
        (
            AgentRequest {
                surfaces: pairs("context", "nosuch"),
                ..r()
            },
            true,
            "SurfaceRefused",
        ),
        (
            AgentRequest {
                surfaces: pairs("bogus", "x"),
                ..r()
            },
            true,
            "SurfaceRefused",
        ),
    ]);
}

#[test]
fn test_a_mode_a_surface_and_a_root_need_the_engine_they_are_checked_against() {
    let (library, mem) = world();
    let profiles = Some(vec!["dev".to_string()]);
    let no_engine = |r: AgentRequest| {
        err(write(
            &library,
            "n",
            &AgentRequest {
                profiles: profiles.clone(),
                ..r
            },
            false,
        ))
    };
    assert!(no_engine(AgentRequest {
        permissions: Some("plan".to_string()),
        ..AgentRequest::default()
    })
    .contains("PermissionsNeedEngine"));
    assert!(no_engine(AgentRequest {
        surfaces: Some(vec![("context".to_string(), "system-prompt".to_string())]),
        ..AgentRequest::default()
    })
    .contains("SurfaceNeedsEngine"));
    assert!(no_engine(AgentRequest {
        roots: Some(vec![("mcp".to_string(), "project-root".to_string())]),
        ..AgentRequest::default()
    })
    .contains("RootNeedsEngine"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_a_label_the_team_wrote_is_a_valid_engine_and_maps_to_its_backend() {
    let (library, _mem) = Setup {
        layers: &["schema_version: 8\nllm:\n  configs:\n    big:\n      type: mock\n"],
        ..Setup::default()
    }
    .fixture();
    let request = |mode: &str| AgentRequest {
        llm: Some("big".to_string()),
        permissions: Some(mode.to_string()),
        ..AgentRequest::default()
    };
    // mock's modes, not claude's.
    assert!(write(&library, "a", &request("acceptEdits"), false)
        .result
        .is_err());
    assert!(write(&library, "a", &request("bypass"), false)
        .result
        .is_ok());
    // The built-in registry's labels are engines too, where the project
    // declares no labels of its own.
    let (plain_library, _) = world();
    let fast = AgentRequest {
        llm: Some("claude-fast".to_string()),
        ..AgentRequest::default()
    };
    assert!(write(&plain_library, "b", &fast, false).result.is_ok());
}

// agent.feature: With nothing bound, the report says so; naming an agent binds it
#[test]
fn test_default_reports_the_bound_agent_and_binds_a_named_one() {
    let (library, mem) = world();
    assert_eq!(
        library.agent_default(&plain(), None).result.unwrap(),
        DefaultAgent::Current(String::new())
    );
    let set = library
        .agent_default(&plain(), Some("ghost"))
        .result
        .unwrap();
    assert_eq!(
        set,
        DefaultAgent::Set {
            name: "ghost".to_string(),
            defined: false
        }
    );
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 8\ndefault_agent: ghost\n"
    );
    assert_eq!(
        library.agent_default(&plain(), None).result.unwrap(),
        DefaultAgent::Current("ghost".to_string())
    );
    write(&library, "dev", &dev(), false).result.unwrap();
    let set = library.agent_default(&plain(), Some("dev")).result.unwrap();
    assert_eq!(
        set,
        DefaultAgent::Set {
            name: "dev".to_string(),
            defined: true
        }
    );
}

// agent.feature: Bare remove reports and destroys nothing / --yes takes the binding out
#[test]
fn test_remove_previews_then_takes_exactly_that_binding() {
    let (library, mem) = world_with(
        "schema_version: 8\nagents:\n  a:\n    llm: claude-code\n  b:\n    llm: claude-code\n",
    );
    let Removal::Preview(p) = library.agent_remove(&plain(), "a", false).result.unwrap() else {
        panic!("a preview")
    };
    assert_eq!(
        (p.target.as_str(), p.apply.as_str()),
        ("agent \"a\"", "ctxloom agent remove a --yes")
    );
    assert!(p.detail.is_empty());
    assert_eq!(mem.writes(), 0);
    let Removal::Done(done) = library.agent_remove(&plain(), "a", true).result.unwrap() else {
        panic!("done")
    };
    assert_eq!((done.status.as_str(), done.name.as_str()), ("removed", "a"));
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 8\nagents:\n  b:\n    llm: claude-code\n"
    );
    library.agent_remove(&plain(), "b", true).result.unwrap();
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 8\n",
        "the empty section goes with its last agent"
    );
}

// agent.feature: Removing a binding nobody defined fails rather than reporting success
#[test]
fn test_remove_of_a_binding_nobody_defined_fails_both_ways() {
    let (library, mem) = world();
    assert!(err(library.agent_remove(&plain(), "nosuchagent", false)).contains("AgentNotDefined"));
    assert!(err(library.agent_remove(&plain(), "nosuchagent", true)).contains("AgentNotInConfig"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_the_home_config_alone_is_never_written_by_an_agent_command() {
    let (library, mem) = Setup {
        layers: &["schema_version: 8\neditor:\n  command: 'true'\n", CONFIG],
        ..Setup::default()
    }
    .fixture();
    write(&library, "dev", &dev(), false).result.unwrap();
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        "schema_version: 8\neditor:\n  command: 'true'\n"
    );
    assert!(mem.config(Scope::Project).unwrap().contains("agents:"));
}
