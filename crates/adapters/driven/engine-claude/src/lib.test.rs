use std::collections::BTreeMap;

use surface_ports::{Hook, SkillFile};

use super::*;

fn plan(kind: Kind, inputs: &Inputs) -> Planned {
    Claude::new().plan(kind, inputs).unwrap()
}

fn ops(planned: &Planned) -> Vec<JsonOp> {
    match planned.claims.as_slice() {
        [Claim::Json { ops, .. }] => ops.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_the_context_is_a_section_of_claude_md_or_the_file_asked_for() {
    let inputs = Inputs {
        context: "ctx".into(),
        ..Inputs::default()
    };
    assert_eq!(
        plan(Kind::Context, &inputs).claims,
        vec![Claim::Section {
            path: "CLAUDE.md".into(),
            text: "ctx".into()
        }]
    );
    let named = Inputs {
        context_file: Some("docs/AGENTS.md".into()),
        ..inputs
    };
    assert_eq!(
        plan(Kind::Context, &named).claims[0].path(),
        "docs/AGENTS.md"
    );
}

#[test]
fn test_each_server_is_an_entry_under_mcp_servers_and_none_conjures_no_file() {
    assert!(plan(Kind::Mcp, &Inputs::default()).claims.is_empty());
    let inputs = Inputs {
        mcp: vec![
            McpServer {
                name: "tool".into(),
                command: "tool-cmd".into(),
                args: vec!["--flag".into()],
                env: BTreeMap::from([("K".to_string(), "V".to_string())]),
                ..McpServer::default()
            },
            McpServer {
                name: "remote".into(),
                url: "https://x.example/mcp".into(),
                headers: BTreeMap::from([("A".to_string(), "b".to_string())]),
                ..McpServer::default()
            },
        ],
        ..Inputs::default()
    };
    let ops = ops(&plan(Kind::Mcp, &inputs));
    let entry = |i: usize| match &ops[i] {
        JsonOp::Set { path, value } => (path.clone(), value.pretty()),
        other => panic!("{other:?}"),
    };
    assert_eq!(entry(0).0, vec!["mcpServers", "tool"]);
    assert_eq!(
        entry(0).1,
        Json::parse(r#"{"command":"tool-cmd","args":["--flag"],"env":{"K":"V"}}"#)
            .unwrap()
            .pretty()
    );
    assert_eq!(
        entry(1).1,
        Json::parse(r#"{"type":"http","url":"https://x.example/mcp","headers":{"A":"b"}}"#)
            .unwrap()
            .pretty()
    );
}

#[test]
fn test_the_statusline_and_each_denied_tool_once_are_the_settings() {
    assert!(plan(Kind::Settings, &Inputs::default()).claims.is_empty());
    let inputs = Inputs {
        statusline: true,
        deny_tools: vec!["Bash".into(), "".into(), "Bash".into(), "Edit".into()],
        ..Inputs::default()
    };
    let ops = ops(&plan(Kind::Settings, &inputs));
    assert_eq!(ops.len(), 3);
    assert!(matches!(&ops[0], JsonOp::SetCommand { path, value }
        if path == &["statusLine"] && value.get("command").and_then(Json::as_str) == Some("ctxloom hook hud")));
    assert!(matches!(&ops[1], JsonOp::Push { path, .. } if path == &["permissions", "deny"]));
}

#[test]
fn test_hooks_are_claims_on_the_same_settings_file() {
    let mut hooks = BTreeMap::new();
    hooks.insert(
        "session_start".to_string(),
        vec![Hook {
            command: "echo hi".into(),
            ..Hook::default()
        }],
    );
    let planned = plan(
        Kind::Hooks,
        &Inputs {
            hooks,
            ..Inputs::default()
        },
    );
    assert_eq!(planned.claims[0].path(), ".claude/settings.json");
    assert!(plan(Kind::Hooks, &Inputs::default()).claims.is_empty());
}

#[test]
fn test_commands_and_skills_are_files_under_the_claude_directory() {
    let inputs = Inputs {
        commands: vec![surface_ports::Command {
            name: "b/c".into(),
            description: String::new(),
            body: "x".into(),
        }],
        skills: vec![surface_ports::Skill {
            name: "s".into(),
            description: "d".into(),
            files: vec![SkillFile {
                path: "SKILL.md".into(),
                bytes: vec![],
                executable: false,
            }],
        }],
        ..Inputs::default()
    };
    assert_eq!(
        plan(Kind::Commands, &inputs).claims[0].path(),
        ".claude/commands/b-c.md"
    );
    assert_eq!(
        plan(Kind::Skills, &inputs).claims[0].path(),
        ".claude/skills/s/SKILL.md"
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
fn test_the_wiring_is_read_off_the_files_not_the_record() {
    let none = Claude::new().wiring(&Files(BTreeMap::new()), "t").unwrap();
    assert_eq!(none, Wiring::default());
    let settings = r#"{"statusLine":{"type":"command","command":"ctxloom hook hud"},"hooks":{"SessionStart":[{"hooks":[{"command":"ctxloom","args":["hook","session-start"]}]}]}}"#;
    let mcp = r#"{"mcpServers":{"ctxloom":{"command":"x"}}}"#;
    let files = Files(BTreeMap::from([
        (
            ".claude/settings.json".to_string(),
            settings.as_bytes().to_vec(),
        ),
        (".mcp.json".to_string(), mcp.as_bytes().to_vec()),
    ]));
    assert_eq!(
        Claude::new().wiring(&files, "t").unwrap(),
        Wiring {
            settings_exists: true,
            hooks_present: true,
            statusline: true,
            mcp_present: true
        }
    );
    let theirs = Files(BTreeMap::from([(
        ".claude/settings.json".to_string(),
        br#"{"statusLine":{"type":"command","command":"ccusage"}}"#.to_vec(),
    )]));
    let wiring = Claude::new().wiring(&theirs, "t").unwrap();
    assert!(wiring.settings_exists && !wiring.statusline && !wiring.hooks_present);
}

#[test]
fn test_the_engine_carries_every_kind_under_its_name() {
    let claude = Claude::new();
    assert_eq!(claude.name(), "claude-code");
    assert!(Kind::ALL.iter().all(|k| claude.carries(*k)));
}
