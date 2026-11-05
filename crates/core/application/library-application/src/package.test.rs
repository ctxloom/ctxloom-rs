//! What a profile set carries for an engine (S9): the bundles' commands,
//! skills, servers and hooks, and ctxloom's own callbacks. Each test names the
//! Go behaviour it holds the application to.

use crate::fakes::{plain, Setup, ENVELOPE};
use crate::PackageRequest;

const CONFIG: &str = "schema_version: 8\nagents:\n  default:\n    llm: mock\n    profiles: [dev]\ndefault_agent: default\nllm:\n  configs:\n    mock:\n      type: mock\n    claude-code:\n      type: claude-code\n    claude-fast:\n      type: claude-code\n";

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
        layers: &["schema_version: 8\nconfig:\n  use_distilled: false\n"],
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

const NO_AGENTS: &str = "schema_version: 8\n";
const AGENT_WITHOUT_PROFILES: &str =
    "schema_version: 8\nagents:\n  a:\n    llm: mock\ndefault_agent: a\nllm:\n  configs:\n    mock:\n      type: mock\n";

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
        layers: &["schema_version: 8\nconfig:\n  use_distilled: false\n  statusline: true\n"],
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
