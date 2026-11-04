//! The setup commands' shape and their words. The commands' behaviour is the
//! application's (`setup-application`) and the binary's acceptance tests'.

use clap::Command;

use super::*;

fn parse(args: &[&str]) -> ArgMatches {
    let mut line = vec!["ctxloom"];
    line.extend(args);
    nouns()
        .into_iter()
        .fold(Command::new("ctxloom"), Command::subcommand)
        .try_get_matches_from(line)
        .expect("parses")
}

#[test]
fn test_the_nouns_are_manage_materialize_and_a_hidden_util() {
    let nouns = nouns();
    let names: Vec<&str> = nouns.iter().map(Command::get_name).collect();
    assert_eq!(names, vec!["manage", "materialize", "util"]);
    assert!(nouns[2].is_hide_set());
}

#[test]
fn test_materialize_takes_profiles_and_repeatable_backends_and_surfaces() {
    let m = parse(&[
        "materialize",
        "dev",
        "ops",
        "--backend",
        "mock",
        "--backend",
        "claude-code",
        "--surface",
        "context",
        "--surface",
        "hooks",
        "--target",
        "out",
        "--release",
        "-y",
        "-n",
    ]);
    let (_, args) = m.subcommand().expect("materialize");
    assert_eq!(many(args, "profiles"), vec!["dev", "ops"]);
    assert_eq!(many(args, "backend"), vec!["mock", "claude-code"]);
    assert_eq!(many(args, "surface"), vec!["context", "hooks"]);
    assert_eq!(one(args, "target").as_deref(), Some("out"));
    assert!(flag(args, "release") && flag(args, "yes") && flag(args, "dry-run"));
    assert!(!flag(args, "force") && !flag(args, "diff"));
}

#[test]
fn test_manage_has_its_verbs_and_hooks_list_takes_an_event_and_profiles() {
    let m = parse(&[
        "manage",
        "hooks",
        "list",
        "--event",
        "pre_tool",
        "--profile",
        "a",
        "--profile",
        "b",
    ]);
    let (_, manage) = m.subcommand().expect("manage");
    let (_, hooks) = manage.subcommand().expect("hooks");
    let (verb, list) = hooks.subcommand().expect("list");
    assert_eq!(verb, "list");
    assert_eq!(one(list, "event").as_deref(), Some("pre_tool"));
    assert_eq!(many(list, "profile"), vec!["a", "b"]);
    for line in [
        &[
            "manage",
            "install",
            "--engine",
            "mock",
            "--no-pull",
            "--print",
        ][..],
        &["manage", "uninstall"],
        &["manage", "check"],
        &["manage", "statusline", "install"],
        &["manage", "statusline", "uninstall"],
        &["manage", "gitignore", "install"],
        &[
            "util",
            "config-write",
            "--file",
            "/x.json",
            "--filetype",
            "json",
        ],
    ] {
        parse(line);
    }
}

#[test]
fn test_config_write_requires_its_file() {
    let result = nouns()
        .into_iter()
        .fold(Command::new("ctxloom"), Command::subcommand)
        .try_get_matches_from(["ctxloom", "util", "config-write"]);
    assert!(result.is_err());
}

#[test]
fn test_a_failure_carries_the_exit_code_of_the_error() {
    let refused = Failure::from(SetupError::Request {
        detail: "d".to_string(),
    });
    assert_eq!(refused.exit, 2);
    let fatal = Failure::from(SetupError::Refused {
        findings: String::new(),
    });
    assert_eq!(fatal.exit, 3);
}

fn words(f: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> String {
    let mut out = Vec::new();
    f(&mut out).expect("written");
    String::from_utf8(out).expect("text")
}

#[test]
fn test_the_check_report_reads_as_go_prints_it() {
    let report = setup_application::CheckReport {
        work_dir: "/p".into(),
        manage_statusline: true,
        backends: vec![
            setup_application::BackendStatus {
                backend: "claude-code".into(),
                settings_exists: true,
                hooks_present: true,
                status_line: true,
                mcp_present: false,
            },
            setup_application::BackendStatus {
                backend: "mock".into(),
                settings_exists: false,
                hooks_present: false,
                status_line: false,
                mcp_present: false,
            },
        ],
        surfaces: vec![setup_application::SurfaceCurrency {
            route: "context".into(),
            backend: "claude-code".into(),
            status: "materialized".into(),
        }],
    };
    let text = words(|w| text::check(w, &report));
    assert!(text.starts_with("Project: /p\nStatusline (HUD): true\n\n"));
    assert!(text.contains("  claude-code: hooks=true statusline=true mcp=false\n"));
    assert!(text.contains("  mock: not configured\n"));
    assert!(text.contains("Materialized surfaces:\n  context (claude-code): materialized\n"));
}

#[test]
fn test_the_hook_list_names_each_hooks_origin_and_empty_events() {
    let list = setup_application::HooksList {
        events: vec![
            setup_application::HooksEvent {
                event: "pre_tool".into(),
                hooks: vec![setup_application::HookRow {
                    position: 1,
                    declared: 1,
                    command: "ctxloom".into(),
                    args: vec!["hook".into(), "x".into()],
                    prompt: String::new(),
                    kind: "command".into(),
                    source_kind: "bundle".into(),
                    source: "ctxloom+local:hooked".into(),
                }],
            },
            setup_application::HooksEvent {
                event: "turn_end".into(),
                hooks: vec![],
            },
        ],
    };
    let text = words(|w| text::hooks(w, &list));
    assert!(text.contains("pre_tool\n  1. ctxloom hook x"));
    assert!(text.contains("[bundle ctxloom+local:hooked]"));
    assert!(text.contains("turn_end\n  (none)\n"));
}
