//! A profile, an MCP server and a skill as the YAML Go marshals them.

use std::collections::BTreeMap;

use angzarr_yaml::parse;
use library::model::{McpServer, Skill};
use library::profile::{FragmentRef, HooksConfig, Profile};

use super::{hooks, mcp, profile, skill};

fn doc(text: &str) -> angzarr_yaml::Value {
    parse(text).expect("a document")
}

// ProfileEntry marshalling: fields in record order, empty ones omitted,
// a fragment with a priority a map and one without a bare name.
#[test]
fn test_a_profile_is_marshalled_in_record_order_without_its_empty_fields() {
    let full = Profile {
        name: "dev".to_string(),
        description: "d".to_string(),
        parents: vec!["base".to_string()],
        tags: vec!["t".to_string()],
        select_tags: vec!["s".to_string()],
        llm: "claude-code".to_string(),
        bundles: vec!["b".to_string()],
        commands: vec!["c".to_string()],
        skills: vec!["k".to_string()],
        fragments: vec![
            FragmentRef {
                name: "plain".to_string(),
                priority: 0,
            },
            FragmentRef {
                name: "weighted".to_string(),
                priority: 5,
            },
        ],
        bundle_items: vec!["bi".to_string()],
        hooks: HooksConfig {
            unified: BTreeMap::from([("pre_tool".to_string(), vec![doc("tool: shell\n")])]),
            ext: BTreeMap::new(),
        },
        variables: BTreeMap::from([("v".to_string(), "1".to_string())]),
        exclude_fragments: vec!["ef".to_string()],
        exclude_mcp: vec!["em".to_string()],
        deny_tools: vec!["dt".to_string()],
    };
    assert_eq!(
        profile(&full),
        "description: d\nparents:\n  - base\ntags:\n  - t\nselect_tags:\n  - s\nllm: claude-code\nbundles:\n  - b\ncommands:\n  - c\nskills:\n  - k\nfragments:\n  - plain\n  - name: weighted\n    priority: 5\nbundle_items:\n  - bi\nhooks:\n  unified:\n    pre_tool:\n      - tool: shell\nvariables:\n  v: \"1\"\nexclude_fragments:\n  - ef\nexclude_mcp:\n  - em\ndeny_tools:\n  - dt\n"
    );
    let bare = Profile {
        description: "only".to_string(),
        ..Profile::default()
    };
    assert_eq!(profile(&bare), "description: only\n");
}

// Hooks: events with no hooks, and engines with none, are left out.
#[test]
fn test_hooks_leave_out_empty_events_and_empty_engines() {
    let config = HooksConfig {
        unified: BTreeMap::from([
            ("pre_tool".to_string(), vec![doc("tool: shell\n")]),
            ("post_tool".to_string(), Vec::new()),
        ]),
        ext: BTreeMap::from([
            (
                "claude-code".to_string(),
                BTreeMap::from([("Stop".to_string(), vec![doc("run: x\n")])]),
            ),
            (
                "codex".to_string(),
                BTreeMap::from([("Idle".to_string(), Vec::new())]),
            ),
        ]),
    };
    let rendered = library::config::typed::to_yaml(&hooks(&config));
    assert_eq!(
        rendered,
        "unified:\n  pre_tool:\n    - tool: shell\next:\n  claude-code:\n    Stop:\n      - run: x\n  codex: {}\n"
    );
    // Only empty hooks: no section at all.
    let hollow = HooksConfig {
        unified: BTreeMap::from([("pre_tool".to_string(), Vec::new())]),
        ext: BTreeMap::from([(
            "codex".to_string(),
            BTreeMap::from([("Idle".to_string(), Vec::new())]),
        )]),
    };
    assert_eq!(library::config::typed::to_yaml(&hooks(&hollow)), "{}\n");
}

// A fragment priority of zero is bare, any other is a map; a skill's exports
// keep the types they were written with.
#[test]
fn test_a_skill_entry_marshals_its_path_tags_notes_and_typed_exports() {
    let entry = Skill {
        path: "skills/x".to_string(),
        tags: vec!["t".to_string()],
        notes: "note".to_string(),
        exports: BTreeMap::from([(
            "claude-code".to_string(),
            doc("on: true\noff: false\nn: 4\nname: word\nnothing:\nlist: [1, a]\nz: 1\n"),
        )]),
        files: Vec::new(),
    };
    assert_eq!(
        skill(&entry),
        "path: skills/x\ntags:\n  - t\nnotes: note\nexports:\n  claude-code:\n    list:\n      - 1\n      - a\n    \"n\": 4\n    name: word\n    nothing: null\n    \"off\": false\n    \"on\": true\n    z: 1\n"
    );
    assert_eq!(skill(&Skill::default()), "{}\n");
    let only_path = Skill {
        path: "p".to_string(),
        ..Skill::default()
    };
    assert_eq!(skill(&only_path), "path: p\n");
}

// McpServer marshalling: command, args, env, url, headers, served_by, tags,
// notes, installation; env and headers as maps.
#[test]
fn test_an_mcp_server_marshals_every_field_it_sets() {
    let server = McpServer {
        command: "c".to_string(),
        args: vec!["a".to_string()],
        env: BTreeMap::from([("K".to_string(), "v".to_string())]),
        url: "u".to_string(),
        headers: BTreeMap::from([("H".to_string(), "x".to_string())]),
        served_by: "session-endpoint".to_string(),
        tags: vec!["t".to_string()],
        notes: "note".to_string(),
        installation: "i".to_string(),
    };
    assert_eq!(
        mcp(&server),
        "command: c\nargs:\n  - a\nenv:\n  K: v\nurl: u\nheaders:\n  H: x\nserved_by: session-endpoint\ntags:\n  - t\nnotes: note\ninstallation: i\n"
    );
    assert_eq!(mcp(&McpServer::default()), "{}\n");
}
