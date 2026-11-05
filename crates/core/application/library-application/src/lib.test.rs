//! The application's behaviour over in-memory ports. Each test names the Go
//! behaviour (a Gherkin scenario, a Go test, or a golden of
//! `crates/root/bin/ctxloom/fixtures/golden`) it holds the application to.

use crate::fakes::{plain, Setup, CONFIG, ENVELOPE};
use crate::{AppError, AssembleRequest, ItemKind, Notice, RunRequest, SearchRequest};

const DEMO: &[(&str, &str)] = &[
    ENVELOPE,
    (
        "fragments/golang.md",
        "---\ndescription: when editing Go code\ntags: [go]\n---\nGO-RULES\n",
    ),
    ("fragments/testing.md", "# testing\n\nTEST-BODY\n"),
    (
        "fragments/dist.md",
        "---\ntags: [distilled]\n---\nlong form\n",
    ),
    ("fragments/dist.distilled.md", "short form\n"),
    (
        "prompts/review.md",
        "---\ndescription: Review code\n---\nREVIEW-BODY\n",
    ),
    (
        "mcp/tree-sitter.yaml",
        "command: tree-sitter\nargs: [--stdio]\n",
    ),
    ("mcp/remote.yaml", "url: https://mcp.example.com/v1\n"),
];

const PROJECT: &[(&str, &str)] = &[
    ENVELOPE,
    (
        "profiles/dev.yaml",
        "description: dev profile\nparents: [base]\nbundles: [demo]\ntags: [team]\n",
    ),
    (
        "profiles/base.yaml",
        "description: base\nbundles: [demo]\nexclude_mcp: [remote]\n",
    ),
    ("profiles/broken.yaml", "bundles: [does-not-exist]\n"),
];

fn setup<'a>(layers: &'a [&'a str]) -> Setup<'a> {
    Setup {
        bundles: &[("demo", DEMO), ("project", PROJECT)],
        layers,
        ..Setup::default()
    }
}

fn ok<T>(outcome: crate::Outcome<T>) -> T {
    match outcome.result {
        Ok(value) => value,
        Err(e) => panic!("{e:?}"),
    }
}

// bundle.feature: the listing summarizes each bundle by version and content.
#[test]
fn test_the_bundle_listing_counts_what_each_bundle_holds() {
    let list = ok(setup(&[CONFIG]).library().bundle_list(&plain()));
    let demo = list.rows.iter().find(|r| r.name == "demo").unwrap();
    assert_eq!(
        (
            demo.fragment_count,
            demo.command_count,
            demo.mcp_count,
            demo.profile_count
        ),
        (3, 1, 2, 0)
    );
    assert_eq!(demo.reference, "ctxloom+local:demo");
    assert_eq!(demo.version, "1.0.0");
    let project = list.rows.iter().find(|r| r.name == "project").unwrap();
    assert_eq!(project.profile_count, 3);
}

// fault_tolerance.feature: a malformed bundle is skipped and named, and the
// listing goes on (golden bad-bundle-list-json).
#[test]
fn test_a_malformed_bundle_is_skipped_with_a_notice_and_the_listing_goes_on() {
    let bad: &[(&str, &str)] = &[("bundle.yaml", "schema_version: 5\nversion: 1.0.0\n")];
    let out = Setup {
        bundles: &[("good", DEMO), ("newer", bad)],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .library()
    .bundle_list(&plain());
    assert_eq!(out.notices.len(), 1);
    assert!(matches!(out.notices[0], Notice::BundleSkipped { .. }));
    let rows = out.result.unwrap().rows;
    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["good"]
    );
}

// bundle show of a bundle that is there and cannot be read says so; of one
// that is not there, "bundle not found" (golden bad-show-*).
#[test]
fn test_showing_an_unreadable_bundle_names_the_fault_and_a_missing_one_is_not_found() {
    let bad: &[(&str, &str)] = &[("bundle.yaml", "")];
    let library = Setup {
        bundles: &[("empty", bad)],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .library();
    let unreadable = library.bundle_show(&plain(), "empty").result.unwrap_err();
    assert!(matches!(unreadable, AppError::BundleUnreadable { .. }));
    let missing = library.bundle_show(&plain(), "nosuch").result.unwrap_err();
    assert!(matches!(missing, AppError::Cause(_)));
    assert_eq!(missing.exit_code(), 1);
}

// config.feature / fault_tolerance.feature: an unknown key is refused naming
// it; --degraded warns and carries on without it.
#[test]
fn test_an_unknown_config_key_is_refused_and_degraded_drops_it_with_a_warning() {
    let layers = ["schema_version: 8\nzzz: 1\n"];
    let library = setup(&layers).library();
    let refused = library.bundle_list(&plain()).result;
    match refused {
        Err(AppError::ConfigUnknownKey { violations }) => {
            assert!(violations.contains("unknown key `zzz`"), "{violations}");
        }
        other => panic!("{:?}", other.err()),
    }
    let degraded = crate::Flags {
        degraded: true,
        ..plain()
    };
    let out = library.bundle_list(&degraded);
    assert!(out.result.is_ok());
    assert!(out
        .notices
        .iter()
        .any(|n| matches!(n, Notice::Finding { text, .. } if text.contains("unknown key `zzz`"))));
}

// config_version_refusal_test.go: owner override of Go, D15, 2026-11-03: a
// layer of another generation (newer, older, unstamped, not a number) is
// refused, naming it, with exit 2.
#[test]
fn test_a_config_of_another_generation_is_refused() {
    for layer in [
        "schema_version: 9\neditor:\n  command: vim\n",
        "schema_version: 6\neditor:\n  command: vim\n",
        "editor:\n  command: vim\n",
        "schema_version: abc\n",
    ] {
        let layers = [layer];
        let refused = setup(&layers)
            .library()
            .config_show(&plain(), Some("editor"), false)
            .result
            .err()
            .unwrap_or_else(|| panic!("{layer:?} was read"));
        assert!(
            matches!(
                refused,
                AppError::ConfigNewer { .. }
                    | AppError::ConfigOlder { .. }
                    | AppError::ConfigVersionUnreadable { .. }
            ),
            "{layer:?}: {refused:?}"
        );
        assert_eq!(refused.exit_code(), 2);
    }
}

// merge.go dropEnginelessAgents: an agent naming neither an llm nor a profile
// is dropped with a warning.
#[test]
fn test_an_agent_bound_to_nothing_is_dropped_with_a_warning() {
    let layers = ["schema_version: 8\nagents:\n  ghost: {}\n  real:\n    llm: claude-code\n"];
    let out = setup(&layers).library().agent_list(&plain());
    assert_eq!(out.notices.len(), 1);
    let rows = out.result.unwrap();
    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["real"]
    );
}

// confload overlay: the environment overrides the files, --config-set the
// environment; a malformed --config-set is a notice.
#[test]
fn test_overrides_beat_the_files_and_a_malformed_flag_is_a_notice() {
    let layers = ["schema_version: 8\neditor:\n  command: vim\n"];
    let library = Setup {
        environ: &[("CTXLOOM_CONFIG_EDITOR_COMMAND", "nano")],
        ..setup(&layers)
    }
    .library();
    let from_env = ok(library.config_show(&plain(), Some("editor"), false));
    assert_eq!(from_env.json["command"], "nano");
    let flags = crate::Flags {
        config_set: vec!["editor.command=code".to_string(), "noequals".to_string()],
        ..plain()
    };
    let out = library.config_show(&flags, Some("editor"), false);
    assert_eq!(out.notices.len(), 1);
    assert_eq!(out.result.unwrap().json["command"], "code");
}

// config.feature: an unknown section is refused and the real ones named; a
// section that is not set shows its zero.
#[test]
fn test_config_sections_show_their_zero_and_an_unknown_one_is_refused() {
    let library = setup(&[CONFIG]).library();
    let unknown = library
        .config_show(&plain(), Some("nonsense"), false)
        .result;
    assert!(matches!(unknown, Err(AppError::UnknownSection { .. })));
    let delegation = ok(library.config_show(&plain(), Some("delegation"), false));
    assert_eq!(delegation.json, serde_json::json!({}));
    let companions = ok(library.config_show(&plain(), Some("companions"), false));
    assert_eq!(companions.json, serde_json::json!([]));
    let workspace = ok(library.config_show(&plain(), Some("workspace"), false));
    assert_eq!(workspace.json, serde_json::json!(""));
}

// command.feature: a command listing with a misspelled --bundle is refused.
#[test]
fn test_a_listing_of_a_bundle_nobody_has_is_refused_naming_the_bundles() {
    let library = setup(&[CONFIG]).library();
    match library
        .item_list(&plain(), ItemKind::Command, "nosuch")
        .result
    {
        Err(AppError::NoBundleNamed { known, .. }) => assert_eq!(known, "demo, project"),
        other => panic!("{:?}", other.err()),
    }
    let list = ok(library.item_list(&plain(), ItemKind::Fragment, "demo"));
    assert_eq!(list.rows.len(), 3);
    assert_eq!(list.rows[0].reference, "demo#fragments/dist");
}

// fault_tolerance.feature: an unknown fragment names what there is; a garbage
// reference says what it should have been; a command reference given to
// fragment show is a kind mismatch.
#[test]
fn test_showing_an_item_names_what_is_wrong_with_the_reference() {
    let library = setup(&[CONFIG]).library();
    match library
        .item_show(&plain(), ItemKind::Fragment, "demo#fragments/nope", false)
        .result
    {
        Err(AppError::ItemMissing { available, .. }) => {
            assert_eq!(available, "dist, golang, testing");
        }
        other => panic!("{:?}", other.err()),
    }
    let garbage = library.item_show(&plain(), ItemKind::Fragment, "garbage", false);
    assert!(matches!(garbage.result, Err(AppError::BadReference { .. })));
    let kind = library.item_show(&plain(), ItemKind::Fragment, "demo#commands/review", false);
    assert!(matches!(kind.result, Err(AppError::WrongKind { .. })));
    let distilled =
        ok(library.item_show(&plain(), ItemKind::Fragment, "demo#fragments/dist", true));
    assert!(distilled.distilled);
    assert_eq!(distilled.content.trim(), "short form");
    let raw = ok(library.item_show(&plain(), ItemKind::Fragment, "demo#fragments/dist", false));
    assert!(!raw.distilled);
}

// premise_test.go / fragment premises: only premised fragments, by qualified name.
#[test]
fn test_the_premises_list_only_premised_fragments() {
    let premises = ok(setup(&[CONFIG]).library().premises(&plain()));
    assert_eq!(premises.fragments.len(), 1);
    assert_eq!(premises.fragments[0].name, "demo#fragments/golang");
}

// skills.go: a package's mode is the file's execute bit; a skill whose
// SKILL.md has no frontmatter is withheld with a warning.
#[test]
fn test_a_skill_mode_is_the_files_execute_bit_and_a_headless_skill_is_withheld() {
    let reviewer: &[(&str, &str)] = &[
        ENVELOPE,
        (
            "skills/reviewer/SKILL.md",
            "---\nname: reviewer\ndescription: Reviews\n---\n\nbody\n",
        ),
        ("skills/reviewer/scripts/run.sh", "#!/bin/sh\n"),
        ("skills/headless/SKILL.md", "no frontmatter\n"),
    ];
    let library = Setup {
        bundles: &[("vault", reviewer)],
        layers: &[CONFIG],
        executable: &["skills/reviewer/scripts/run.sh"],
        ..Setup::default()
    }
    .library();
    let shown = ok(library.skill_show(&plain(), "vault#skills/reviewer"));
    let modes: Vec<(&str, &str)> = shown
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.mode.as_str()))
        .collect();
    assert_eq!(modes, [("SKILL.md", "0644"), ("scripts/run.sh", "0755")]);
    assert!(shown.files[0].sha256.starts_with("sha256:"));
    let list = library.skill_list(&plain(), "");
    assert!(list
        .notices
        .iter()
        .any(|n| matches!(n, Notice::Finding { text, .. } if text.contains("headless"))));
    assert_eq!(list.result.unwrap().rows.len(), 1);
    let by_name = ok(library.skill_show(&plain(), "reviewer"));
    assert_eq!(by_name.bundle, "vault");
    let missing = library.skill_show(&plain(), "vault#skills/none").result;
    assert!(matches!(missing, Err(AppError::SkillNotFound { .. })));
}

// profile.feature / profiles.go: the listing names the default agent's
// profiles; a profile that is not there is not found.
#[test]
fn test_the_profile_listing_marks_the_defaults_and_a_missing_profile_is_not_found() {
    let layers = ["schema_version: 8\nagents:\n  dev:\n    llm: claude-code\n    profiles: [dev]\ndefault_agent: dev\n"];
    let library = setup(&layers).library();
    let list = ok(library.profile_list(&plain()));
    let names: Vec<(&str, bool)> = list
        .rows
        .iter()
        .map(|r| (r.name.as_str(), r.default))
        .collect();
    assert_eq!(names, [("base", false), ("broken", false), ("dev", true)]);
    let shown = ok(library.profile_show(&plain(), "dev"));
    assert!(shown.default);
    assert_eq!(shown.bundle, "ctxloom+local:project");
    assert!(library.profile_show(&plain(), "nope").result.is_err());
}

// mcp.feature: the servers listed are the default agent's profiles' bundles',
// minus what a profile excludes.
#[test]
fn test_the_registered_servers_follow_the_default_agents_profiles_and_their_exclusions() {
    let layers = ["schema_version: 8\nagents:\n  dev:\n    llm: claude-code\n    profiles: [dev]\ndefault_agent: dev\n"];
    let library = setup(&layers).library();
    let list = ok(library.mcp_list(&plain()));
    assert_eq!(list.servers.len(), 1);
    assert_eq!(list.servers[0].name, "tree-sitter");
    assert_eq!(list.servers[0].source, "ctxloom+local:demo");
    assert!(library.mcp_show(&plain(), "remote").result.is_err());
    // With no default agent there is no scope, so no server.
    let none = ok(setup(&[CONFIG]).library().mcp_list(&plain()));
    assert!(none.servers.is_empty());
}

// llm.feature: the built-in engines, the default marked, a configured label
// told apart from one ctxloom supplied.
#[test]
fn test_the_llm_listing_marks_the_default_and_who_wrote_each_label() {
    let layers = ["schema_version: 8\nllm:\n  configs:\n    big:\n      type: mock\n      model: o1\n    claude-code:\n      type: claude-code\n      model: opus\n  defaults:\n    primary: claude-code\n"];
    let rows = ok(setup(&layers).library().llm_list(&plain()));
    let by = |label: &str| rows.iter().find(|r| r.label == label).unwrap();
    assert!(by("claude-code").default && by("claude-code").authored);
    assert!(by("big").authored && !by("big").default);
    assert!(!by("mock").authored);
    let bare = ok(setup(&[CONFIG]).library().llm_list(&plain()));
    let fast = bare.iter().find(|r| r.label == "claude-fast").unwrap();
    assert!(!fast.authored, "the shipped registry is not the user's");
}

// companion.feature: the binary is ctxloom-companion-<name>, found on PATH.
#[test]
fn test_a_companion_resolves_to_its_binary_on_the_path() {
    let layers = ["schema_version: 8\ncompanions: [acme, ltk]\n"];
    let rows = ok(Setup {
        path: &[("ctxloom-companion-acme", "/bin/ctxloom-companion-acme")],
        ..setup(&layers)
    }
    .library()
    .companion_list(&plain()));
    assert_eq!(
        (rows[0].bin.as_str(), rows[0].resolves),
        ("ctxloom-companion-acme", true)
    );
    assert_eq!(rows[0].path, "/bin/ctxloom-companion-acme");
    assert_eq!((rows[1].bin.as_str(), rows[1].resolves), ("ltk", false));
}

// search.feature: name matches rank before tag matches; a search needs a
// query or a tag; --type names its vocabulary.
#[test]
fn test_a_search_ranks_names_before_tags_and_refuses_what_it_cannot_answer() {
    let library = setup(&[CONFIG]).library();
    let ask = |query: &str, kind: &str| SearchRequest {
        query: query.to_string(),
        kind: kind.to_string(),
        local: true,
        ..SearchRequest::default()
    };
    let found = ok(library.search(&plain(), &ask("go", "")));
    assert_eq!(found.local[0].name, "golang");
    assert_eq!(found.local[0].r#match, "name");
    let by_tag = ok(library.search(&plain(), &ask("distilled", "fragment")));
    assert_eq!(by_tag.local[0].r#match, "tag");
    assert!(matches!(
        library.search(&plain(), &ask("", "")).result,
        Err(AppError::SearchNothing)
    ));
    assert!(matches!(
        library.search(&plain(), &ask("x", "widget")).result,
        Err(AppError::SearchType { .. })
    ));
    let remote = SearchRequest {
        query: "x".to_string(),
        remote: true,
        ..SearchRequest::default()
    };
    assert_eq!(
        library
            .search(&plain(), &remote)
            .result
            .unwrap_err()
            .exit_code(),
        2
    );
}

// run.feature: a dry run names the profiles, fragments and context it would
// carry; an agent and a profile cannot both decide; a tag matching nothing
// fails loudly.
#[test]
fn test_a_dry_run_carries_the_composed_context_and_refuses_what_run_refuses() {
    let layers = ["schema_version: 8\nagents:\n  dev:\n    llm: claude-fast\n    profiles: [dev]\n    runtime: host\ndefault_agent: dev\n"];
    let library = setup(&layers).library();
    let plan = ok(library.dry_run(&plain(), &RunRequest::default())).plan;
    assert_eq!(plan.llm, "claude-fast");
    assert_eq!(plan.backend, "claude-code");
    assert_eq!(plan.profiles, ["dev"]);
    assert_eq!(plan.runtime, "host");
    assert!(plan.context.contains("TEST-BODY"));
    assert!(
        !plan.context.contains("GO-RULES"),
        "a premised fragment is withheld"
    );
    assert_eq!(
        plan.tokens as usize,
        library::tokens::estimate(&plan.context)
    );
    let both = RunRequest {
        agent: "dev".to_string(),
        profile: "dev".to_string(),
        ..RunRequest::default()
    };
    let refused = library.dry_run(&plain(), &both).result.err().unwrap();
    assert_eq!(refused.exit_code(), 2);
    let tag = RunRequest {
        tags: vec!["no-such-tag".to_string()],
        ..RunRequest::default()
    };
    assert!(matches!(
        library.dry_run(&plain(), &tag).result,
        Err(AppError::NoFragments { .. })
    ));
    let fragment = RunRequest {
        fragments: vec!["golang".to_string()],
        ..RunRequest::default()
    };
    let named = ok(library.dry_run(&plain(), &fragment)).plan;
    assert!(
        named.context.contains("GO-RULES"),
        "naming a premised fragment loads it"
    );
}

// fault_tolerance.feature: a profile naming a missing bundle is refused like
// a run (the findings are fatal); --degraded previews past it.
#[test]
fn test_fatal_findings_are_fatal_unless_degraded() {
    let layers = [CONFIG];
    let library = setup(&layers).library();
    let broken = RunRequest {
        profile: "broken".to_string(),
        ..RunRequest::default()
    };
    let strict = ok(library.dry_run(&plain(), &broken));
    assert_eq!(strict.fatal.len(), 1);
    assert_eq!(strict.fatal[0].kind, "bundle");
    assert!(strict.plan.findings[0].fatal);
    let degraded = crate::Flags {
        degraded: true,
        ..plain()
    };
    let past = ok(library.dry_run(&degraded, &broken));
    assert!(past.fatal.is_empty());
    assert_eq!(past.plan.findings.len(), 1);
    assert!(!past.plan.findings[0].fatal);
    let aborted = AppError::aborted(&strict.fatal);
    assert_eq!(aborted.exit_code(), 3);
}

// launch: with no agent the run says which of the three reasons applies.
#[test]
fn test_a_run_with_no_agent_says_why() {
    let library = setup(&[CONFIG]).library();
    let none = library
        .dry_run(&plain(), &RunRequest::default())
        .result
        .err()
        .unwrap();
    assert!(matches!(none, AppError::LaunchNoAgents));
    let layers = ["schema_version: 8\nagents:\n  dev:\n    llm: claude-code\n"];
    let some = setup(&layers)
        .library()
        .dry_run(&plain(), &RunRequest::default());
    assert!(matches!(some.result, Err(AppError::LaunchNoDefault)));
    let named = RunRequest {
        agent: "nosuch".to_string(),
        ..RunRequest::default()
    };
    assert!(matches!(
        library.dry_run(&plain(), &named).result,
        Err(AppError::LaunchNoAgent { .. })
    ));
    let home = Setup {
        home_only: true,
        layers: &[CONFIG],
        ..Setup::default()
    }
    .library();
    assert!(matches!(
        home.dry_run(&plain(), &RunRequest::default()).result,
        Err(AppError::LaunchNoProject)
    ));
    let llm = RunRequest {
        llm: "nosuch".to_string(),
        profile: "dev".to_string(),
        ..RunRequest::default()
    };
    assert!(matches!(
        setup(&[CONFIG]).library().dry_run(&plain(), &llm).result,
        Err(AppError::UnknownLlm { .. })
    ));
}

// operations.AssembleContext: the missing asks and tags are reported, and an
// ask that loads nothing at all is refused.
#[test]
fn test_assembling_reports_what_it_could_not_load() {
    let library = setup(&[CONFIG]).library();
    let request = AssembleRequest {
        fragments: vec!["testing".to_string(), "nope".to_string()],
        ..AssembleRequest::default()
    };
    let assembled = ok(library.assemble(&plain(), &request));
    assert_eq!(assembled.missing_fragments, ["nope"]);
    assert!(assembled.context.contains("TEST-BODY"));
    let nothing = AssembleRequest {
        fragments: vec!["nope".to_string()],
        ..AssembleRequest::default()
    };
    assert!(matches!(
        library.assemble(&plain(), &nothing).result,
        Err(AppError::NoFragments { .. })
    ));
}

// agent.feature: showing a binding reports what it declared and what it
// resolves to; one that does not resolve says so rather than failing.
#[test]
fn test_agent_show_reports_the_declaration_the_resolution_and_a_failure_to_resolve() {
    let layers = ["schema_version: 8\nagents:\n  good:\n    llm: claude-code\n    profiles: [dev]\n  bad:\n    llm: claude-code\n    profiles: [ghost]\n"];
    let library = setup(&layers).library();
    let say = |d: &dyn angzarr_diagnostic::Diagnostic| d.code().to_string();
    let good = ok(library.agent_show(&plain(), "good", &say));
    let resolved = good.resolved.unwrap();
    assert_eq!(
        (resolved.label.as_str(), resolved.backend.as_str()),
        ("claude-code", "claude-code")
    );
    assert_eq!(resolved.model, "opus");
    assert_eq!(resolved.effective_permissions, "acceptEdits");
    assert_eq!(resolved.approver, "human");
    assert_eq!(resolved.home_mode, "session");
    assert_eq!(good.requirements.len(), 1);
    assert_eq!(good.requirements[0].command, "tree-sitter");
    assert_eq!(good.requirements[0].sources[0].kind, "mcp");
    let bad = ok(library.agent_show(&plain(), "bad", &say));
    assert!(bad.resolved.is_none());
    assert!(bad
        .resolution_error
        .starts_with("agent \"bad\": compose profiles: profile ghost: "));
    assert!(matches!(
        library.agent_show(&plain(), "nope", &say).result,
        Err(AppError::AgentNotFound { .. })
    ));
}

// bundle view: the whole tree under "==> path <==", or one item.
#[test]
fn test_viewing_a_bundle_dumps_its_tree_or_one_item() {
    let library = setup(&[CONFIG]).library();
    let whole = ok(library.bundle_view(&plain(), "demo", false));
    assert!(whole.content.starts_with("==> bundle.yaml <==\n"));
    assert!(!whole.item);
    let item = ok(library.bundle_view(&plain(), "demo#fragments/dist", true));
    assert_eq!(item.content, "# (distilled version)\nshort form\n");
    assert!(item.distilled);
    let mcp = ok(library.bundle_view(&plain(), "demo#mcp/tree-sitter", false));
    assert_eq!(
        mcp.content,
        "# MCP Server: tree-sitter\ncommand: tree-sitter\nargs:\n  - --stdio\n"
    );
    assert!(matches!(
        library
            .bundle_view(&plain(), "demo#fragments", false)
            .result,
        Err(AppError::ViewPath { .. })
    ));
    assert!(matches!(
        library
            .bundle_view(&plain(), "demo#fragments/nope", false)
            .result,
        Err(AppError::ViewMissing { .. })
    ));
    assert!(matches!(
        library.bundle_view(&plain(), "demo#hooks/x", false).result,
        Err(AppError::ViewPath { .. } | AppError::NotViewable { .. })
    ));
}

// The in-memory content repository refuses what the real one does: a path
// that leaves the bundle, and writes nothing of the batch.
#[test]
fn test_the_fake_content_repo_refuses_a_path_that_climbs() {
    use ctxloom_ports::{Change, ContentRepo, Put};
    let (_, mem) = Setup::default().fixture();
    let err = mem.apply("demo", &[Change::Write(Put::file("../x", b"evil"))]);
    assert!(err.is_err());
    assert!(mem.paths("demo").is_empty());
    assert!(mem
        .apply("demo", &[Change::Write(Put::file("ok.md", b"fine"))])
        .is_ok());
}
