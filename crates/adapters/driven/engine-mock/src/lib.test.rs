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
