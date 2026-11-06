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
    assert!(!flag(args, "force") && one(args, "diff").is_none());
}

#[test]
fn test_diff_takes_a_file_not_a_switch() {
    let m = parse(&["materialize", "dev", "--diff", "../bob/CLAUDE.md"]);
    let (_, args) = m.subcommand().expect("materialize");
    assert_eq!(one(args, "diff").as_deref(), Some("../bob/CLAUDE.md"));
    assert_eq!(many(args, "profiles"), vec!["dev"]);
    // A bare --diff is a usage error: there is no file to compare against.
    let bare = nouns()
        .into_iter()
        .fold(Command::new("ctxloom"), Command::subcommand)
        .try_get_matches_from(["ctxloom", "materialize", "--diff"]);
    assert!(bare.is_err());
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
    let refused = failed(SetupError::Request {
        detail: "d".to_string(),
    });
    assert_eq!(refused.exit, 2);
    let fatal = failed(SetupError::Refused {
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

// ------------------------------------------------------------ the commands run

mod commands_run {
    use std::sync::Arc;

    use angzarr_presentation::Catalog;
    use surface_ports::ForeignFiles;

    use super::*;
    use crate::world::{world, World, CONFIG};

    struct Ran {
        exit: Result<u8, Failure>,
        out: String,
        err: String,
        world: World,
    }

    fn catalog() -> Catalog {
        Catalog::en_us(&[
            library_application::EN_US,
            setup_application::EN_US,
            surface_ports::EN_US,
        ])
        .expect("a catalog")
    }

    fn go_to(line: &[&str], format: Format, out: &mut dyn Write) -> (Ran, ArgMatches) {
        let world = world(CONFIG);
        let matches = parse(line);
        let noun = matches.subcommand_name().expect("a noun").to_string();
        let call = Call {
            matches: &matches,
            explicit_format: Some(format),
            default_format: Format::Json,
            stdout_is_terminal: false,
            globals: library_application::Flags {
                degraded: false,
                no_companions: false,
                config_set: Vec::new(),
            },
        };
        let mut err = Vec::new();
        let exit = run(&world.setup, &noun, &call, &catalog(), out, &mut err);
        let ran = Ran {
            exit,
            out: String::new(),
            err: String::from_utf8(err).unwrap(),
            world,
        };
        (ran, matches)
    }

    fn go(line: &[&str], format: Format) -> Ran {
        let mut out = Vec::new();
        let (mut ran, _) = go_to(line, format, &mut out);
        ran.out = String::from_utf8(out).unwrap();
        ran
    }

    #[test]
    fn a_noun_the_line_does_not_carry_is_refused_with_exit_two() {
        let world = world(CONFIG);
        let matches = parse(&["manage", "check"]);
        let call = Call {
            matches: &matches,
            explicit_format: None,
            default_format: Format::Text,
            stdout_is_terminal: false,
            globals: library_application::Flags {
                degraded: false,
                no_companions: false,
                config_set: Vec::new(),
            },
        };
        let failure = run(
            &world.setup,
            "materialize",
            &call,
            &catalog(),
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect_err("refused");
        assert_eq!(failure.exit, 2);
        assert!(
            failure
                .cause
                .args()
                .iter()
                .any(|(_, a)| format!("{a:?}").contains("no materialize command")),
            "{:?}",
            failure.cause.args()
        );
    }

    #[test]
    fn the_format_is_the_terminals_text_unless_one_was_asked_for() {
        let world = world(CONFIG);
        let matches = parse(&["manage", "check"]);
        let mut call = Call {
            matches: &matches,
            explicit_format: None,
            default_format: Format::Json,
            stdout_is_terminal: true,
            globals: library_application::Flags {
                degraded: false,
                no_companions: false,
                config_set: Vec::new(),
            },
        };
        let mut out = Vec::new();
        run(
            &world.setup,
            "manage",
            &call,
            &catalog(),
            &mut out,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().starts_with("Project: "));
        call.stdout_is_terminal = false;
        let mut out = Vec::new();
        run(
            &world.setup,
            "manage",
            &call,
            &catalog(),
            &mut out,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(String::from_utf8(out).unwrap().starts_with('{'));
    }

    #[test]
    fn manage_check_and_a_bare_hooks_report_the_project() {
        for line in [&["manage", "check"][..], &["manage", "hooks"]] {
            let ran = go(line, Format::Text);
            assert_eq!(ran.exit.unwrap(), 0, "{line:?}");
            assert!(
                ran.out.starts_with("Project: /p\n"),
                "{line:?}: {}",
                ran.out
            );
        }
        let json = go(&["manage", "check"], Format::Json);
        assert!(json.out.contains("\"work_dir\": \"/p\""), "{}", json.out);
    }

    #[test]
    fn manage_hooks_list_lists_the_events() {
        let ran = go(&["manage", "hooks", "list"], Format::Text);
        assert_eq!(ran.exit.unwrap(), 0);
        assert!(ran.out.contains("(none)"), "{}", ran.out);
        assert!(!ran.out.contains("Project:"), "{}", ran.out);
    }

    #[test]
    fn manage_install_print_plans_and_a_plain_install_installs() {
        let plan = go(&["manage", "install", "--print"], Format::Text);
        assert_eq!(plan.exit.unwrap(), 0);
        assert!(
            plan.out.starts_with("manage install would:\n"),
            "{}",
            plan.out
        );
        assert!(plan.world.mem.scaffold_paths().is_empty());
        let done = go(&["manage", "install", "--no-pull"], Format::Text);
        assert_eq!(done.exit.unwrap(), 0);
        assert!(
            done.out.contains("Initialized ctxloom directory: "),
            "{}",
            done.out
        );
        assert!(!done.out.contains("would:"), "{}", done.out);
        assert!(done.world.mem.scaffold_text("config.yaml").is_some());
    }

    #[test]
    fn manage_uninstall_names_what_it_removed() {
        let ran = go(&["manage", "uninstall"], Format::Text);
        assert_eq!(ran.exit.unwrap(), 0);
        assert!(
            ran.out.starts_with("Removed ctxloom harness from: "),
            "{}",
            ran.out
        );
    }

    #[test]
    fn manage_statusline_install_and_uninstall_say_the_new_state() {
        let on = go(&["manage", "statusline", "install"], Format::Text);
        assert_eq!(on.exit.unwrap(), 0);
        assert_eq!(
            on.out,
            "ctxloom HUD statusline: enabled\nRun 'ctxloom run' or 'ctxloom materialize' to apply the change.\n"
        );
        let off = go(&["manage", "statusline", "uninstall"], Format::Text);
        assert!(
            off.out.starts_with("ctxloom HUD statusline: disabled\n"),
            "{}",
            off.out
        );
        let json = go(&["manage", "statusline", "install"], Format::Json);
        assert!(json.out.contains("\"status\": \"enabled\""), "{}", json.out);
        assert!(json.out.contains("\"statusline\": true"), "{}", json.out);
        let bare = go(&["manage", "statusline"], Format::Text);
        assert_eq!((bare.exit.unwrap(), bare.out.as_str()), (0, ""));
    }

    #[test]
    fn manage_gitignore_install_writes_the_rules_and_a_bare_gitignore_does_nothing() {
        let ran = go(&["manage", "gitignore", "install"], Format::Text);
        assert_eq!(ran.exit.unwrap(), 0);
        assert!(ran.out.starts_with("Wrote "), "{}", ran.out);
        assert!(ran.world.mem.scaffold_text(".gitignore").is_some());
        let bare = go(&["manage", "gitignore"], Format::Text);
        assert_eq!((bare.exit.unwrap(), bare.out.as_str()), (0, ""));
        assert!(bare.world.mem.scaffold_text(".gitignore").is_none());
    }

    #[test]
    fn a_bare_manage_does_nothing_and_succeeds() {
        let ran = go(&["manage"], Format::Text);
        assert_eq!(
            (ran.exit.unwrap(), ran.out.as_str(), ran.err.as_str()),
            (0, "", "")
        );
    }

    #[test]
    fn materialize_delivers_and_dry_run_only_plans() {
        let delivered = go(
            &[
                "materialize",
                "--target",
                "/out",
                "--backend",
                "mock",
                "dev",
            ],
            Format::Text,
        );
        assert_eq!(delivered.exit.unwrap(), 0);
        assert!(
            delivered.out.starts_with("Target: /out\n"),
            "{}",
            delivered.out
        );
        assert!(delivered.out.contains("mock: wrote "), "{}", delivered.out);
        assert!(!delivered.world.store.state().claims.is_empty());
        let planned = go(
            &[
                "materialize",
                "--target",
                "/out",
                "--backend",
                "mock",
                "dev",
                "--dry-run",
            ],
            Format::Text,
        );
        assert_eq!(planned.exit.unwrap(), 0);
        assert!(
            planned.world.store.state().claims.is_empty(),
            "--dry-run must not deliver"
        );
    }

    /// `materialize --diff FILE` over a world holding `files`.
    fn diff_go(line: &[&str], format: Format, files: &[(&str, &str)]) -> Ran {
        let world = world(CONFIG);
        for (path, text) in files {
            world
                .store
                .state()
                .files
                .insert(path.to_string(), text.as_bytes().to_vec());
        }
        let matches = parse(line);
        let call = Call {
            matches: &matches,
            explicit_format: Some(format),
            default_format: Format::Json,
            stdout_is_terminal: false,
            globals: library_application::Flags::default(),
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let exit = run(
            &world.setup,
            "materialize",
            &call,
            &catalog(),
            &mut out,
            &mut err,
        );
        Ran {
            exit,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
            world,
        }
    }

    #[test]
    fn diff_prints_the_unified_diff_and_writes_nothing() {
        let ran = diff_go(
            &["materialize", "--diff", "bob.md", "--backend", "mock"],
            Format::Text,
            &[("/p/bob.md", "BOB-ONLY-LINE\n")],
        );
        assert_eq!(ran.exit.unwrap(), 0);
        assert!(
            ran.out.starts_with(
                "Comparing the default agent's materialized context with bob.md \u{2014} content present on one side and absent on the other:\n--- bob.md\n+++ the default agent (materialized here)\n@@ -1 +1,"
            ),
            "{}",
            ran.out
        );
        assert!(ran.out.contains("\n-BOB-ONLY-LINE\n"), "{}", ran.out);
        assert!(ran.out.contains("\n+FRAGMENT-BODY-testing"), "{}", ran.out);
        let state = ran.world.store.state();
        assert!(state.claims.is_empty());
        assert!(state.log.is_empty(), "{:?}", state.log);
    }

    #[test]
    fn diff_over_the_same_context_says_there_is_no_difference() {
        let first = diff_go(
            &["materialize", "--diff", "bob.md", "--backend", "mock"],
            Format::Json,
            &[("/p/bob.md", "x")],
        );
        let first: serde_json::Value = serde_json::from_str(&first.out).unwrap();
        assert_eq!(first["identical"], false);
        // Feed the context back: the diff's "+" lines are the context.
        let context: String = first["diff"]
            .as_str()
            .unwrap()
            .lines()
            .filter_map(|l| l.strip_prefix('+').filter(|_| !l.starts_with("+++")))
            .map(|l| format!("{l}\n"))
            .collect();
        let same = diff_go(
            &[
                "materialize",
                "dev",
                "--diff",
                "bob.md",
                "--backend",
                "mock",
            ],
            Format::Text,
            &[("/p/bob.md", &context)],
        );
        assert_eq!(same.exit.unwrap(), 0);
        assert_eq!(
            same.out,
            "dev's materialized context matches bob.md: no difference.\n"
        );
    }

    #[test]
    fn diff_json_is_the_flat_object() {
        let ran = diff_go(
            &[
                "materialize",
                "dev",
                "--diff",
                "bob.md",
                "--backend",
                "mock",
            ],
            Format::Json,
            &[("/p/bob.md", "x")],
        );
        assert_eq!(ran.exit.unwrap(), 0);
        let json: serde_json::Value = serde_json::from_str(&ran.out).unwrap();
        assert_eq!(json["profiles"], serde_json::json!(["dev"]));
        assert_eq!(json["compared_to"], "bob.md");
        assert_eq!(json["identical"], false);
        assert!(json["diff"]
            .as_str()
            .unwrap()
            .starts_with("--- bob.md\n+++ dev (materialized here)\n"));
    }

    #[test]
    fn diff_over_a_missing_file_fails_naming_it() {
        let ran = diff_go(
            &["materialize", "--diff", "nowhere.md", "--backend", "mock"],
            Format::Text,
            &[],
        );
        let failure = ran.exit.expect_err("a missing file");
        assert_eq!(failure.exit, 1);
        assert_eq!(failure.cause.code(), "setup-application-diff-file-missing");
        assert_eq!(ran.out, "");
    }

    #[test]
    fn a_refused_delivery_fails_the_command_after_the_report() {
        let world = world(CONFIG);
        world.store.state().refuse_path = Some("mock.md".to_string());
        let matches = parse(&[
            "materialize",
            "--target",
            "/out",
            "--backend",
            "mock",
            "dev",
        ]);
        let call = Call {
            matches: &matches,
            explicit_format: Some(Format::Text),
            default_format: Format::Text,
            stdout_is_terminal: false,
            globals: library_application::Flags {
                degraded: false,
                no_companions: false,
                config_set: Vec::new(),
            },
        };
        let mut out = Vec::new();
        let failure = run(
            &world.setup,
            "materialize",
            &call,
            &catalog(),
            &mut out,
            &mut Vec::new(),
        )
        .expect_err("fails");
        assert_eq!(failure.exit, 1);
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.contains("error: "),
            "the report is still written: {out}"
        );
    }

    #[test]
    fn an_unwritable_answer_is_a_refused_request() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let (ran, _) = go_to(&["manage", "check"], Format::Text, &mut Broken);
        let failure = ran.exit.expect_err("fails");
        assert_eq!(failure.exit, 2);
        let (ran, _) = go_to(&["manage", "check"], Format::Json, &mut Broken);
        assert!(ran.exit.is_err());
    }

    mod util_run {
        use super::*;

        #[derive(Default)]
        struct Files {
            files: std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
        }

        impl ForeignFiles for Files {
            fn read(&self, path: &str) -> Result<Option<Vec<u8>>, surface_ports::SurfaceError> {
                Ok(self.files.lock().unwrap().get(path).cloned())
            }
            fn write(&self, path: &str, bytes: &[u8]) -> Result<(), surface_ports::SurfaceError> {
                self.files
                    .lock()
                    .unwrap()
                    .insert(path.into(), bytes.to_vec());
                Ok(())
            }
            fn record(&self, _: &str) -> Result<Option<String>, surface_ports::SurfaceError> {
                Ok(None)
            }
            fn write_record(
                &self,
                path: &str,
                _: &str,
            ) -> Result<String, surface_ports::SurfaceError> {
                Ok(format!("records/{path}"))
            }
        }

        fn util_with(
            line: &[&str],
            patch: &str,
            format: Format,
        ) -> (Result<u8, Failure>, String, Arc<Files>) {
            let files = Arc::new(Files::default());
            let setup = world(CONFIG).setup.with_foreign(files.clone());
            let matches = parse(line);
            let (_, sub) = matches.subcommand().expect("util");
            let cx = Context {
                setup: &setup,
                flags: Flags::default(),
                format,
                catalog: &catalog(),
            };
            let mut out = Vec::new();
            let exit = util(&cx, sub, &mut patch.as_bytes(), &mut out);
            (exit, String::from_utf8(out).unwrap(), files)
        }

        const WRITE: &[&str] = &["util", "config-write", "--file", "/x.json"];

        #[test]
        fn config_write_merges_the_patch_from_the_input() {
            let (exit, out, files) = util_with(WRITE, r#"{"a": {"b": 1}}"#, Format::Text);
            assert_eq!(exit.unwrap(), 0);
            assert_eq!(out, "Merged a into /x.json (created)\n");
            assert!(files.files.lock().unwrap().contains_key("/x.json"));
            let (_, json, _) = util_with(WRITE, r#"{"a": 1}"#, Format::Json);
            assert!(json.contains("\"file\": \"/x.json\""), "{json}");
        }

        #[test]
        fn config_write_refuses_an_empty_patch_and_an_unreadable_input() {
            let (exit, out, _) = util_with(WRITE, "", Format::Text);
            assert!(exit.is_err());
            assert_eq!(out, "");
            struct Unreadable;
            impl Read for Unreadable {
                fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                    Err(io::Error::other("closed"))
                }
            }
            let setup = world(CONFIG).setup;
            let matches = parse(WRITE);
            let (_, sub) = matches.subcommand().unwrap();
            let cx = Context {
                setup: &setup,
                flags: Flags::default(),
                format: Format::Text,
                catalog: &catalog(),
            };
            let failure = util(&cx, sub, &mut Unreadable, &mut Vec::new()).expect_err("fails");
            assert_eq!(failure.cause.code(), "setup-application-config-write");
        }

        #[test]
        fn a_util_without_its_verb_reads_nothing_and_succeeds() {
            let matches = parse(&["util"]);
            let (_, sub) = matches.subcommand().unwrap();
            let setup = world(CONFIG).setup;
            let cx = Context {
                setup: &setup,
                flags: Flags::default(),
                format: Format::Text,
                catalog: &catalog(),
            };
            struct Never;
            impl Read for Never {
                fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                    panic!("nothing to read")
                }
            }
            assert_eq!(util(&cx, sub, &mut Never, &mut Vec::new()).unwrap(), 0);
        }
    }
}
