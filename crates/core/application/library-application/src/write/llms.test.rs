//! `llm.feature`'s write scenarios at the application port.

use std::sync::Arc;

use ctxloom_ports::Scope;

use crate::fakes::{plain, Mem, Setup, CONFIG};
use crate::write::requests::LlmRequest;
use crate::write::views::{LlmDefault, Removal};
use crate::Library;

fn world() -> (Library, Arc<Mem>) {
    Setup {
        layers: &["schema_version: 9\neditor:\n  command: 'true'\n", CONFIG],
        ..Setup::default()
    }
    .fixture()
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

fn big() -> LlmRequest {
    LlmRequest {
        kind: Some("mock".to_string()),
        model: Some("o1".to_string()),
        ..LlmRequest::default()
    }
}

// llm.feature: A labeled engine config is created and read back out of the project file
#[test]
fn test_create_writes_the_entry_sorted_in_the_project_config_and_answers_it() {
    let (library, mem) = world();
    let done = library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    assert_eq!(
        (done.label.as_str(), done.kind.as_str(), done.model.as_str()),
        ("big", "mock", "o1")
    );
    assert_eq!(done.permissions, serde_json::json!({}));
    assert!(!done.edited);
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 9\nllm:\n  configs:\n    big:\n      model: o1\n      type: mock\n"
    );
    assert_eq!(
        mem.config(Scope::Home).unwrap(),
        "schema_version: 9\neditor:\n  command: 'true'\n"
    );
}

#[test]
fn test_home_writes_the_user_global_config_and_not_the_projects() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "qwen", &big(), true, false)
        .result
        .unwrap();
    assert!(mem
        .config(Scope::Home)
        .unwrap()
        .contains("qwen:\n      model: o1\n      type: mock\n"));
    assert!(mem.config(Scope::Home).unwrap().contains("editor:"));
    assert_eq!(mem.config(Scope::Project).unwrap(), CONFIG);
}

// llm.feature: Create refuses a label that already resolves, including a built-in backend
#[test]
fn test_create_refuses_a_label_that_resolves_and_edit_one_that_does_not() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    let before = mem.config(Scope::Project);
    assert!(err(library.llm_write(&plain(), "big", &big(), false, false)).contains("LlmExists"));
    assert!(
        err(library.llm_write(&plain(), "claude-code", &big(), false, false)).contains("LlmExists")
    );
    assert!(
        err(library.llm_write(&plain(), "nosuchlabel", &big(), false, true)).contains("LlmMissing")
    );
    assert_eq!(mem.config(Scope::Project), before);
}

// llm.feature: An edit naming only --model leaves the backend type intact
#[test]
fn test_an_edit_changes_what_it_names_and_a_built_in_becomes_an_explicit_entry() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    let done = library
        .llm_write(
            &plain(),
            "big",
            &LlmRequest {
                model: Some("o1-pro".to_string()),
                ..LlmRequest::default()
            },
            false,
            true,
        )
        .result
        .unwrap();
    assert_eq!(
        (done.kind.as_str(), done.model.as_str()),
        ("mock", "o1-pro")
    );
    assert!(done.edited);
    let built_in = library
        .llm_write(
            &plain(),
            "claude-code",
            &LlmRequest {
                model: Some("sonnet".to_string()),
                ..LlmRequest::default()
            },
            false,
            true,
        )
        .result
        .unwrap();
    assert_eq!(built_in.model, "sonnet");
    assert!(mem
        .config(Scope::Project)
        .unwrap()
        .contains("    claude-code:\n      model: sonnet\n"));
}

// llm.feature: Setting the permission posture records it in the project config
#[test]
fn test_a_permission_mode_is_checked_against_the_labels_engine_and_written() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    let set = |mode: &str| LlmRequest {
        permissions: Some(mode.to_string()),
        ..LlmRequest::default()
    };
    let done = library
        .llm_write(&plain(), "big", &set("bypass"), false, true)
        .result
        .unwrap();
    assert_eq!(done.permissions, serde_json::json!({"mode": "bypass"}));
    assert_eq!(done.permissions_line, "mode=bypass");
    assert!(mem
        .config(Scope::Project)
        .unwrap()
        .contains("      permissions:\n        mode: bypass\n"));
    let before = mem.config(Scope::Project);
    assert!(
        err(library.llm_write(&plain(), "big", &set("acceptEdits"), false, true))
            .contains("Permissions")
    );
    assert_eq!(mem.config(Scope::Project), before);
    let cleared = library
        .llm_write(&plain(), "big", &set(""), false, true)
        .result
        .unwrap();
    assert_eq!(cleared.permissions, serde_json::json!({}));
    assert!(!mem.config(Scope::Project).unwrap().contains("permissions"));
}

#[test]
fn test_a_type_nobody_knows_is_refused_and_an_empty_one_clears_it() {
    let (library, mem) = world();
    let bad = LlmRequest {
        kind: Some("nosuch".to_string()),
        ..LlmRequest::default()
    };
    assert!(err(library.llm_write(&plain(), "x", &bad, false, false)).contains("UnknownLlmType"));
    assert_eq!(mem.writes(), 0);
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    let cleared = library
        .llm_write(
            &plain(),
            "big",
            &LlmRequest {
                kind: Some(String::new()),
                ..LlmRequest::default()
            },
            false,
            true,
        )
        .result
        .unwrap();
    // The schema judges an entry by its type: with none given it is the default's.
    assert_eq!(cleared.kind, "claude-code");
}

// operations.applyLLMFields: --set reads YAML, checks the schema, removes on empty
#[test]
fn test_set_reads_yaml_nests_by_dotted_key_and_is_checked_against_the_schema() {
    let (library, mem) = world();
    let request = LlmRequest {
        kind: Some("opencode".to_string()),
        model: Some("qwen3.8-27b".to_string()),
        set: vec![
            "provider.output_limit=32768".to_string(),
            "context_window=131072".to_string(),
            "provider.base_url=http://192.168.1.20:8081/v1".to_string(),
        ],
        ..LlmRequest::default()
    };
    library
        .llm_write(&plain(), "qwen", &request, true, false)
        .result
        .unwrap();
    let home = mem.config(Scope::Home).unwrap();
    assert!(home.contains("      context_window: 131072\n"), "{home}");
    assert!(
        home.contains("      provider:\n        base_url: http://192.168.1.20:8081/v1\n"),
        "{home}"
    );
    // An empty value removes the key, and its emptied parent.
    let clear = LlmRequest {
        set: vec![
            "provider.base_url=".to_string(),
            "provider.output_limit=".to_string(),
        ],
        ..LlmRequest::default()
    };
    library
        .llm_write(&plain(), "qwen", &clear, true, true)
        .result
        .unwrap();
    assert!(!mem.config(Scope::Home).unwrap().contains("provider"));
    let before = mem.config(Scope::Home);
    for bad in [
        "no_such_key=1",
        "context_window=[1]",
        "type=mock",
        "novalue",
        "=x",
    ] {
        let request = LlmRequest {
            set: vec![bad.to_string()],
            ..LlmRequest::default()
        };
        assert!(
            err(library.llm_write(&plain(), "qwen", &request, true, true)).contains("LlmField"),
            "{bad}"
        );
    }
    assert_eq!(mem.config(Scope::Home), before);
}

// llm.feature: Setting the default moves the marker in the listing
#[test]
fn test_the_default_is_read_and_set_by_the_labels_the_project_knows() {
    let (library, mem) = world();
    let LlmDefault::Shown(shown) = library.llm_default(&plain(), None).result.unwrap() else {
        panic!("shown")
    };
    assert_eq!(shown.default, "claude-code");
    let LlmDefault::Set(set) = library
        .llm_default(&plain(), Some("claude-fast"))
        .result
        .unwrap()
    else {
        panic!("set")
    };
    assert_eq!(
        (set.status.as_str(), set.name.as_str()),
        ("set", "claude-fast")
    );
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 9\nllm:\n  defaults:\n    primary: claude-fast\n"
    );
    let LlmDefault::Set(again) = library
        .llm_default(&plain(), Some("claude-fast"))
        .result
        .unwrap()
    else {
        panic!("set")
    };
    assert_eq!(again.status, "unchanged");
    let LlmDefault::Shown(now) = library.llm_default(&plain(), None).result.unwrap() else {
        panic!("shown")
    };
    assert_eq!(now.default, "claude-fast");
    let listed = library.llm_list(&plain()).result.unwrap();
    assert!(listed.iter().any(|r| r.label == "claude-fast" && r.default));
}

// llm.feature: An unknown label is refused, and the old default stands
#[test]
fn test_an_unknown_default_is_refused_naming_what_is_usable() {
    let (library, mem) = world();
    let said = err(library.llm_default(&plain(), Some("nosuchengine")));
    assert!(
        said.contains("UnknownLlm") && said.contains("claude-code"),
        "{said}"
    );
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_the_default_keeps_the_labels_beside_it() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    library.llm_default(&plain(), Some("big")).result.unwrap();
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 9\nllm:\n  configs:\n    big:\n      model: o1\n      type: mock\n  defaults:\n    primary: big\n"
    );
}

// llm.feature: Bare remove reports and destroys nothing / --yes takes the entry
#[test]
fn test_remove_previews_then_takes_exactly_that_entry() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    library
        .llm_write(&plain(), "small", &big(), false, false)
        .result
        .unwrap();
    let before = mem.config(Scope::Project);
    let Removal::Preview(p) = library
        .llm_remove(&plain(), "big", false, false)
        .result
        .unwrap()
    else {
        panic!("a preview")
    };
    assert_eq!(
        (p.target.as_str(), p.apply.as_str()),
        ("llm \"big\"", "ctxloom llm remove big --yes")
    );
    assert_eq!(mem.config(Scope::Project), before);
    let Removal::Done(done) = library
        .llm_remove(&plain(), "big", false, true)
        .result
        .unwrap()
    else {
        panic!("done")
    };
    assert_eq!(
        (done.status.as_str(), done.label.as_str()),
        ("removed", "big")
    );
    let text = mem.config(Scope::Project).unwrap();
    assert!(!text.contains("big") && text.contains("small:"), "{text}");
    library
        .llm_remove(&plain(), "small", false, true)
        .result
        .unwrap();
    assert_eq!(
        mem.config(Scope::Project).unwrap(),
        "schema_version: 9\n",
        "the emptied sections go too"
    );
}

#[test]
fn test_the_preview_with_home_names_the_flag_in_the_command() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "qwen", &big(), true, false)
        .result
        .unwrap();
    let Removal::Preview(p) = library
        .llm_remove(&plain(), "qwen", true, false)
        .result
        .unwrap()
    else {
        panic!("a preview")
    };
    assert_eq!(p.apply, "ctxloom llm remove qwen --yes --home");
    library
        .llm_remove(&plain(), "qwen", true, true)
        .result
        .unwrap();
    assert!(!mem.config(Scope::Home).unwrap().contains("qwen"));
}

// llm.feature: Removing a label config.yaml never declared is refused
#[test]
fn test_remove_of_a_label_no_config_declares_is_refused() {
    let (library, mem) = world();
    assert!(
        err(library.llm_remove(&plain(), "claude-code", false, false)).contains("LlmNotDefined")
    );
    assert!(
        err(library.llm_remove(&plain(), "claude-code", false, true)).contains("LlmNotInConfig")
    );
    assert!(err(library.llm_remove(&plain(), "nosuch", false, true)).contains("LlmNotInConfig"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_removing_the_default_label_says_so() {
    let (library, mem) = world();
    library
        .llm_write(&plain(), "big", &big(), false, false)
        .result
        .unwrap();
    library.llm_default(&plain(), Some("big")).result.unwrap();
    let out = library.llm_remove(&plain(), "big", false, true);
    assert!(out.result.is_ok());
    assert!(
        out.notices
            .iter()
            .any(|n| format!("{n:?}").contains("llm.defaults.primary")),
        "{:?}",
        out.notices
    );
    let _ = mem;
}
