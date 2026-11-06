//! Go's acceptance scenarios for the library's read side (S1), ported: each
//! test names its feature file and scenario, and says what the scenario
//! asserts, in the order it asserts it. A scenario outline's Examples rows
//! are the `FORMATS` loop: no `--format` (off a terminal, json), json, text.
//!
//! The commands that WRITE (create, edit, remove, import, ...) are S2's; where
//! a scenario starts by running one, the project is put in place directly
//! (`Project::bundle` and the rest of `library.test.rs`).

use serde_json::Value;

use super::library::{contains_value, entries, reports_as, Project, MINIMAL_CONFIG};
use crate::tests::files;

/// An outline's `<flags>` column: the three rows every `@json-default` table has.
const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn run_with_flags(project: &Project, base: &[&str], flags: &[&str]) -> super::Ran {
    let mut args = base.to_vec();
    args.extend_from_slice(flags);
    project.run(&args)
}

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

/// `bundle "demo"` with `fragment "testing"`, as the steps make them.
fn demo_with_testing() -> Project {
    let project = Project::initialized();
    project.bundle("demo");
    project.fragment("demo", "testing");
    project
}

// ---------------------------------------------------------------- cli/bundle.feature

// bundle.feature: Scenario Outline: The listing summarizes each bundle by
// version and by what it holds (the read half: counts, version; the edit that
// follows is S2's).
#[test]
fn test_bundle_listing_summarizes_each_bundle_by_version_and_content() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        let ran = run_with_flags(&project, &["bundle", "list", "--no-companions"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Installed bundles (1):")
                .contains("demo (v1.0.0)")
                .contains("Contains: 1 fragment")
                .contains("1 command");
        } else {
            let doc = ran.json();
            assert_eq!(entries(&doc, "$"), 1);
            assert_eq!(reports_as(&doc, "[name=demo].version"), "1.0.0");
            assert_eq!(reports_as(&doc, "[name=demo].fragment_count"), "1");
            assert_eq!(reports_as(&doc, "[name=demo].command_count"), "1");
        }
    }
}

// bundle.feature: Scenario Outline: Bare bundle lists what is installed
#[test]
fn test_bare_bundle_lists_what_is_installed() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        let ran = run_with_flags(&project, &["bundle", "--no-companions"], flags);
        ran.succeeds().lacks("Available Commands:");
        if is_text(flags) {
            ran.contains("Installed bundles (1):")
                .contains("demo (v1.0.0)");
        } else {
            let doc = ran.json();
            assert_eq!(entries(&doc, "$"), 1);
            assert_eq!(reports_as(&doc, "[name=demo].version"), "1.0.0");
        }
    }
}

// bundle.feature: Scenario Outline: Showing a bundle names every section it carries
#[test]
fn test_showing_a_bundle_names_every_section_it_carries() {
    for flags in FORMATS {
        let project = demo_with_testing();
        let ran = run_with_flags(&project, &["bundle", "show", "demo"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Bundle: demo")
                .contains("Description: acceptance fixture bundle")
                .contains("Fragments (2):")
                .contains("- example [example] (no_distill)")
                .contains("- testing")
                .contains("Commands (1):")
                .contains("Example prompt");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "name"), "demo");
            assert_eq!(reports_as(&doc, "description"), "acceptance fixture bundle");
            assert_eq!(entries(&doc, "fragments"), 2);
            assert_eq!(reports_as(&doc, "fragments.example.no_distill"), "true");
            assert_eq!(reports_as(&doc, "fragments.testing.no_distill"), "false");
            assert_eq!(entries(&doc, "commands"), 1);
            assert_eq!(
                reports_as(&doc, "commands.example.description"),
                "Example prompt"
            );
        }
    }
}

// bundle.feature: Scenario Outline: Show does not take the item selector —
// that grammar belongs to view
#[test]
fn test_show_does_not_take_the_item_selector_but_view_does() {
    for flags in FORMATS {
        let project = demo_with_testing();
        let shown = run_with_flags(&project, &["bundle", "show", "demo"], flags);
        shown.succeeds();
        if is_text(flags) {
            shown.contains("Fragments (2):");
        } else {
            assert_eq!(entries(&shown.json(), "fragments"), 2);
        }
        project
            .run(&["bundle", "show", "demo#fragments/testing"])
            .fails()
            .contains("bundle not found: demo#fragments/testing");
        project
            .run(&["bundle", "view", "demo#fragments/testing"])
            .succeeds()
            .contains("FRAGMENT-BODY-testing");
    }
}

// bundle.feature: Scenario: Viewing a bundle dumps the manifest with its
// items' bodies in it
#[test]
fn test_viewing_a_bundle_dumps_the_manifest_with_its_items_bodies() {
    let project = demo_with_testing();
    project
        .run(&["bundle", "view", "demo"])
        .succeeds()
        .contains("testing")
        .contains("FRAGMENT-BODY-testing")
        .contains("Example prompt content. Describe what this prompt does.");
}

// bundle.feature: Scenario: A path selector returns one item's bytes and
// leaves the rest behind
#[test]
fn test_a_path_selector_returns_one_items_bytes() {
    let project = demo_with_testing();
    let fragment = project.run(&["bundle", "view", "demo#fragments/testing"]);
    fragment
        .succeeds()
        .contains("FRAGMENT-BODY-testing")
        .lacks("Example prompt content. Describe what this prompt does.");
    let command = project.run(&["bundle", "view", "demo#commands/example"]);
    command
        .succeeds()
        .contains("Example prompt content. Describe what this prompt does.")
        .lacks("FRAGMENT-BODY-testing");
}

// bundle.feature: Scenario: The selector reaches an MCP server's
// configuration too
#[test]
fn test_the_selector_reaches_an_mcp_servers_configuration() {
    let project = Project::initialized();
    project.bundle("demo");
    project.write(
        ".ctxloom/content/bundles/v2/demo/mcp/tree-sitter.yaml",
        "command: tree-sitter\n",
    );
    project
        .run(&["bundle", "view", "demo#mcp/tree-sitter"])
        .succeeds()
        .contains("# MCP Server: tree-sitter")
        .contains("command: tree-sitter");
}

// bundle.feature: Scenario: Asking for the distilled rendering of an item
// that has none falls back to the raw bytes
#[test]
fn test_asking_for_a_distilled_rendering_that_was_never_made_falls_back() {
    let project = demo_with_testing();
    project
        .run(&["bundle", "view", "demo#fragments/testing"])
        .succeeds()
        .contains("FRAGMENT-BODY-testing");
    project
        .run(&["bundle", "view", "demo#fragments/testing", "--distilled"])
        .succeeds()
        .contains("FRAGMENT-BODY-testing");
}

// bundle.feature: Scenario: A selector that is not `type/name`, or names a
// type that does not exist, is refused by name
#[test]
fn test_a_bad_selector_is_refused_by_name() {
    let project = demo_with_testing();
    project
        .run(&["bundle", "view", "demo#fragments"])
        .fails()
        .contains("invalid path format");
    project
        .run(&["bundle", "view", "demo#widgets/testing"])
        .fails()
        .contains("unknown item kind")
        .contains("widgets")
        .contains("expected fragments, commands, mcp, skills, or profiles");
    project
        .run(&["bundle", "view", "demo#fragments/testing"])
        .succeeds()
        .contains("FRAGMENT-BODY-testing");
}

// ---------------------------------------------------------------- cli/fragment.feature, read_extras.feature

// fragment.feature: Scenario: Search finds a fragment by name
#[test]
fn test_search_finds_a_fragment_by_name() {
    let project = demo_with_testing();
    project
        .run(&["search", "--local", "testing"])
        .succeeds()
        .contains("testing");
}

// read_extras.feature: Scenario: Show a fragment's content
#[test]
fn test_show_a_fragments_content() {
    let project = demo_with_testing();
    project
        .run(&["fragment", "show", "demo#fragments/testing"])
        .succeeds()
        .contains("testing")
        .contains("FRAGMENT-BODY-testing");
}

// read_extras.feature: Scenario: Show a prompt's content
#[test]
fn test_show_a_prompts_content() {
    let project = Project::initialized();
    project.bundle("demo");
    project.command("demo", "review");
    project
        .run(&["command", "show", "demo#commands/review"])
        .succeeds()
        .contains("review")
        .contains("COMMAND-BODY-review");
}

// fragment.feature: Scenario: Alice creates a fragment and it lands in the
// manifest, the listing, and the agent's resource (the listing half; the
// create is S2's and the MCP resource is the MCP slice's)
#[test]
fn test_a_fragment_in_a_bundle_is_in_the_listing() {
    let project = demo_with_testing();
    project
        .run(&["fragment", "list"])
        .succeeds()
        .contains("testing");
}

// ---------------------------------------------------------------- cli/command.feature

// command.feature: Scenario: Listing commands with a misspelled --bundle is
// refused, not answered empty
#[test]
fn test_a_command_listing_with_a_misspelled_bundle_is_refused() {
    let project = Project::initialized();
    project.bundle("demo");
    project.command("demo", "review");
    project
        .run(&["command", "list", "--bundle", "nosuchbundle"])
        .fails()
        .contains("no bundle named")
        .contains("nosuchbundle");
}

// command.feature: Scenario: Alice creates a command and it is listed (the
// listing half)
#[test]
fn test_a_command_in_a_bundle_is_listed() {
    let project = Project::initialized();
    project.bundle("demo");
    project.command("demo", "review");
    project
        .run(&["command", "list"])
        .succeeds()
        .contains("review");
}

// ---------------------------------------------------------------- cli/skill.feature

const SKILL_ROOT: &str = ".ctxloom/content/bundles/v2/vault";

/// Go's "Alice's project has a directory-form bundle "vault"" with the skill
/// `skill create vault reviewer -d SKILL-MARKER-reviewer-9f3c21` makes, and
/// "Alice adds an executable scripts/run.sh" (`steps_skill.go`: written with
/// mode 0755 on disk, which is what `skill show` reports: Go reads a package's
/// listing off its directory, so the mode is the file's, whatever a sidecar
/// declares).
fn vault_with_reviewer() -> Project {
    let project = Project::initialized();
    project.write(
        &format!("{SKILL_ROOT}/bundle.yaml"),
        "schema_version: 4\nversion: 1.0.0\n",
    );
    project.write(
        &format!("{SKILL_ROOT}/skills/reviewer/SKILL.md"),
        "---\nname: reviewer\ndescription: SKILL-MARKER-reviewer-9f3c21\n---\n\n# reviewer\n\nTODO: describe what this skill does and how to use it.\n",
    );
    let script = format!("{SKILL_ROOT}/skills/reviewer/scripts/run.sh");
    project.write(&script, "#!/bin/sh\n# SCRIPT-MARKER-reviewer-2b7e40\n");
    files::set_mode(project.dir().join(&script), 0o755);
    project.write(
        &format!("{SKILL_ROOT}/skills/.reviewer.meta.yaml"),
        "executable:\n  - scripts/run.sh\n",
    );
    project
}

// skill.feature: Scenario: Alice authors a skill package and its listing and
// show reflect the real tree
#[test]
fn test_a_skills_listing_and_show_reflect_the_real_tree() {
    let project = vault_with_reviewer();
    let shown = project.run(&["skill", "show", "vault#skills/reviewer", "--format", "json"]);
    shown.succeeds();
    let doc = shown.json();
    let files = doc["files"].as_array().expect("a files list");
    let file = |path: &str| files.iter().find(|f| f["path"] == path);
    assert!(file("SKILL.md").is_some(), "{doc}");
    let script = file("scripts/run.sh").expect("the script is listed");
    assert_eq!(script["mode"], "0755", "{doc}");
    assert!(script["sha256"]
        .as_str()
        .is_some_and(|s| s.starts_with("sha256:")));
    let list = project.run(&["skill", "list"]);
    list.succeeds().contains("reviewer").contains("vault");
    project
        .run(&["skill", "show", "vault#skills/reviewer"])
        .succeeds()
        .contains("SKILL-MARKER-reviewer-9f3c21");
}

// skill.feature: Scenario: Removing a skill nobody created is refused, not
// silently reported as done (the read half: showing one is refused)
#[test]
fn test_showing_a_skill_nobody_created_is_refused() {
    let project = vault_with_reviewer();
    project
        .run(&["skill", "show", "vault#skills/never-created"])
        .fails();
}

// ---------------------------------------------------------------- cli/profile.feature

// profile.feature: Scenario Outline: Creating a profile writes its definition
// and it is listed back (the listing half)
#[test]
fn test_a_profile_is_listed_back_with_its_description() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        project.write(
            ".ctxloom/content/bundles/v2/project/bundle.yaml",
            "schema_version: 4\nversion: 1.0.0\n",
        );
        project.write(
            ".ctxloom/content/bundles/v2/project/profiles/dev.yaml",
            "description: day-to-day-work\nbundles:\n  - demo\n",
        );
        let ran = run_with_flags(&project, &["profile", "list"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Profiles (1):").contains("day-to-day-work");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "$.0.name"), "dev");
            assert_eq!(reports_as(&doc, "$.0.description"), "day-to-day-work");
        }
    }
}

// profile.feature: Scenario Outline: Showing a profile reads its composition back
#[test]
fn test_showing_a_profile_reads_its_composition_back() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        project.profile("dev", "demo");
        let ran = run_with_flags(&project, &["profile", "show", "dev"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Profile: dev").contains("- demo");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "name"), "dev");
            assert!(contains_value(&doc, "bundles", "demo"), "{doc}");
        }
    }
}

// profile.feature: Scenario Outline: Bare profile lists the profiles, and so
// does ls
#[test]
fn test_bare_profile_lists_the_profiles_and_so_does_ls() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        project.profile("dev", "demo");
        for base in [&["profile"][..], &["profile", "ls"]] {
            let ran = run_with_flags(&project, base, flags);
            ran.succeeds().lacks("Available Commands:");
            if is_text(flags) {
                ran.contains("dev");
            } else {
                assert_eq!(reports_as(&ran.json(), "$.0.name"), "dev");
            }
        }
    }
}

// profile.feature: Scenario Outline: A parent profile's content is inherited
// (the read half: show names the parent)
#[test]
fn test_a_profile_show_names_its_parent() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        project.profile("base", "demo");
        project.write(
            ".ctxloom/content/bundles/v2/project/profiles/dev.yaml",
            "description: day-to-day-work\nparents:\n  - base\nbundles:\n  - demo\n",
        );
        let ran = run_with_flags(&project, &["profile", "show", "dev"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("- base");
        } else {
            assert!(contains_value(&ran.json(), "parents", "base"));
        }
    }
}

// profile.feature: Scenario: Removing a profile with --yes takes the file and
// the listing entry (the empty listing it leaves)
#[test]
fn test_a_project_with_no_profiles_says_so() {
    let project = Project::initialized();
    project
        .run(&["profile", "list", "--format", "text"])
        .succeeds()
        .contains("No profiles defined.");
}

// ---------------------------------------------------------------- cli/agent.feature

// agent.feature: Scenario Outline: A project with no bindings says so, and
// says where to define one
#[test]
fn test_a_project_with_no_bindings_says_so_and_where_to_define_one() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = run_with_flags(&project, &["agent", "list"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("No agents defined")
                .contains("'agents:' in .ctxloom/config.yaml");
        } else {
            assert_eq!(ran.json(), Value::Array(vec![]));
        }
    }
}

// agent.feature: Scenario Outline: Creating a binding records every axis, and
// the listing reads each one back (the listing half, from a binding written
// in the config)
#[test]
fn test_the_listing_reads_each_axis_of_a_binding_back() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("dev-base");
        project.profile("dev", "dev-base");
        project.write(
            ".ctxloom/config.yaml",
            "schema_version: 8\nagents:\n  developer:\n    llm: claude-code\n    profiles:\n      - dev\n    runtime: container-rootless\n",
        );
        let ran = run_with_flags(&project, &["agent", "list"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("llm: claude-code")
                .contains("profiles: dev")
                .contains("runtime: container-rootless");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "[name=developer].llm"), "claude-code");
            assert!(contains_value(&doc, "[name=developer].profiles", "dev"));
            assert_eq!(
                reports_as(&doc, "[name=developer].runtime"),
                "container-rootless"
            );
        }
    }
}

// agent.feature: Scenario Outline: Bare agent lists the bindings
#[test]
fn test_bare_agent_lists_the_bindings() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("dev-base");
        project.profile("dev", "dev-base");
        project.write(
            ".ctxloom/config.yaml",
            "schema_version: 8\nagents:\n  developer:\n    profiles: [dev]\n",
        );
        let ran = run_with_flags(&project, &["agent"], flags);
        ran.succeeds().lacks("Available Commands:");
        if is_text(flags) {
            ran.contains("developer");
        } else {
            assert_eq!(
                reports_as(&ran.json(), "[name=developer].name"),
                "developer"
            );
        }
    }
}

// agent.feature: Scenario Outline: Showing one binding reports what was
// declared and what it resolves to
#[test]
fn test_showing_one_binding_reports_what_was_declared_and_what_it_resolves_to() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("dev-base");
        project.profile("dev", "dev-base");
        project.write(
            ".ctxloom/config.yaml",
            "schema_version: 8\nagents:\n  developer:\n    llm: claude-code\n    profiles: [dev]\n    runtime: container-rootless\n",
        );
        let ran = run_with_flags(&project, &["agent", "show", "developer"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Agent: developer")
                .contains("Engine (declared): claude-code")
                .contains("Runtime: container-rootless")
                .contains("Resolved llm:")
                .contains("Composed fragments:");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "definition.name"), "developer");
            assert_eq!(reports_as(&doc, "definition.llm"), "claude-code");
            assert_eq!(reports_as(&doc, "definition.runtime"), "container-rootless");
            assert!(contains_value(&doc, "definition.profiles", "dev"));
            assert_eq!(reports_as(&doc, "resolved.label"), "claude-code");
            assert!(contains_value(
                &doc,
                "resolved.fragments",
                "ctxloom+local:dev-base#fragments/example"
            ));
        }
    }
}

// agent.feature: Scenario: Showing a binding nobody defined fails, naming
// what was asked for
#[test]
fn test_showing_a_binding_nobody_defined_fails_naming_it() {
    let project = Project::initialized();
    project
        .run(&["agent", "show", "nosuchagent"])
        .fails()
        .contains("nosuchagent");
}

// agent.feature: Scenario: A binding still carrying auth is refused, naming the key
#[test]
fn test_a_binding_still_carrying_auth_is_refused_naming_the_key() {
    let project = Project::initialized();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\nagents:\n  developer:\n    profiles: [dev]\n    auth: login\n",
    );
    project
        .run(&["agent", "list"])
        .fails()
        .contains("unknown key `agents.developer.auth`");
}

// ---------------------------------------------------------------- cli/llm.feature

// llm.feature: Scenario Outline: The listing names the built-in backends and
// marks the default
#[test]
fn test_the_llm_listing_names_the_builtin_backends_and_marks_the_default() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = run_with_flags(&project, &["llm", "list"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("(default)").contains("claude-fast");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "[label=claude-code].default"), "true");
            assert_eq!(reports_as(&doc, "[label=claude-fast].label"), "claude-fast");
        }
    }
}

// llm.feature: Scenario: Bare llm lists the engines
#[test]
fn test_bare_llm_lists_the_engines() {
    let project = Project::initialized();
    project
        .run(&["llm"])
        .succeeds()
        .contains("claude-code")
        .lacks("Available Commands:");
}

// llm.feature: Scenario Outline: A label the team wrote is told apart from an
// engine ctxloom supplied (the listing half, with the label in the config)
#[test]
fn test_a_label_the_team_wrote_is_told_apart_from_one_ctxloom_supplied() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.write(
            ".ctxloom/config.yaml",
            "schema_version: 8\nllm:\n  configs:\n    big:\n      type: mock\n      model: o1\n    claude-code:\n      type: claude-code\n      model: opus\n  defaults:\n    primary: claude-code\n",
        );
        let ran = run_with_flags(&project, &["llm", "list"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("[configured]").contains("[built-in]");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "[label=big].authored"), "true");
            assert_eq!(reports_as(&doc, "[label=claude-code].authored"), "true");
            assert_eq!(reports_as(&doc, "[label=mock].authored"), "false");
        }
    }
}

// ---------------------------------------------------------------- cli/mcp.feature

// mcp.feature: Scenario: (the listing of a bundle's server, j000400 in the
// same feature): the servers a profile's bundles ship are listed with their
// command
#[test]
fn test_mcp_server_list_names_the_servers_the_default_agents_bundles_ship() {
    let project = Project::initialized();
    project.bundle("demo");
    project.write(
        ".ctxloom/content/bundles/v2/demo/mcp/toolserver.yaml",
        "command: j000400-mcp-tool-9e1b52\n",
    );
    project.profile("dev", "demo");
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\nagents:\n  developer:\n    profiles: [dev]\ndefault_agent: developer\n",
    );
    let ran = project.run(&["mcp", "server", "list"]);
    ran.succeeds();
    assert_eq!(
        reports_as(&ran.json(), "servers[name=toolserver].command"),
        "j000400-mcp-tool-9e1b52"
    );
}

// mcp.feature: Scenario Outline: The config-level write leaves do not exist
#[test]
fn test_the_config_level_mcp_write_leaves_do_not_exist() {
    for leaf in ["create", "remove"] {
        let project = Project::initialized();
        project
            .run(&["mcp", "server", leaf, "tools"])
            .fails()
            .contains("unknown command");
    }
}

// mcp.feature: Scenario: A native mcp: block fails the load and names the key
#[test]
fn test_a_native_mcp_block_fails_the_load_and_names_the_key() {
    let project = Project::initialized();
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\nmcp:\n    auto_register_ctxloom: true\n    servers:\n        tools:\n            command: echo\n",
    );
    project
        .run(&["mcp", "server", "list"])
        .fails()
        .contains("unknown key `mcp`");
}

// ---------------------------------------------------------------- cli/companion.feature

// companion.feature: Scenario: A project's companions are added to the home
// registrations
#[test]
fn test_a_projects_companions_are_added_to_the_home_registrations() {
    let project = Project::initialized();
    project.write_home(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: [acme]\n",
    );
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: [beta]\n",
    );
    let ran = project.run(&["companion", "list", "--format", "json"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "0.name"), "acme");
    assert_eq!(reports_as(&doc, "1.name"), "beta");
}

// companion.feature: Scenario: An empty project list does not unregister a
// home companion
#[test]
fn test_an_empty_project_list_does_not_unregister_a_home_companion() {
    let project = Project::initialized();
    project.write_home(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: [acme]\n",
    );
    project.write(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: []\n",
    );
    let ran = project.run(&["companion", "list", "--format", "json"]);
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "0.name"), "acme");
}

// companion.feature: Scenario: List answers which binary each registered
// name resolves to (and Scenario: Bare companion lists the registered
// companions): the binary is `ctxloom-companion-<name>`, looked for on PATH
#[test]
fn test_list_answers_which_binary_each_registered_name_resolves_to() {
    let project = Project::initialized();
    project.write_home(
        ".ctxloom/config.yaml",
        "schema_version: 8\ncompanions: [acme]\n",
    );
    let bin = project.dir().join("bin");
    let exe = bin.join("ctxloom-companion-acme");
    files::put_program(&exe, "#!/bin/sh\n");
    let path = format!(
        "{}:{}",
        bin.display(),
        crate::tests::host_var("PATH")
            .and_then(|v| v.into_string().ok())
            .unwrap_or_default()
    );
    let ran = project.run_with(
        &[("PATH", &path)],
        &["companion", "list", "--format", "json"],
    );
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(reports_as(&doc, "0.name"), "acme");
    assert_eq!(reports_as(&doc, "0.bin"), "ctxloom-companion-acme");
    assert_eq!(reports_as(&doc, "0.resolves"), "true");
    project
        .run_with(&[("PATH", &path)], &["companion"])
        .succeeds()
        .contains("acme")
        .contains("ctxloom-companion-acme")
        .lacks("Available Commands:");
}

// companion.feature: Scenario: A name with nothing on PATH is refused,
// naming the binary it looked for (the listing half: it says none is
// registered)
#[test]
fn test_no_registered_companion_is_said_in_prose() {
    let project = Project::initialized();
    project
        .run(&["companion", "list", "--format", "text"])
        .succeeds()
        .contains("No companions registered");
}

// ---------------------------------------------------------------- cli/search.feature

// search.feature: Scenario Outline: A search with neither a query nor a tag
// is refused, not answered with everything
#[test]
fn test_a_search_with_neither_a_query_nor_a_tag_is_refused() {
    for flags in FORMATS {
        let project = demo_with_testing();
        let ran = run_with_flags(&project, &["search", "--local", "testing"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Results (1):");
        } else {
            assert_eq!(reports_as(&ran.json(), "count"), "1");
        }
        project
            .run(&["search", "--local"])
            .fails()
            .contains("please provide a search query or tags")
            .lacks("Results (");
    }
}

// search.feature: Scenario Outline: A --type nobody recognizes is refused,
// and the message names the vocabulary
#[test]
fn test_an_unknown_search_type_is_refused_naming_the_vocabulary() {
    for flags in FORMATS {
        let project = demo_with_testing();
        let ran = run_with_flags(
            &project,
            &["search", "--local", "--type", "fragment", "testing"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Results (1):");
        } else {
            assert_eq!(reports_as(&ran.json(), "count"), "1");
        }
        project
            .run(&["search", "--local", "--type", "widget", "testing"])
            .fails()
            .contains("unknown type")
            .contains("valid: fragment, command, skill, profile, bundle, mcp_server");
    }
}

// search.feature: Scenario Outline: One query finds matching content of more
// than one kind
#[test]
fn test_one_query_finds_matching_content_of_more_than_one_kind() {
    for flags in FORMATS {
        let project = demo_with_testing();
        project.command("demo", "testing-review");
        let ran = run_with_flags(&project, &["search", "--local", "testing"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Results (2):")
                .contains("- testing")
                .contains("- testing-review");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "count"), "2");
            assert_eq!(reports_as(&doc, "local[name=testing].type"), "fragment");
            assert_eq!(
                reports_as(&doc, "local[name=testing-review].type"),
                "command"
            );
        }
    }
}

// search.feature: Scenario Outline: Restricting to fragments drops the
// command the same query matched; Restricting to commands drops the fragment
#[test]
fn test_restricting_to_a_kind_drops_the_other_kinds() {
    for (kind, survivor) in [("fragment", "testing"), ("command", "testing-review")] {
        let project = demo_with_testing();
        project.command("demo", "testing-review");
        assert_eq!(
            reports_as(
                &project
                    .run(&["search", "--local", "testing"])
                    .succeeds()
                    .json(),
                "count"
            ),
            "2"
        );
        let ran = project.run(&["search", "--local", "--type", kind, "testing"]);
        ran.succeeds();
        let doc = ran.json();
        assert_eq!(reports_as(&doc, "count"), "1");
        assert_eq!(
            reports_as(&doc, &format!("local[name={survivor}].type")),
            kind
        );
    }
}

// search.feature: Scenario Outline: Restricting to profiles finds a
// composition and nothing else
#[test]
fn test_restricting_to_profiles_finds_a_composition_and_nothing_else() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        project.fragment("demo", "dev-notes");
        project.profile("dev", "demo");
        assert_eq!(
            reports_as(
                &project.run(&["search", "--local", "dev"]).succeeds().json(),
                "count"
            ),
            "2"
        );
        let ran = run_with_flags(
            &project,
            &["search", "--local", "--type", "profile", "dev"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Results (1):").contains("- dev");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "count"), "1");
            assert_eq!(reports_as(&doc, "local[name=dev].type"), "profile");
        }
    }
}

// search.feature: Scenario Outline: A tag search returns what carries the tag
// and leaves out what does not
#[test]
fn test_a_tag_search_returns_what_carries_the_tag() {
    for flags in FORMATS {
        let project = demo_with_testing();
        let by_name = project.run(&["search", "--local", "testing"]);
        assert_eq!(
            reports_as(&by_name.succeeds().json(), "local[name=testing].type"),
            "fragment"
        );
        let ran = run_with_flags(&project, &["search", "--local", "--tag", "example"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Results (1):").contains("- example [example]");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "count"), "1");
            assert!(contains_value(&doc, "local[name=example].tags", "example"));
        }
    }
}

// ---------------------------------------------------------------- cli/run.feature

// run.feature: Scenario: An agent binding and a hand-assembled profile cannot
// both decide the context
#[test]
fn test_an_agent_and_a_profile_cannot_both_decide_the_context() {
    let project = Project::initialized();
    project.bundle("dev-base");
    project.profile("dev", "dev-base");
    let ran = project.run(&[
        "run",
        "--dry-run",
        "--agent",
        "developer",
        "--profile",
        "dev",
        "hello",
    ]);
    assert_eq!(
        ran.code,
        2,
        "a usage refusal is exit 2 (D28): {}",
        ran.said()
    );
    ran.contains("none of the others can be");
}

// run.feature: Scenario: A dry run shows the composed context without
// launching a model
#[test]
fn test_a_dry_run_shows_the_composed_context_without_launching_a_model() {
    let project = demo_with_testing();
    project.profile("dev", "demo");
    let ran = project.run(&["run", "--dry-run", "--profile", "dev", "hello"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(doc["profiles"], serde_json::json!(["dev"]));
    assert!(
        contains_value(&doc, "fragments", "ctxloom+local:demo#fragments/testing"),
        "{doc}"
    );
    assert!(doc["context"]
        .as_str()
        .unwrap()
        .contains("FRAGMENT-BODY-testing"));
    let text = project.run(&[
        "run",
        "--dry-run",
        "--profile",
        "dev",
        "hello",
        "--format",
        "text",
    ]);
    text.succeeds()
        .contains("dev")
        .contains("demo#fragments/testing")
        .contains("FRAGMENT-BODY-testing");
}

// run.feature: Scenario: An explicit tag selection matching nothing fails
// loudly, not silently
#[test]
fn test_an_explicit_tag_selection_matching_nothing_fails_loudly() {
    let project = demo_with_testing();
    project
        .run(&["run", "--dry-run", "-t", "no-such-tag-anywhere", "hello"])
        .fails()
        .contains("no-such-tag-anywhere");
}

// run.feature: (outside --dry-run the launch is S6's: this build says so)
// the run without --dry-run is refused, not silently a dry run
#[test]
fn test_a_run_that_is_not_a_dry_run_is_refused_until_the_launch_arrives() {
    let project = demo_with_testing();
    project.profile("dev", "demo");
    let ran = project.run(&["run", "--profile", "dev", "hello"]);
    assert_eq!(ran.code, 2, "{}", ran.said());
    ran.contains("--dry-run");
}

// ---------------------------------------------------------------- fault_tolerance.feature

// fault_tolerance.feature: Scenario: A malformed config is refused by name,
// not read around
#[test]
fn test_a_malformed_config_is_refused_by_name() {
    let project = Project::initialized();
    project.bundle("demo");
    project.write(".ctxloom/config.yaml", "::: not: [valid yaml\n\tbroken");
    project
        .run(&["bundle", "list"])
        .fails()
        .contains("cannot be parsed")
        .contains("config.yaml");
}

// fault_tolerance.feature: Scenario: A profile referencing a missing bundle
// is refused like a run; --degraded previews past it
#[test]
fn test_a_profile_referencing_a_missing_bundle_is_refused_like_a_run() {
    let project = Project::initialized();
    project.write(
        ".ctxloom/content/bundles/v2/project/profiles/broken.yaml",
        "description: references a missing bundle\nbundles:\n  - does-not-exist\n",
    );
    project.write(
        ".ctxloom/content/bundles/v2/project/bundle.yaml",
        "schema_version: 4\nversion: 1.0.0\n",
    );
    let token = [("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-fixture")];
    let refused = project.run_with(
        &token,
        &["run", "--dry-run", "--profile", "broken", "hello"],
    );
    assert_eq!(refused.code, 3, "{}", refused.said());
    refused
        .contains("failed to load bundle")
        .contains("does-not-exist");
    let degraded = project.run_with(
        &token,
        &[
            "--degraded",
            "run",
            "--dry-run",
            "--profile",
            "broken",
            "hello",
        ],
    );
    degraded
        .succeeds()
        .contains("warning")
        .contains("does-not-exist");
}

// fault_tolerance.feature: Scenario: An unknown fragment reference fails cleanly
#[test]
fn test_an_unknown_fragment_reference_fails_cleanly() {
    let project = Project::initialized();
    project.bundle("demo");
    project
        .run(&["fragment", "show", "demo#fragments/does-not-exist"])
        .fails()
        .contains("item not found")
        .contains("Available fragments: example");
}

// fault_tolerance.feature: Scenario: A garbage reference fails cleanly
#[test]
fn test_a_garbage_reference_fails_cleanly() {
    let project = Project::initialized();
    project.bundle("demo");
    project
        .run(&["fragment", "show", "not-a-real-reference"])
        .fails()
        .contains("invalid reference format: expected bundle#fragments/name");
}

// ---------------------------------------------------------------- environment.md: finding the project

// docs/environment.md (CTXLOOM_ROOT) and configload/appdir.go: with no
// CTXLOOM_ROOT the project is the nearest .ctxloom up from the working
// directory; nothing is created when a command only reads.
#[test]
fn test_the_project_is_the_nearest_ctxloom_up_from_the_working_directory() {
    let project = Project::initialized();
    project.bundle("demo");
    let deeper = project.dir().join("sub/deeper");
    files::mkdirs(&deeper);
    let ran = project.run_in(&deeper, &["bundle", "list", "--format", "json"]);
    ran.succeeds();
    assert_eq!(reports_as(&ran.json(), "[name=demo].version"), "1.0.0");
    assert!(
        !files::exists(deeper.join(".ctxloom")),
        "reading creates nothing"
    );
}

// docs/environment.md: the minimal config is all a project needs
#[test]
fn test_the_minimal_config_is_enough() {
    let project = Project::bare();
    project.write(".ctxloom/config.yaml", MINIMAL_CONFIG);
    project
        .run(&["bundle", "list", "--format", "json"])
        .succeeds();
}

// ---------------------------------------------------------------- context assemble

// There is no Go CLI for `context assemble`: Go serves it as the MCP tool
// `assemble_context` over `operations.AssembleContext`, whose behaviour these
// hold the command to (operations/premise_test.go: TestAssembleContext_
// ExplicitAskLoadsPremisedFragment, _UnknownAskIsReportedMissing,
// _PremisedFragmentIsWithheldAndIndexed; operations/context.go
// refuseEmptySelection). The command's own shape (the verb, `-p -f -t`, the
// result's fields) is a question for the owner, in the S1 report.
#[test]
fn test_context_assemble_holds_a_premised_fragment_back_unless_it_is_named() {
    let project = Project::initialized();
    project.bundle("demo");
    project.write(
        ".ctxloom/content/bundles/v2/demo/fragments/golang.md",
        "---\ndescription: when editing Go code\n---\nGO-RULES-BODY\n",
    );
    project.fragment("demo", "testing");
    project.profile("dev", "demo");
    let by_profile = project.run(&["context", "assemble", "-p", "dev", "--format", "json"]);
    by_profile.succeeds();
    let doc = by_profile.json();
    assert_eq!(doc["profiles"], serde_json::json!(["dev"]));
    let context = doc["context"].as_str().unwrap();
    assert!(context.contains("FRAGMENT-BODY-testing"), "{doc}");
    assert!(
        !context.contains("GO-RULES-BODY"),
        "a premised fragment is held back: {doc}"
    );

    let named = project.run(&[
        "context",
        "assemble",
        "-f",
        "demo#fragments/golang",
        "--format",
        "json",
    ]);
    named.succeeds();
    assert!(named.json()["context"]
        .as_str()
        .unwrap()
        .contains("GO-RULES-BODY"));
}

// premise_test.go: an unknown ask is reported missing, not silently empty; an
// ask that loads nothing at all is refused (context.go refuseEmptySelection).
#[test]
fn test_context_assemble_reports_a_missing_ask_and_refuses_one_that_loads_nothing() {
    let project = demo_with_testing();
    let partly = project.run(&[
        "context",
        "assemble",
        "-f",
        "demo#fragments/testing",
        "-f",
        "demo#fragments/nope",
        "--format",
        "json",
    ]);
    partly.succeeds();
    let doc = partly.json();
    assert_eq!(
        doc["missing_fragments"],
        serde_json::json!(["ctxloom+local:demo#fragments/nope"])
    );
    assert!(doc["fragments_loaded"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f == "ctxloom+local:demo#fragments/testing"));
    project
        .run(&["context", "assemble", "-f", "nosuch"])
        .fails()
        .contains("requested fragments not found: nosuch");
    project
        .run(&["context", "assemble", "-t", "no-such-tag-anywhere"])
        .fails()
        .contains("no fragment matches tag(s): no-such-tag-anywhere");
}
