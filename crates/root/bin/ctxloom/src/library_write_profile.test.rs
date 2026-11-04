//! Go's acceptance scenarios for the profile noun's write side (S2),
//! `tests/acceptance/features/cli/profile.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. The bytes and answers are Go's own
//! (captured from a Go ctxloom run in a scratch project).
//!
//! `ctxloom materialize` is S9's: where a scenario reads the assembled
//! context out of a materialized file, the assembled context is read from
//! `context assemble` instead (the same composition, S1's command), and the
//! scenario's title says so. Go's create, modify, edit, export and import
//! answer in text only (its format debt): an explicit `--format json` is
//! refused before anything is written, as Go refuses it.

use super::library::{contains_value, reports_as, Editor, Project};

const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn with_flags(project: &Project, base: &[&str], flags: &[&str]) -> super::Ran {
    let mut args = base.to_vec();
    args.extend_from_slice(flags);
    project.run(&args)
}

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

const PROFILES: &str = ".ctxloom/content/bundles/v2/project/profiles";

const DEV_YAML: &str = ".ctxloom/content/bundles/v2/project/profiles/dev.yaml";

/// Go's `a bundle "demo" exists` and `a profile "dev" with bundle "demo"`.
fn demo_and_dev() -> Project {
    let project = Project::initialized();
    project.bundle("demo");
    project.profile("dev", "demo");
    project
}

/// The assembled context of `profile`.
fn assembled(project: &Project, profile: &str) -> String {
    let ran = project.run(&["context", "assemble", "-p", profile]);
    ran.succeeds();
    ran.stdout.clone()
}

// ---------------------------------------------------------------- create

// profile.feature: Scenario Outline: Creating a profile writes its definition
// and it is listed back
#[test]
fn test_creating_a_profile_writes_its_definition_and_it_is_listed_back() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("demo");
        project
            .run(&[
                "profile",
                "create",
                "dev",
                "--include",
                "demo",
                "-d",
                "day-to-day-work",
            ])
            .succeeds();
        // Go's bytes; the project bundle that holds profiles is made too.
        assert_eq!(
            project.read(DEV_YAML).as_deref(),
            Some("description: day-to-day-work\nbundles:\n  - demo\n")
        );
        assert_eq!(
            project
                .read(".ctxloom/content/bundles/v2/project/bundle.yaml")
                .as_deref(),
            Some("schema_version: 4\nversion: 1.0.0\n")
        );
        let ran = with_flags(&project, &["profile", "list"], flags);
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

// cli/profile.go create (documented condition): what the command says, the
// keys it writes in the order Go writes them, and what it will not write.
#[test]
fn test_create_says_what_it_wrote_and_refuses_what_it_must() {
    let project = Project::initialized();
    project.bundle("demo");
    let ran = project.run(&[
        "profile",
        "create",
        "base",
        "--include",
        "demo",
        "-d",
        "base d",
    ]);
    ran.succeeds();
    assert_eq!(
        ran.stdout,
        format!(
            "Created profile \"base\" with bundles: demo\nSaved to: {}\n",
            project.dir().join(PROFILES).join("base.yaml").display()
        )
    );
    let ran = project.run(&[
        "profile",
        "create",
        "dev",
        "--parent",
        "base",
        "--include",
        "demo",
        "--llm",
        "claude-fast",
        "-d",
        "day-to-day-work",
    ]);
    ran.succeeds()
        .contains("Created profile \"dev\" with parents: base; bundles: demo");
    assert_eq!(
        project.read(DEV_YAML).as_deref(),
        Some("description: day-to-day-work\nparents:\n  - base\nllm: claude-fast\nbundles:\n  - demo\n")
    );
    project
        .run(&["profile", "create", "only-parent", "--parent", "base"])
        .succeeds()
        .contains("Created profile \"only-parent\" with parents: base\n");
    let before = project.tree(PROFILES);
    project
        .run(&["profile", "create", "dev", "--include", "demo"])
        .fails()
        .contains("profile \"dev\" already exists");
    project
        .run(&["profile", "create", "bad#name", "--include", "demo"])
        .fails()
        .contains("a bundle profile is addressed as <bundle>#profiles/<name>");
    project
        .run(&["profile", "create", "a/b", "--include", "demo"])
        .fails()
        .contains("a profile name is a single path segment");
    project
        .run(&[
            "profile",
            "create",
            "x",
            "--include",
            "demo",
            "--llm",
            "zzz",
        ])
        .fails()
        .contains("unknown LLM \"zzz\"");
    assert_eq!(
        project.tree(PROFILES),
        before,
        "a refused create wrote something"
    );
    project
        .run(&[
            "profile",
            "create",
            "dev9",
            "--include",
            "demo",
            "--format",
            "json",
        ])
        .fails()
        .contains("does not support it yet");
    project.file_is_absent(&format!("{PROFILES}/dev9.yaml"));
}

// profile.feature: Scenario: A profile that names neither a bundle nor a
// parent is refused
#[test]
fn test_a_profile_that_names_neither_a_bundle_nor_a_parent_is_refused() {
    let project = Project::initialized();
    project.bundle("demo");
    project
        .run(&["profile", "create", "dev", "--include", "demo"])
        .succeeds();
    project.file_is_absent(&format!("{PROFILES}/hollow.yaml"));
    assert!(project.exists(DEV_YAML));
    project
        .run(&["profile", "create", "hollow"])
        .fails()
        .contains("at least one parent (--parent) or included bundle (--include) is required");
    project.file_is_absent(&format!("{PROFILES}/hollow.yaml"));
}

// profile.feature: Scenario: Inheriting from a profile that does not exist is
// refused at creation
#[test]
fn test_inheriting_from_a_profile_that_does_not_exist_is_refused_at_creation() {
    let project = demo_and_dev();
    project
        .run(&["profile", "create", "base", "--include", "demo"])
        .succeeds();
    project
        .run(&[
            "profile",
            "create",
            "good",
            "--parent",
            "base",
            "--include",
            "demo",
        ])
        .succeeds();
    assert!(project.exists(&format!("{PROFILES}/good.yaml")));
    project
        .run(&[
            "profile",
            "create",
            "dev2",
            "--parent",
            "no-such-base",
            "--include",
            "demo",
        ])
        .fails()
        .contains("parent profile \"no-such-base\" not found");
    project.file_is_absent(&format!("{PROFILES}/dev2.yaml"));
}

// ---------------------------------------------------------------- modify

// profile.feature: Scenario: A bundle is attached to a profile and detached
// again, and the assembled context follows (read from `context assemble`)
#[test]
fn test_a_bundle_is_attached_to_a_profile_and_detached_again_and_the_assembled_context_follows() {
    let project = Project::initialized();
    project.bundle("house-style");
    project.fragment("house-style", "house-rules");
    project.bundle("demo");
    project.fragment("demo", "testing");
    project.profile("dev", "demo");
    let ran = project.run(&["profile", "modify", "dev", "--add-bundle", "house-style"]);
    ran.succeeds();
    assert_eq!(
        ran.stdout,
        "added bundle: house-style\nModified profile \"dev\"\n"
    );
    let attached = assembled(&project, "dev");
    assert!(attached.contains("FRAGMENT-BODY-house-rules"), "{attached}");
    assert!(attached.contains("FRAGMENT-BODY-testing"), "{attached}");
    project
        .run(&["profile", "modify", "dev", "--remove-bundle", "house-style"])
        .succeeds()
        .contains("removed bundle: house-style");
    let detached = assembled(&project, "dev");
    assert!(
        !detached.contains("FRAGMENT-BODY-house-rules"),
        "{detached}"
    );
    assert!(detached.contains("FRAGMENT-BODY-testing"), "{detached}");
}

// profile.feature: Scenario Outline: A parent profile's content is inherited,
// and detaching the parent takes it away (read from `context assemble`)
#[test]
fn test_a_parent_profiles_content_is_inherited_and_detaching_the_parent_takes_it_away() {
    for flags in FORMATS {
        let project = Project::initialized();
        project.bundle("house-style");
        project.fragment("house-style", "house-rules");
        project.profile("base", "house-style");
        project.bundle("demo");
        project.fragment("demo", "testing");
        project
            .run(&[
                "profile",
                "create",
                "dev",
                "--parent",
                "base",
                "--include",
                "demo",
                "-d",
                "day-to-day-work",
            ])
            .succeeds();
        let shown = with_flags(&project, &["profile", "show", "dev"], flags);
        shown.succeeds();
        if is_text(flags) {
            shown.contains("- base");
        } else {
            assert!(contains_value(&shown.json(), "parents", "base"));
        }
        let inherited = assembled(&project, "dev");
        assert!(
            inherited.contains("FRAGMENT-BODY-house-rules"),
            "{inherited}"
        );
        assert!(inherited.contains("FRAGMENT-BODY-testing"), "{inherited}");
        project
            .run(&["profile", "modify", "dev", "--remove-parent", "base"])
            .succeeds();
        let standalone = assembled(&project, "dev");
        assert!(
            !standalone.contains("FRAGMENT-BODY-house-rules"),
            "{standalone}"
        );
        assert!(standalone.contains("FRAGMENT-BODY-testing"), "{standalone}");
    }
}

// profile.feature: Scenario: A modify that was told to change nothing says so
#[test]
fn test_a_modify_that_was_told_to_change_nothing_says_so() {
    let project = demo_and_dev();
    let before = project.tree(PROFILES);
    project
        .run(&["profile", "modify", "dev"])
        .succeeds()
        .contains("No changes made.");
    project.file_contains(DEV_YAML, "- demo");
    // Flags that restate what is there, or take out what is not, change
    // nothing either.
    for args in [
        vec!["--add-bundle", "demo"],
        vec!["--remove-bundle", "nope"],
        vec!["--remove-parent", "nope"],
        vec!["--include-fragment", "nope"],
        vec!["--include-mcp", "nope"],
    ] {
        let mut all = vec!["profile", "modify", "dev"];
        all.extend(args);
        let ran = project.run(&all);
        assert_eq!(ran.stdout, "No changes made.\n", "{all:?}");
    }
    assert_eq!(project.tree(PROFILES), before);
    project
        .run(&["profile", "modify", "nosuch", "-d", "x"])
        .fails()
        .contains("profile not found: nosuch");
    project
        .run(&["profile", "modify", "dev", "--add-parent", "nosuch"])
        .fails()
        .contains("parent profile \"nosuch\" not found");
    project
        .run(&["profile", "modify", "dev", "--llm", "zzz"])
        .fails()
        .contains("unknown LLM \"zzz\"");
    assert_eq!(
        project.tree(PROFILES),
        before,
        "a refused modify wrote something"
    );
}

// profile.feature: Scenario: A fragment is masked out of the assembled
// context and let back in (read from `context assemble`)
#[test]
fn test_a_fragment_is_masked_out_of_the_assembled_context_and_let_back_in() {
    let project = Project::initialized();
    project.bundle("demo");
    project.fragment("demo", "testing");
    project.profile("dev", "demo");
    assert!(assembled(&project, "dev").contains("FRAGMENT-BODY-testing"));
    project
        .run(&["profile", "modify", "dev", "--exclude-fragment", "testing"])
        .succeeds()
        .contains("added exclude fragment: testing");
    project.file_contains(DEV_YAML, "exclude_fragments");
    let masked = assembled(&project, "dev");
    assert!(!masked.contains("FRAGMENT-BODY-testing"), "{masked}");
    assert!(masked.contains("Add your content here."), "{masked}");
    project
        .run(&["profile", "modify", "dev", "--include-fragment", "testing"])
        .succeeds()
        .contains("removed exclude fragment: testing");
    assert!(assembled(&project, "dev").contains("FRAGMENT-BODY-testing"));
}

// profile.feature: Scenario: An MCP server is masked out of the materialized
// registry and let back in (the profile's half: the veto and its lifting;
// .mcp.json is S9's)
#[test]
fn test_an_mcp_server_is_masked_out_of_the_registry_and_let_back_in() {
    let project = Project::initialized();
    project.bundle("demo");
    project
        .run(&[
            "bundle",
            "edit",
            "demo",
            "--add-mcp",
            "tree-sitter",
            "--add-mcp",
            "thinker",
        ])
        .succeeds();
    project.profile("dev", "demo");
    project
        .run(&["profile", "modify", "dev", "--exclude-mcp", "tree-sitter"])
        .succeeds()
        .contains("added exclude mcp: tree-sitter");
    project.file_contains(DEV_YAML, "exclude_mcp:\n  - tree-sitter\n");
    project
        .run(&["profile", "modify", "dev", "--include-mcp", "tree-sitter"])
        .succeeds()
        .contains("removed exclude mcp: tree-sitter");
    project.file_lacks(DEV_YAML, "exclude_mcp");
}

// profile.feature: Scenario: Editing a profile in the configured editor
// round-trips into the file
#[test]
fn test_editing_a_profile_in_the_configured_editor_round_trips_into_the_file() {
    let project = Project::with_editor(Editor::Description);
    project.bundle("demo");
    project.profile("dev", "demo");
    let ran = project.run(&["profile", "edit", "dev"]);
    ran.succeeds().contains("Updated profile");
    project.file_contains(DEV_YAML, "EDITED-BY-TEST");
    project
        .run(&["profile", "show", "dev"])
        .succeeds()
        .contains("EDITED-BY-TEST");
    // An editor that changes nothing says so; one that writes a profile the
    // schema refuses is refused and the file survives.
    let plain = Project::initialized();
    plain.bundle("demo");
    plain.profile("dev", "demo");
    let before = plain.read(DEV_YAML);
    assert_eq!(
        plain.run(&["profile", "edit", "dev"]).stdout,
        "No changes made.\n"
    );
    assert_eq!(plain.read(DEV_YAML), before);
    let marker = Project::with_editor(Editor::Marker);
    marker.bundle("demo");
    marker.profile("dev", "demo");
    let before = marker.read(DEV_YAML);
    marker.run(&["profile", "edit", "dev"]).fails();
    assert_eq!(
        marker.read(DEV_YAML),
        before,
        "a refused edit rewrote the profile"
    );
}

// profile.feature: Scenario Outline: The description can be set by flag
// instead
#[test]
fn test_the_description_can_be_set_by_flag_instead() {
    for flags in FORMATS {
        let project = demo_and_dev();
        project.file_contains(DEV_YAML, "acceptance fixture profile");
        project
            .run(&["profile", "modify", "dev", "-d", "updated-desc"])
            .succeeds();
        project
            .file_contains(DEV_YAML, "updated-desc")
            .file_lacks(DEV_YAML, "acceptance fixture profile");
        let ran = with_flags(&project, &["profile", "show", "dev"], flags);
        if is_text(flags) {
            ran.contains("updated-desc");
        } else {
            assert_eq!(reports_as(&ran.json(), "description"), "updated-desc");
        }
    }
}

// profile.feature: Scenario: The preferred engine can be set by flag
#[test]
fn test_the_preferred_engine_can_be_set_by_flag() {
    let project = demo_and_dev();
    project
        .run(&["llm", "create", "big", "--type", "mock", "--model", "o1"])
        .succeeds();
    project
        .run(&["profile", "modify", "dev", "--llm", "big"])
        .succeeds()
        .contains("set llm to \"big\"");
    project.file_contains(DEV_YAML, "llm: big");
}

// cli/profile.go modify (documented condition): every change it makes is
// named, in Go's order; the rest of the file, comments included, is as the
// author left it.
#[test]
fn test_modify_names_each_change_and_leaves_the_rest_of_the_file_alone() {
    let project = Project::initialized();
    project.bundle("demo");
    project.profile("dev", "demo");
    project.write(
        DEV_YAML,
        "# my own profile\ndescription: mine\nbundles:\n  - demo # the base\n",
    );
    project.write(&format!("{PROFILES}/dev3.yaml"), "bundles:\n  - demo\n");
    let ran = project.run(&[
        "profile",
        "modify",
        "dev",
        "--add-bundle",
        "demo2",
        "--add-bundle",
        "demo3",
        "--exclude-fragment",
        "f1",
        "--exclude-fragment",
        "f2",
        "--exclude-mcp",
        "m1",
        "--add-parent",
        "dev3",
    ]);
    ran.succeeds();
    assert_eq!(
        ran.stdout,
        "added parent: dev3\nadded bundle: demo2\nadded bundle: demo3\nadded exclude fragment: f1\nadded exclude fragment: f2\nadded exclude mcp: m1\nModified profile \"dev\"\n"
    );
    project.file_contains(DEV_YAML, "# my own profile");
    project.file_contains(DEV_YAML, "  - demo # the base");
    let ran = project.run(&[
        "profile",
        "modify",
        "dev",
        "--remove-bundle",
        "demo2",
        "--remove-parent",
        "dev3",
        "--include-fragment",
        "f1",
        "--include-mcp",
        "m1",
        "-d",
        "new d",
    ]);
    assert_eq!(
        ran.stdout,
        "updated description\nremoved parent: dev3\nremoved bundle: demo2\nremoved exclude fragment: f1\nremoved exclude mcp: m1\nModified profile \"dev\"\n"
    );
    project
        .file_contains(DEV_YAML, "# my own profile")
        .file_contains(DEV_YAML, "description: new d")
        .file_lacks(DEV_YAML, "m1")
        .file_lacks(DEV_YAML, "demo2")
        .file_contains(DEV_YAML, "  - f2");
    project
        .run(&["profile", "modify", "dev", "--format", "json", "-d", "x"])
        .fails()
        .contains("does not support it yet");
    project.file_contains(DEV_YAML, "description: new d");
}

// ---------------------------------------------------------------- export, import

// profile.feature: Scenario Outline: Export, remove, and re-import a profile
#[test]
fn test_export_remove_and_re_import_a_profile() {
    for flags in FORMATS {
        let project = demo_and_dev();
        let exported = project.run(&["profile", "export", "dev", "pexport"]);
        exported.succeeds();
        assert_eq!(
            exported.stdout,
            format!(
                "Exported: {} -> pexport/dev.yaml\n",
                project.dir().join(DEV_YAML).display()
            )
        );
        project.file_contains("pexport/dev.yaml", "- demo");
        project
            .run(&["profile", "remove", "dev", "--yes"])
            .succeeds();
        project.file_is_absent(DEV_YAML);
        let imported = project.run(&["profile", "import", "pexport/dev.yaml", "-f"]);
        imported.succeeds();
        assert_eq!(
            imported.stdout,
            format!(
                "Imported: pexport/dev.yaml -> {}\n",
                project.dir().join(DEV_YAML).display()
            )
        );
        let listed = with_flags(&project, &["profile", "list"], flags);
        let shown = with_flags(&project, &["profile", "show", "dev"], flags);
        if is_text(flags) {
            listed.contains("dev");
            shown.contains("- demo");
        } else {
            assert_eq!(reports_as(&listed.json(), "$.0.name"), "dev");
            assert!(contains_value(&shown.json(), "bundles", "demo"));
        }
        assert_eq!(
            project.read("pexport/dev.yaml"),
            project.read(DEV_YAML),
            "the definition changed in transit"
        );
    }
}

// profile.feature: Scenario: Importing over an existing profile refuses, and
// the local edit survives
#[test]
fn test_importing_over_an_existing_profile_refuses_and_the_local_edit_survives() {
    let project = demo_and_dev();
    project
        .run(&["profile", "export", "dev", "pexport"])
        .succeeds();
    project
        .run(&["profile", "modify", "dev", "-d", "LOCAL-EDIT-MARKER"])
        .succeeds();
    let before = project.tree(PROFILES);
    project
        .run(&["profile", "import", "pexport/dev.yaml"])
        .fails()
        .contains("profile already exists")
        .contains("use --force to overwrite");
    project.file_contains(DEV_YAML, "LOCAL-EDIT-MARKER");
    assert_eq!(project.tree(PROFILES), before);
    project
        .run(&["profile", "import", "pexport/dev.yaml", "--force"])
        .succeeds();
    project.file_lacks(DEV_YAML, "LOCAL-EDIT-MARKER");
}

// cli/profile.go export/import (documented condition): an export needs a
// destination, an import needs a profile the schema accepts, and neither
// writes what it refuses.
#[test]
fn test_export_and_import_refuse_what_they_must() {
    let project = demo_and_dev();
    project
        .run(&["profile", "export", "nosuch", "out"])
        .fails()
        .contains("profile not found: nosuch");
    project.run(&["profile", "export", "dev"]).fails();
    project.write("bad.yaml", "bundles: [demo]\nnot_a_profile_key: 1\n");
    let before = project.tree(PROFILES);
    project
        .run(&["profile", "import", "bad.yaml"])
        .fails()
        .contains("not_a_profile_key");
    project
        .run(&["profile", "import", "nosuch.yaml"])
        .fails()
        .contains("nosuch.yaml");
    assert_eq!(project.tree(PROFILES), before);
}

// profile.feature: Scenario: Export carries the definition and materialize
// carries the assembled context (the export half: the definition, and not
// the content it composes)
#[test]
fn test_export_carries_the_definition() {
    let project = Project::initialized();
    project.bundle("demo");
    project.fragment("demo", "testing");
    project.profile("dev", "demo");
    project
        .run(&["profile", "export", "dev", "definition"])
        .succeeds();
    project
        .file_contains("definition/dev.yaml", "bundles:")
        .file_contains("definition/dev.yaml", "- demo")
        .file_lacks("definition/dev.yaml", "FRAGMENT-BODY-testing");
}

// ---------------------------------------------------------------- remove

// profile.feature: Scenario Outline: Bare profile remove reports what would
// go and destroys nothing
#[test]
fn test_bare_profile_remove_reports_what_would_go_and_destroys_nothing() {
    for flags in FORMATS {
        let project = demo_and_dev();
        let before = project.tree(".ctxloom");
        let ran = with_flags(&project, &["profile", "remove", "dev"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Would remove profile \"dev\":\n  1 bundle(s)\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom profile remove dev --yes\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "applied"), "false");
            assert_eq!(reports_as(&doc, "detail.0"), "1 bundle(s)");
            assert_eq!(
                reports_as(&doc, "apply"),
                "ctxloom profile remove dev --yes"
            );
        }
        project.file_contains(DEV_YAML, "- demo");
        assert_eq!(project.tree(".ctxloom"), before);
        let listed = with_flags(&project, &["profile", "list"], flags);
        if !is_text(flags) {
            assert_eq!(reports_as(&listed.json(), "$.0.name"), "dev");
        }
    }
}

// profile.feature: Scenario: Removing a profile with --yes takes the file and
// the listing entry
#[test]
fn test_removing_a_profile_with_yes_takes_exactly_the_file_and_the_listing_entry() {
    for flags in FORMATS {
        let project = demo_and_dev();
        project.write(&format!("{PROFILES}/other.yaml"), "bundles:\n  - demo\n");
        let other = project.read(&format!("{PROFILES}/other.yaml"));
        let ran = with_flags(&project, &["profile", "remove", "dev", "--yes"], flags);
        ran.succeeds();
        if !is_text(flags) {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "deleted");
            assert_eq!(reports_as(&doc, "profile"), "dev");
        } else {
            assert_eq!(ran.stdout, "Removed profile \"dev\"\n");
        }
        project.file_is_absent(DEV_YAML);
        assert_eq!(project.read(&format!("{PROFILES}/other.yaml")), other);
    }
    let project = demo_and_dev();
    project
        .run(&["profile", "remove", "dev", "--yes"])
        .succeeds();
    project
        .run(&["profile", "list", "--format", "text"])
        .succeeds()
        .contains("No profiles defined.");
    project
        .run(&["profile", "remove", "dev", "--yes"])
        .fails()
        .contains("profile not found: dev");
}

// profile.feature: Scenario Outline: The rm alias previews and destroys
// exactly as remove does
#[test]
fn test_the_profile_rm_alias_previews_and_destroys_exactly_as_remove_does() {
    for flags in FORMATS {
        let project = demo_and_dev();
        let ran = with_flags(&project, &["profile", "rm", "dev"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Nothing was removed. Re-run with --yes to apply:");
        } else {
            assert_eq!(reports_as(&ran.json(), "applied"), "false");
        }
        project.file_contains(DEV_YAML, "- demo");
        project.run(&["profile", "del", "dev", "--yes"]).succeeds();
        project.file_is_absent(DEV_YAML);
    }
}
