use surface_ports::{Command, Skill, SkillFile};

use super::*;

fn paths(planned: &Planned) -> Vec<&str> {
    planned.claims.iter().map(Claim::path).collect()
}

#[test]
fn test_mock_carries_every_kind_in_its_own_files() {
    let mock = Mock::standard();
    assert_eq!(mock.name(), "mock");
    let inputs = Inputs {
        context: "ctx".into(),
        deny_tools: vec!["Bash".into()],
        statusline: true,
        commands: vec![Command {
            name: "demo/example".into(),
            description: "d".into(),
            body: "body".into(),
        }],
        skills: vec![Skill {
            name: "reviewer".into(),
            description: "d".into(),
            files: vec![SkillFile {
                path: "SKILL.md".into(),
                bytes: b"s".to_vec(),
                executable: false,
            }],
        }],
        ..Inputs::default()
    };
    let at = |kind| mock.plan(kind, &inputs).unwrap();
    assert_eq!(paths(&at(Kind::Context)), vec!["MOCK_CONTEXT.md"]);
    assert_eq!(paths(&at(Kind::Mcp)), vec![".mock/mcp.json"]);
    assert_eq!(paths(&at(Kind::Settings)), vec![".mock/settings.json"]);
    assert_eq!(paths(&at(Kind::Hooks)), vec![".mock/hooks.json"]);
    assert_eq!(
        paths(&at(Kind::Commands)),
        vec![".mock/commands/demo/example.md"]
    );
    assert_eq!(
        paths(&at(Kind::Skills)),
        vec![".mock/skills/reviewer/SKILL.md"]
    );
    let Claim::Whole { bytes, .. } = &at(Kind::Commands).claims[0] else {
        panic!()
    };
    assert_eq!(bytes, b"body");
    let Claim::Whole { bytes, .. } = &at(Kind::Settings).claims[0] else {
        panic!()
    };
    let doc = Json::parse(&String::from_utf8(bytes.clone()).unwrap()).unwrap();
    assert_eq!(doc.get("statusline"), Some(&Json::Bool(true)));
}

#[test]
fn test_mock_writes_its_mcp_registry_even_with_no_server() {
    let planned = Mock::standard()
        .plan(Kind::Mcp, &Inputs::default())
        .unwrap();
    let Claim::Whole { bytes, .. } = &planned.claims[0] else {
        panic!()
    };
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        "{\n  \"mcpServers\": {}\n}\n"
    );
}

#[test]
fn test_the_home_keyed_variant_carries_the_context_and_nothing_else() {
    let mock = Mock::home_keyed();
    assert_eq!(mock.name(), "mock-launch");
    assert!(mock.carries(Kind::Context));
    for kind in [
        Kind::Mcp,
        Kind::Settings,
        Kind::Hooks,
        Kind::Commands,
        Kind::Skills,
    ] {
        assert!(!mock.carries(kind));
        assert!(mock
            .plan(kind, &Inputs::default())
            .unwrap()
            .claims
            .is_empty());
    }
}

#[test]
fn test_a_name_that_climbs_is_skipped_and_said() {
    let inputs = Inputs {
        commands: vec![Command {
            name: "../x".into(),
            ..Command::default()
        }],
        skills: vec![Skill {
            name: "/abs".into(),
            ..Skill::default()
        }],
        ..Inputs::default()
    };
    for kind in [Kind::Commands, Kind::Skills] {
        let planned = Mock::standard().plan(kind, &inputs).unwrap();
        assert!(planned.claims.is_empty());
        assert_eq!(planned.skipped.len(), 1);
    }
}

#[test]
fn test_hooks_are_written_by_event_and_a_callback_names_the_engine() {
    let mut hooks = Hooks::new();
    hooks.insert(
        "session_start".into(),
        vec![Hook {
            command: "ctxloom".into(),
            args: vec!["hook".into(), "session-start".into()],
            ..Hook::default()
        }],
    );
    let planned = Mock::standard()
        .plan(
            Kind::Hooks,
            &Inputs {
                hooks,
                ..Inputs::default()
            },
        )
        .unwrap();
    let Claim::Whole { bytes, .. } = &planned.claims[0] else {
        panic!()
    };
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.contains("\"--engine\"") && text.contains("\"mock\""),
        "{text}"
    );
}

struct Files(BTreeMap<String, Vec<u8>>);

impl SurfaceStore for Files {
    fn deliver(
        &self,
        _: &str,
        _: &str,
        _: Kind,
        _: &[Claim],
        _: bool,
    ) -> Result<surface_ports::Delivered, SurfaceError> {
        unreachable!()
    }
    fn release(
        &self,
        _: &str,
        _: &str,
        _: Kind,
        _: bool,
    ) -> Result<surface_ports::Released, SurfaceError> {
        unreachable!()
    }
    fn claimed(&self, _: &str, _: &str) -> Result<Vec<Kind>, SurfaceError> {
        unreachable!()
    }
    fn read(&self, _: &str, rel: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok(self.0.get(rel).cloned())
    }
    fn read_absolute(&self, _: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        unreachable!()
    }
    fn inspect(&self, _: &str) -> Result<surface_ports::Inspected, SurfaceError> {
        unreachable!()
    }
}

#[test]
fn test_the_wiring_is_read_off_the_mock_files() {
    let mock = Mock::standard();
    assert_eq!(
        mock.wiring(&Files(BTreeMap::new()), "t").unwrap(),
        Wiring::default()
    );
    let files = Files(BTreeMap::from([
        (
            ".mock/settings.json".to_string(),
            br#"{"statusline": true}"#.to_vec(),
        ),
        (
            ".mock/hooks.json".to_string(),
            br#"{"session_start": [{"command": "ctxloom", "args": ["hook", "x"]}]}"#.to_vec(),
        ),
        (
            ".mock/mcp.json".to_string(),
            br#"{"mcpServers": {"ctxloom": {}}}"#.to_vec(),
        ),
    ]));
    assert_eq!(
        mock.wiring(&files, "t").unwrap(),
        Wiring {
            settings_exists: true,
            hooks_present: true,
            statusline: true,
            mcp_present: true
        }
    );
}

#[test]
fn test_only_a_name_that_stays_inside_is_safe_to_make_a_path_of() {
    for ok in ["a", "a/b", "a.b", "a/b/c.md", "..a", ".hidden"] {
        assert!(safe(ok), "{ok}");
    }
    for bad in [
        "", "/a", "a\\b", "a//b", "a/", "/", "./a", "a/./b", "a/..", "../a", "a/../b",
    ] {
        assert!(!safe(bad), "{bad}");
    }
}

#[test]
fn test_a_bare_hook_is_written_as_a_command_hook_with_nothing_else() {
    assert_eq!(
        hook_json("mock", &Hook::default()),
        Json::object([("type", Json::text("command"))])
    );
    // A zero or negative timeout is no timeout.
    for timeout in [0, -1] {
        let hook = Hook {
            timeout,
            ..Hook::default()
        };
        assert_eq!(
            hook_json("mock", &hook),
            Json::object([("type", Json::text("command"))])
        );
    }
}

#[test]
fn test_every_field_of_a_hook_is_written_where_it_is_set() {
    let hook = Hook {
        kind: "prompt".into(),
        command: "run".into(),
        args: vec!["a".into(), "b".into()],
        prompt: "p".into(),
        timeout: 1,
        tool: "shell".into(),
        matcher: "Bash".into(),
        ..Hook::default()
    };
    assert_eq!(
        hook_json("mock", &hook),
        Json::object([
            ("type", Json::text("prompt")),
            ("command", Json::text("run")),
            ("args", Json::strings(["a", "b"])),
            ("prompt", Json::text("p")),
            ("timeout", Json::Number("1".into())),
            ("tool", Json::text("shell")),
            ("matcher", Json::text("Bash")),
        ])
    );
    for (field, hook) in [
        (
            "command",
            Hook {
                command: "c".into(),
                ..Hook::default()
            },
        ),
        (
            "prompt",
            Hook {
                prompt: "c".into(),
                ..Hook::default()
            },
        ),
        (
            "tool",
            Hook {
                tool: "c".into(),
                ..Hook::default()
            },
        ),
        (
            "matcher",
            Hook {
                matcher: "c".into(),
                ..Hook::default()
            },
        ),
    ] {
        let doc = hook_json("mock", &hook);
        let Json::Object(members) = doc else { panic!() };
        assert_eq!(members.len(), 2, "{field}");
    }
}

#[test]
fn test_only_a_callback_is_given_the_engine_and_never_twice() {
    let args = |doc: Json| doc.get("args").cloned();
    let callback = Hook {
        command: "ctxloom".into(),
        args: vec!["hook".into(), "x".into()],
        ..Hook::default()
    };
    assert_eq!(
        args(hook_json("mock", &callback)),
        Some(Json::strings(["hook", "x", "--engine", "mock"]))
    );
    let named = Hook {
        args: vec!["hook".into(), "--engine".into(), "other".into()],
        ..callback.clone()
    };
    assert_eq!(
        args(hook_json("mock", &named)),
        Some(Json::strings(["hook", "--engine", "other"]))
    );
    let foreign = Hook {
        command: "other".into(),
        args: vec!["hook".into()],
        ..Hook::default()
    };
    assert_eq!(
        args(hook_json("mock", &foreign)),
        Some(Json::strings(["hook"]))
    );
    let bare = Hook {
        command: "ctxloom".into(),
        ..Hook::default()
    };
    assert_eq!(args(hook_json("mock", &bare)), None);
}

#[test]
fn test_an_mcp_server_is_written_with_only_the_fields_it_has() {
    let server = |name: &str, command: &str, args: &[&str], url: &str| surface_ports::McpServer {
        name: name.into(),
        command: command.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        url: url.into(),
        ..Default::default()
    };
    let inputs = Inputs {
        mcp: vec![
            server("full", "run", &["a"], "http://x"),
            server("cmd", "run", &[], ""),
            server("args", "", &["a"], ""),
            server("url", "", &[], "http://x"),
            server("none", "", &[], ""),
        ],
        ..Inputs::default()
    };
    let planned = Mock::standard().plan(Kind::Mcp, &inputs).unwrap();
    let Claim::Whole { bytes, .. } = &planned.claims[0] else {
        panic!()
    };
    let doc = Json::parse(&String::from_utf8(bytes.clone()).unwrap()).unwrap();
    let servers = doc.get("mcpServers").unwrap();
    assert_eq!(
        servers.get("full").unwrap(),
        &Json::object([
            ("command", Json::text("run")),
            ("args", Json::strings(["a"])),
            ("url", Json::text("http://x")),
        ])
    );
    assert_eq!(
        servers.get("cmd").unwrap(),
        &Json::object([("command", Json::text("run"))])
    );
    assert_eq!(
        servers.get("args").unwrap(),
        &Json::object([("args", Json::strings(["a"]))])
    );
    assert_eq!(
        servers.get("url").unwrap(),
        &Json::object([("url", Json::text("http://x"))])
    );
    assert_eq!(servers.get("none").unwrap(), &Json::object::<&str>([]));
}

#[test]
fn test_a_hook_file_is_ctxloom_wiring_only_for_the_bare_ctxloom_hook_command() {
    let wired = |hooks: &str| {
        Mock::standard()
            .wiring(
                &Files(BTreeMap::from([(
                    ".mock/hooks.json".to_string(),
                    hooks.as_bytes().to_vec(),
                )])),
                "t",
            )
            .unwrap()
            .hooks_present
    };
    assert!(wired(
        r#"{"e": [{"command": "ctxloom", "args": ["hook"]}]}"#
    ));
    assert!(!wired(
        r#"{"e": [{"command": "ctxloom", "args": ["other"]}]}"#
    ));
    assert!(!wired(r#"{"e": [{"command": "other", "args": ["hook"]}]}"#));
    assert!(!wired(r#"{"e": [{"command": "ctxloom"}]}"#));
}
