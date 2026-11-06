//! What a profile set carries for an engine (S9): the bundles' commands,
//! skills, servers and hooks, and ctxloom's own callbacks. Each test names the
//! Go behaviour it holds the application to.

use crate::fakes::{plain, Setup, ENVELOPE};
use crate::PackageRequest;

const CONFIG: &str = "schema_version: 9\nagents:\n  default:\n    llm: mock\n    profiles: [dev]\ndefault_agent: default\nllm:\n  configs:\n    mock:\n      type: mock\n    claude-code:\n      type: claude-code\n    claude-fast:\n      type: claude-code\n";

const DEMO: &[(&str, &str)] = &[
    ENVELOPE,
    ("fragments/testing.md", "FRAGMENT-BODY-testing"),
    (
        "prompts/example.md",
        "---\ndescription: Example prompt\n---\nEXAMPLE-BODY",
    ),
    ("mcp/tool.yaml", "command: tool-cmd\nargs: [--flag]\n"),
    ("mcp/ctxloom.yaml", "served_by: session-endpoint\n"),
    (
        "skills/reviewer/SKILL.md",
        "---\nname: code-reviewer\ndescription: SKILL-MARKER\n---\nReview.\n",
    ),
    ("skills/reviewer/scripts/run.sh", "echo"),
    (
        "skills/.reviewer.meta.yaml",
        "tags: [ctxloom:link_id=pair]\nexecutable: [scripts/run.sh]\n",
    ),
    (
        "hooks/pre_tool/b-first.yaml",
        "type: command\ncommand: echo FIRST\n",
    ),
    ("hooks/pre_tool/.b-first.meta.yaml", "order: 100\n"),
    (
        "hooks/pre_tool/a-second.yaml",
        "type: command\ncommand: echo SECOND\n",
    ),
    ("hooks/pre_tool/.a-second.meta.yaml", "order: 200\n"),
];

const PROJECT: &[(&str, &str)] = &[
    ENVELOPE,
    (
        "profiles/dev.yaml",
        "bundles: [demo]\ndeny_tools: [Bash]\nhooks:\n  unified:\n    pre_tool:\n      - type: command\n        command: echo FROM-PROFILE\n",
    ),
];

fn setup() -> Setup<'static> {
    Setup {
        bundles: &[("demo", DEMO), ("project", PROJECT)],
        layers: &[CONFIG],
        ..Setup::default()
    }
}

fn package() -> crate::EnginePackage {
    setup()
        .library()
        .package(&plain(), &PackageRequest::default())
        .result
        .expect("a package")
}

// materialize.feature: the default agent's profiles are the set when none is
// named; the bundle's context is assembled.
#[test]
fn test_the_package_is_the_default_agents_profiles_assembled() {
    let p = package();
    assert_eq!(p.profiles, vec!["dev".to_string()]);
    assert!(p.context.contains("FRAGMENT-BODY-testing"));
    assert_eq!(p.deny_tools, vec!["Bash".to_string()]);
    assert!(p.statusline);
    assert_eq!(
        p.configured_engines,
        vec!["claude-code".to_string(), "mock".to_string()]
    );
    assert!(p.has_agents && p.has_profiles);
}

// materialize.feature: ".mock/commands/demo/example.md": a command is
// `<bundle>/<name>`.
#[test]
fn test_a_bundles_commands_are_named_for_their_bundle() {
    let p = package();
    assert_eq!(p.commands.len(), 1);
    assert_eq!(p.commands[0].name, "demo/example");
    assert_eq!(p.commands[0].description, "Example prompt");
    assert_eq!(p.commands[0].body, "EXAMPLE-BODY");
}

// j000600_agent_skill.feature: a skill arrives whole, under its frontmatter
// name, its exec bit from the sidecar's `executable:`.
#[test]
fn test_a_skill_arrives_whole_under_its_frontmatter_name() {
    let p = package();
    assert_eq!(p.skills.len(), 1);
    let skill = &p.skills[0];
    assert_eq!(skill.name, "code-reviewer");
    assert_eq!(skill.description, "SKILL-MARKER");
    assert_eq!(skill.tags, vec!["ctxloom:link_id=pair".to_string()]);
    let run = skill
        .files
        .iter()
        .find(|f| f.path == "scripts/run.sh")
        .unwrap();
    assert!(run.executable);
    assert!(
        !skill
            .files
            .iter()
            .find(|f| f.path == "SKILL.md")
            .unwrap()
            .executable
    );
}

const VAULT: &[(&str, &str)] = &[
    ENVELOPE,
    (
        "skills/reviewer/SKILL.md",
        "---\nname: reviewer\ndescription: R\n---\nReview.\n",
    ),
    (
        "skills/planner/SKILL.md",
        "---\nname: planner\ndescription: P\n---\nPlan.\n",
    ),
    ("prompts/review.md", "---\ndescription: r\n---\nREVIEW-BODY"),
    ("prompts/plan.md", "---\ndescription: p\n---\nPLAN-BODY"),
];

fn vault_package(profile: &'static str) -> crate::EnginePackage {
    let project: &'static [(&'static str, &'static str)] =
        Box::leak(Box::new([ENVELOPE, ("profiles/dev.yaml", profile)]));
    Setup {
        bundles: &[("vault", VAULT), ("project", project)],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .library()
    .package(&plain(), &PackageRequest::default())
    .result
    .expect("a package")
}

// cli/skill.feature "Profile skill curation exports only the curated skill,
// not the bundle's other one" (Go's composite assembly: a profile that
// curates skills exports those, not every skill of the bundles it includes).
#[test]
fn test_a_profile_that_curates_skills_exports_only_those() {
    let p = vault_package("bundles: [vault]\nskills: [vault#skills/reviewer]\n");
    let names: Vec<&str> = p.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["reviewer"]);
}

// The same profile without curation exports every skill its bundles hold.
#[test]
fn test_a_profile_that_curates_nothing_exports_every_bundle_skill() {
    let p = vault_package("bundles: [vault]\n");
    let mut names: Vec<&str> = p.skills.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["planner", "reviewer"]);
}

// Go's composite assembly treats commands alike: curated commands replace the
// bundles' whole set.
#[test]
fn test_a_profile_that_curates_commands_exports_only_those() {
    let p = vault_package("bundles: [vault]\ncommands: [vault#commands/review]\n");
    let names: Vec<&str> = p.commands.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["vault/review"]);
}

// manage.feature: a server the bundle ships is carried; ctxloom's own, served
// by the session's endpoint, is carried as such (and registers nothing at
// rest).
#[test]
fn test_the_servers_the_bundles_ship_are_carried_with_who_serves_them() {
    let p = package();
    let tool = p.mcp.iter().find(|m| m.name == "tool").unwrap();
    assert_eq!(
        (tool.command.as_str(), tool.args.clone()),
        ("tool-cmd", vec!["--flag".to_string()])
    );
    let own = p.mcp.iter().find(|m| m.name == "ctxloom").unwrap();
    assert_eq!(own.served_by, "session-endpoint");
}

// manage.feature: "Listing hooks shows the resolved order and where each came
// from": profile hooks, then the bundle's by `order:`, then ctxloom's own.
#[test]
fn test_hooks_merge_by_append_each_bundle_ordered_by_its_own_order() {
    let p = package();
    let pre: Vec<(&str, &str, usize)> = p
        .hooks
        .iter()
        .filter(|h| h.event == "pre_tool")
        .map(|h| (h.hook.command.as_str(), h.source_kind.as_str(), h.position))
        .collect();
    assert_eq!(
        pre,
        vec![
            ("echo FROM-PROFILE", "profile-directory", 1),
            ("echo FIRST", "bundle", 2),
            ("echo SECOND", "bundle", 3),
        ]
    );
    let source = |command: &str| {
        p.hooks
            .iter()
            .find(|h| h.hook.command == command)
            .map(|h| h.source.clone())
            .unwrap()
    };
    assert_eq!(source("echo FIRST"), "ctxloom+local:demo");
    assert_eq!(source("echo FROM-PROFILE"), "dev");
}

// 8.4: ctxloom's own session-start and skill-mates callbacks ride every
// package; skill-mates is narrowed to the skill tool class.
#[test]
fn test_ctxloom_own_callbacks_ride_every_package() {
    let p = package();
    let own: Vec<_> = p
        .hooks
        .iter()
        .filter(|h| h.source_kind == "context")
        .collect();
    assert_eq!(own.len(), 2);
    let start = own.iter().find(|h| h.event == "session_start").unwrap();
    assert_eq!(start.hook.args, vec!["hook", "session-start"]);
    assert_eq!(start.hook.event, "session_start");
    assert_eq!(start.hook.name, "session-start");
    assert_eq!(start.hook.command, "ctxloom");
    assert_eq!(start.hook.kind, "command");
    assert_eq!(start.hook.timeout, 15);
    let mates = own.iter().find(|h| h.event == "post_tool").unwrap();
    assert_eq!(
        (mates.hook.args[1].as_str(), mates.hook.tool.as_str()),
        ("skill-mates", "skill")
    );
}

// manage.feature: "Statusline can be disabled and re-enabled": the choice is
// kept in the project's config.
#[test]
fn test_the_statusline_choice_is_kept_in_the_projects_config() {
    let (library, mem) = setup().fixture();
    library
        .set_statusline(&plain(), false)
        .result
        .expect("written");
    let config = mem.config(ctxloom_ports::Scope::Project).unwrap();
    assert!(config.contains("statusline: false"), "{config}");
    let p = library
        .package(&plain(), &PackageRequest::default())
        .result
        .unwrap();
    assert!(!p.statusline);
    library
        .set_statusline(&plain(), true)
        .result
        .expect("written");
    let p = library
        .package(&plain(), &PackageRequest::default())
        .result
        .unwrap();
    assert!(p.statusline);
}

// manage.feature: a profile naming a bundle that is not installed is a fatal
// finding under strict, and none under --degraded.
#[test]
fn test_a_missing_bundle_is_fatal_unless_degraded() {
    let bundles: &[(&str, &[(&str, &str)])] = &[(
        "project",
        &[ENVELOPE, ("profiles/dev.yaml", "bundles: [absent]\n")],
    )];
    let setup = Setup {
        bundles,
        layers: &[CONFIG],
        ..Setup::default()
    };
    let strict = setup
        .library()
        .package(&plain(), &PackageRequest::default());
    let strict = strict.result.unwrap();
    // Only the failing finding is fatal, never the warning beside it.
    assert_eq!(strict.fatal.len(), 1);
    assert!(
        matches!(&strict.fatal[0], crate::Notice::Finding { text, .. } if text.starts_with("failed to load bundle")),
        "{:?}",
        strict.fatal
    );
    let mut flags = plain();
    flags.degraded = true;
    let waived = setup.library().package(&flags, &PackageRequest::default());
    assert!(waived.result.unwrap().fatal.is_empty());
}

// materialize.feature: profiles named on the command line replace the default
// agent's.
#[test]
fn test_named_profiles_replace_the_default_agents() {
    let request = PackageRequest {
        profiles: vec!["dev".to_string()],
    };
    let p = setup()
        .library()
        .package(&plain(), &request)
        .result
        .unwrap();
    assert_eq!(p.profiles, vec!["dev".to_string()]);
}

#[test]
fn test_the_engines_are_the_ones_this_build_knows() {
    let names = setup().library().engine_names();
    let known: Vec<String> = crate::ENGINES.iter().map(|e| e.to_string()).collect();
    assert!(!known.is_empty());
    assert_eq!(names, known);
}

#[test]
fn test_distilled_forms_are_preferred_unless_the_config_says_not() {
    assert!(package().use_distilled);
    let off = Setup {
        layers: &["schema_version: 9\nconfig:\n  use_distilled: false\n"],
        ..setup()
    };
    let p = off
        .library()
        .package(&plain(), &PackageRequest::default())
        .result
        .unwrap();
    assert!(!p.use_distilled);
}

fn packaged(
    layer: &'static str,
    bundles: &'static [(&str, &[(&str, &str)])],
) -> crate::EnginePackage {
    Setup {
        bundles,
        layers: &[layer],
        ..Setup::default()
    }
    .library()
    .package(&plain(), &PackageRequest::default())
    .result
    .expect("a package")
}

const NO_AGENTS: &str = "schema_version: 9\n";
const AGENT_WITHOUT_PROFILES: &str =
    "schema_version: 9\nagents:\n  a:\n    llm: mock\ndefault_agent: a\nllm:\n  configs:\n    mock:\n      type: mock\n";

#[test]
fn test_a_project_says_whether_it_has_agents_and_profiles_to_bind_them_to() {
    let bare = packaged(NO_AGENTS, &[("project", &[ENVELOPE])]);
    assert!(!bare.has_agents);
    assert!(!bare.has_profiles);

    // Agents, but no profile anywhere.
    let agent = packaged(AGENT_WITHOUT_PROFILES, &[("project", &[ENVELOPE])]);
    assert!(agent.has_agents);
    assert!(!agent.has_profiles);

    // A profile in a bundle, with no default agent profiles.
    let in_bundle = packaged(
        NO_AGENTS,
        &[(
            "project",
            &[ENVELOPE, ("profiles/dev.yaml", "bundles: []\n")],
        )],
    );
    assert!(!in_bundle.has_agents);
    assert!(in_bundle.has_profiles);
}

#[test]
fn test_the_statusline_choice_keeps_the_other_config_entries() {
    let held = Setup {
        layers: &["schema_version: 9\nconfig:\n  use_distilled: false\n  statusline: true\n"],
        ..setup()
    };
    let (library, mem) = held.fixture();
    library
        .set_statusline(&plain(), false)
        .result
        .expect("written");
    let config = mem.config(ctxloom_ports::Scope::Project).unwrap();
    assert!(config.contains("use_distilled: false"), "{config}");
    assert!(config.contains("statusline: false"), "{config}");
    assert!(!config.contains("statusline: true"), "{config}");
}

// ---- companion loadouts in the package (virtuous-word, companions slice 2) ----

mod companions {
    use ctxloom_ports::CompanionLoadout::{self, Loaded};

    use crate::fakes::{plain, Setup};
    use crate::{EnginePackage, Flags, PackageRequest};

    use super::{CONFIG, DEMO, PROJECT};

    const WITH: &str = "schema_version: 9\nagents:\n  default:\n    llm: mock\n    profiles: [dev]\ndefault_agent: default\nllm:\n  configs:\n    mock:\n      type: mock\ncompanions:\n  zeta:\n    command: [zeta]\n  alpha:\n    command: [alpha]\n";

    const ALPHA: &str = "run:\n  version: 1.0.0\n  hooks:\n    pre_tool:\n      - command: alpha guard\n        tool: shell\n        type: command\n      - command: alpha guard\n        tool: file_edit\n        type: command\n  commands:\n    check:\n      description: Alpha check\n      content: ALPHA-CHECK-BODY\n  mcp:\n    alpha-tool:\n      command: alpha-mcp\n      args: [--serve]\n";

    const ZETA: &str = "run:\n  version: 1.0.0\n  hooks:\n    pre_tool:\n      - command: zeta guard\n        type: command\n  commands:\n    check:\n      description: Zeta check\n      content: ZETA-CHECK-BODY\n";

    fn package_with(
        flags: &Flags,
        layer: &'static str,
        companions: &[(&str, CompanionLoadout)],
    ) -> EnginePackage {
        Setup {
            bundles: &[("demo", DEMO), ("project", PROJECT)],
            layers: &[layer],
            companions,
            ..Setup::default()
        }
        .library()
        .package(flags, &PackageRequest::default())
        .result
        .expect("a package")
    }

    fn both() -> [(&'static str, CompanionLoadout); 2] {
        [
            ("alpha", Loaded(ALPHA.to_string())),
            ("zeta", Loaded(ZETA.to_string())),
        ]
    }

    fn pre_tool(p: &EnginePackage) -> Vec<(String, String, String, usize)> {
        p.hooks
            .iter()
            .filter(|h| h.event == "pre_tool")
            .map(|h| {
                (
                    h.hook.command.clone(),
                    h.source_kind.clone(),
                    h.source.clone(),
                    h.position,
                )
            })
            .collect()
    }

    // Go's ResolveBundleHooksFor: companions' hooks, then the profile
    // bundles', after the profiles' own and before ctxloom's callbacks.
    #[test]
    fn companion_hooks_follow_the_profiles_own_and_precede_the_bundles() {
        let p = package_with(&plain(), WITH, &both());
        let row = |c: &str, kind: &str, source: &str, n: usize| {
            (c.to_string(), kind.to_string(), source.to_string(), n)
        };
        assert_eq!(
            pre_tool(&p),
            vec![
                row("echo FROM-PROFILE", "profile-directory", "dev", 1),
                row("alpha guard", "bundle", "ctxloom+companion:alpha", 2),
                row("alpha guard", "bundle", "ctxloom+companion:alpha", 3),
                row("zeta guard", "bundle", "ctxloom+companion:zeta", 4),
                row("echo FIRST", "bundle", "ctxloom+local:demo", 5),
                row("echo SECOND", "bundle", "ctxloom+local:demo", 6),
            ]
        );
    }

    #[test]
    fn a_companion_hook_keeps_its_tool_class_for_the_engine_to_map() {
        let p = package_with(&plain(), WITH, &both());
        let tools: Vec<&str> = p
            .hooks
            .iter()
            .filter(|h| h.hook.command == "alpha guard")
            .map(|h| h.hook.tool.as_str())
            .collect();
        assert_eq!(tools, ["shell", "file_edit"]);
    }

    #[test]
    fn ctxloom_own_callbacks_still_come_last() {
        let p = package_with(&plain(), WITH, &both());
        let start: Vec<&str> = p
            .hooks
            .iter()
            .filter(|h| h.event == "session_start")
            .map(|h| h.source_kind.as_str())
            .collect();
        assert_eq!(start, ["context"]);
        let kinds: Vec<&str> = p
            .hooks
            .iter()
            .filter(|h| h.event == "post_tool")
            .map(|h| h.source_kind.as_str())
            .collect();
        assert_eq!(kinds, ["context"]);
    }

    // Go's commandCollector.fromBundles(CompanionRefs()): every command of
    // every companion, by companion name, named for the companion.
    #[test]
    fn a_companions_commands_are_named_for_the_companion_in_name_order() {
        let p = package_with(&plain(), WITH, &both());
        let names: Vec<(&str, &str, &str)> = p
            .commands
            .iter()
            .map(|c| (c.name.as_str(), c.description.as_str(), c.body.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("demo/example", "Example prompt", "EXAMPLE-BODY"),
                ("alpha/check", "Alpha check", "ALPHA-CHECK-BODY"),
                ("zeta/check", "Zeta check", "ZETA-CHECK-BODY"),
            ]
        );
    }

    // "a curation names bundle commands, never theirs".
    #[test]
    fn a_companions_commands_are_unconditional_under_a_curation() {
        let project: &'static [(&str, &str)] = &[
            crate::fakes::ENVELOPE,
            (
                "profiles/dev.yaml",
                "bundles: [demo]\ncommands: [demo#commands/example]\n",
            ),
        ];
        let companions = both();
        let p = Setup {
            bundles: &[("demo", DEMO), ("project", project)],
            layers: &[WITH],
            companions: &companions,
            ..Setup::default()
        }
        .library()
        .package(&plain(), &PackageRequest::default())
        .result
        .expect("a package");
        let names: Vec<&str> = p.commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["demo/example", "alpha/check", "zeta/check"]);
    }

    #[test]
    fn a_companions_mcp_server_is_carried_with_its_command() {
        let p = package_with(&plain(), WITH, &both());
        let server = p.mcp.iter().find(|m| m.name == "alpha-tool").unwrap();
        assert_eq!(
            (server.command.as_str(), server.args.clone()),
            ("alpha-mcp", vec!["--serve".to_string()])
        );
        assert!(p.mcp.iter().any(|m| m.name == "tool"), "the bundle's too");
        assert!(p.fatal.is_empty(), "{:?}", p.fatal);
    }

    // Go: companions claim first; a profile bundle declaring the name is the
    // contest's loser, loudly.
    #[test]
    fn a_profile_bundle_cannot_take_a_name_a_companion_claimed() {
        let rival = "run:\n  version: 1.0.0\n  mcp:\n    tool:\n      command: companion-tool\n";
        let companions = [("alpha", Loaded(rival.to_string()))];
        let p = package_with(&plain(), WITH, &companions);
        let tool: Vec<&str> = p
            .mcp
            .iter()
            .filter(|m| m.name == "tool")
            .map(|m| m.command.as_str())
            .collect();
        assert_eq!(tool, ["companion-tool"]);
        assert_eq!(p.fatal.len(), 1, "{:?}", p.fatal);
    }

    #[test]
    fn no_companions_flag_delivers_nothing_of_them() {
        let flags = Flags {
            no_companions: true,
            ..Flags::default()
        };
        let p = package_with(&flags, WITH, &both());
        assert!(pre_tool(&p)
            .iter()
            .all(|h| h.2 != "ctxloom+companion:alpha"));
        assert_eq!(p.commands.len(), 1);
        assert!(p.mcp.iter().all(|m| m.name != "alpha-tool"));
    }

    #[test]
    fn a_package_without_companions_is_unchanged() {
        let p = package_with(&plain(), CONFIG, &[]);
        assert_eq!(pre_tool(&p).len(), 3);
        assert_eq!(p.commands.len(), 1);
    }
}
