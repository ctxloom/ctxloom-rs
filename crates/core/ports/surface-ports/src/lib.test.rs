use super::*;

#[test]
fn test_a_kind_is_found_by_the_name_it_reports() {
    for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.name()), Some(kind));
    }
    assert_eq!(Kind::parse("nothing"), None);
}

#[test]
fn test_the_kinds_are_listed_in_delivery_order() {
    let mut sorted = Kind::ALL;
    sorted.sort();
    assert_eq!(sorted, Kind::ALL);
}

#[test]
fn test_a_ctxloom_callback_is_the_bare_executable_running_hook() {
    let callback = Hook {
        command: "ctxloom".into(),
        args: vec!["hook".into(), "skill-mates".into()],
        ..Hook::default()
    };
    assert!(callback.is_callback());
    let other = Hook {
        command: "ctxloom".into(),
        args: vec!["profile".into(), "list".into()],
        ..Hook::default()
    };
    assert!(!other.is_callback());
    let shell = Hook {
        command: "echo hook".into(),
        ..Hook::default()
    };
    assert!(!shell.is_callback());
}

#[test]
fn test_a_claim_names_the_file_it_is_on() {
    let section = Claim::Section {
        path: "CLAUDE.md".into(),
        text: String::new(),
    };
    let json = Claim::Json {
        path: ".mcp.json".into(),
        ops: Vec::new(),
    };
    let whole = Claim::Whole {
        path: "a/b.md".into(),
        bytes: Vec::new(),
        executable: false,
    };
    assert_eq!(section.path(), "CLAUDE.md");
    assert_eq!(json.path(), ".mcp.json");
    assert_eq!(whole.path(), "a/b.md");
}

struct Named(&'static str, bool);

impl EngineSurfaces for Named {
    fn name(&self) -> &str {
        self.0
    }
    fn carries(&self, _: Kind) -> bool {
        false
    }
    fn context_rides_launch(&self) -> bool {
        self.1
    }
    fn plan(&self, _: Kind, _: &Inputs) -> Result<Planned, SurfaceError> {
        unreachable!()
    }
    fn wiring(&self, _: &dyn SurfaceStore, _: &str) -> Result<Wiring, SurfaceError> {
        unreachable!()
    }
    fn codec(&self) -> &dyn HookCodec {
        unreachable!()
    }
}

struct Plain;

impl EngineSurfaces for Plain {
    fn name(&self) -> &str {
        "plain"
    }
    fn carries(&self, _: Kind) -> bool {
        false
    }
    fn plan(&self, _: Kind, _: &Inputs) -> Result<Planned, SurfaceError> {
        unreachable!()
    }
    fn wiring(&self, _: &dyn SurfaceStore, _: &str) -> Result<Wiring, SurfaceError> {
        unreachable!()
    }
    fn codec(&self) -> &dyn HookCodec {
        unreachable!()
    }
}

#[test]
fn test_the_registry_finds_an_engine_by_name_and_lists_them_in_order() {
    let registry = Registry::new(vec![
        Box::new(Named("b", false)),
        Box::new(Named("a", true)),
    ]);
    assert_eq!(registry.names(), vec!["b".to_string(), "a".to_string()]);
    assert_eq!(
        registry.get("a").map(|e| e.name().to_string()),
        Some("a".into())
    );
    assert_eq!(
        registry.get("b").map(|e| e.name().to_string()),
        Some("b".into())
    );
    assert!(registry.get("c").is_none());
    assert!(Registry::default().names().is_empty());
}

#[test]
fn test_context_rides_a_launch_only_when_the_engine_says_so() {
    assert!(!Plain.context_rides_launch());
    assert!(Named("x", true).context_rides_launch());
}

#[test]
fn test_a_hook_spells_its_shared_members_in_order() {
    let hook = Hook {
        command: "ctxloom".into(),
        args: vec!["hook".into(), "stop".into()],
        prompt: "p".into(),
        timeout: 5,
        is_async: true,
        tool: "shell".into(),
        matcher: "m".into(),
        ..Hook::default()
    };
    let members = hook.json_members("eng");
    let keys: Vec<_> = members.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys, ["type", "command", "args", "prompt", "timeout"]);
    assert_eq!(members[0].1, Json::text("command"));
    assert_eq!(
        members[2].1,
        Json::strings(["hook", "stop", "--engine", "eng"])
    );
    assert_eq!(members[4].1, Json::Number("5".into()));
    let bare = Hook::default().json_members("eng");
    assert_eq!(bare, vec![("type", Json::text("command"))]);
}

#[test]
fn test_an_engine_declares_no_hook_losses_unless_it_says_so() {
    assert!(Plain.hook_losses().is_empty());
}
