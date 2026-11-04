//! Go's acceptance scenarios for the bundle noun's write side (S2),
//! `tests/acceptance/features/cli/bundle.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. The bytes the write commands leave
//! are Go's own (captured from a Go ctxloom run in a scratch project: the
//! envelope, the example fragment and the example command).
//!
//! Not here: `bundle push`, `bundle move` and `deps hold` (S3), `bundle
//! distill` (S7).

use super::library::{entries, reports_as, Project};

/// An outline's `<flags>` column: the three rows every `@json-default` table has.
const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn with_flags(project: &Project, base: &[&str], flags: &[&str]) -> super::Ran {
    let mut args = base.to_vec();
    args.extend_from_slice(flags);
    project.run(&args)
}

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

const DEMO: &str = ".ctxloom/content/bundles/v2/demo";

/// Go's `a bundle "demo" exists`: `ctxloom bundle create demo -d "acceptance
/// fixture bundle"`, through the binary this time.
fn demo() -> Project {
    let project = Project::initialized();
    project
        .run(&[
            "bundle",
            "create",
            "demo",
            "-d",
            "acceptance fixture bundle",
        ])
        .succeeds();
    project
}

// ---------------------------------------------------------------- creating

// bundle.feature: Scenario: Creating a bundle writes a manifest carrying its
// seeded content
#[test]
fn test_creating_a_bundle_writes_a_manifest_carrying_its_seeded_content() {
    let project = Project::initialized();
    project
        .run(&[
            "bundle",
            "create",
            "demo",
            "-d",
            "acceptance-fixture-bundle",
        ])
        .succeeds();
    // The bytes Go writes (captured from a Go run).
    assert_eq!(
        project.read(&format!("{DEMO}/bundle.yaml")).as_deref(),
        Some("schema_version: 4\nversion: 1.0.0\ndescription: acceptance-fixture-bundle\n")
    );
    assert_eq!(
        project.read(&format!("{DEMO}/fragments/example.md")).as_deref(),
        Some("---\ntags:\n  - example\nno_distill: true\n---\n# Example Fragment\n\nAdd your content here.")
    );
    assert_eq!(
        project.read(&format!("{DEMO}/prompts/example.md")).as_deref(),
        Some("---\ndescription: Example prompt\ntags:\n  - example\nno_distill: true\n---\nExample prompt content. Describe what this prompt does.")
    );
    project.run(&["bundle", "list"]).succeeds().contains("demo");
}

// operations.CreateBundle (documented condition): the answer in each format.
#[test]
fn test_creating_a_bundle_answers_in_json_and_in_text() {
    for flags in FORMATS {
        let project = Project::initialized();
        let ran = with_flags(&project, &["bundle", "create", "demo", "-d", "x"], flags);
        ran.succeeds();
        let manifest = project.dir().join(DEMO).join("bundle.yaml");
        let manifest = manifest.to_string_lossy().into_owned();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                format!("Created bundle: {manifest}\nEdit its item files to add your fragments and prompts.\n")
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "created");
            assert_eq!(reports_as(&doc, "name"), "demo");
            assert_eq!(reports_as(&doc, "path"), manifest);
        }
    }
}

// operations.CreateBundle (documented condition): create refuses an existing
// bundle and leaves it as it was.
#[test]
fn test_creating_a_bundle_that_exists_is_refused_and_leaves_it_alone() {
    let project = demo();
    project.write(&format!("{DEMO}/fragments/mine.md"), "MINE");
    let ran = project.run(&["bundle", "create", "demo", "-d", "other"]);
    ran.fails().contains("bundle already exists");
    assert_eq!(
        project.read(&format!("{DEMO}/bundle.yaml")).as_deref(),
        Some("schema_version: 4\nversion: 1.0.0\ndescription: acceptance fixture bundle\n")
    );
    assert_eq!(
        project
            .read(&format!("{DEMO}/fragments/mine.md"))
            .as_deref(),
        Some("MINE")
    );
}

// operations.CreateBundle (documented condition): a slash separated name
// lands in a nested directory; a name that climbs out is refused and writes
// nothing.
#[test]
fn test_a_bundle_name_may_be_nested_but_never_climbs_out() {
    let project = Project::initialized();
    project.run(&["bundle", "create", "team/rules"]).succeeds();
    assert!(project.exists(".ctxloom/content/bundles/v2/team/rules/bundle.yaml"));
    for bad in ["../evil", "/abs", "a/../b"] {
        let ran = project.run(&["bundle", "create", bad]);
        ran.fails().contains("invalid bundle name");
    }
    assert!(!project.exists(".ctxloom/content/bundles/evil"));
    assert!(!project.exists(".ctxloom/content/bundles/v2/b"));
}

// ---------------------------------------------------------------- editing

// bundle.feature: Scenario Outline: The listing summarizes each bundle by
// version and by what it holds (the edit half: version and fragment count
// after the edit)
#[test]
fn test_the_listing_follows_an_edit_by_version_and_by_what_it_holds() {
    for flags in FORMATS {
        let project = demo();
        project
            .run(&[
                "bundle",
                "edit",
                "demo",
                "--version",
                "2.0.0",
                "--add-fragment",
                "testing",
            ])
            .succeeds();
        let ran = with_flags(&project, &["bundle", "list"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("demo (v2.0.0)")
                .contains("Contains: 2 fragments");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "[name=demo].version"), "2.0.0");
            assert_eq!(reports_as(&doc, "[name=demo].fragment_count"), "2");
        }
    }
}

// bundle.feature: Scenario: The selector reaches an MCP server's
// configuration too (the server is attached by `bundle edit --add-mcp`)
#[test]
fn test_an_attached_mcp_server_is_reached_by_the_selector() {
    let project = demo();
    project
        .run(&["bundle", "edit", "demo", "--add-mcp", "tree-sitter"])
        .succeeds();
    project
        .run(&["bundle", "view", "demo#mcp/tree-sitter"])
        .succeeds()
        .contains("# MCP Server: tree-sitter")
        .contains("command: tree-sitter");
}

// bundle.feature: Scenario: Metadata edits land in the file
#[test]
fn test_metadata_edits_land_in_the_file() {
    let project = demo();
    project.file_contains(&format!("{DEMO}/bundle.yaml"), "acceptance fixture bundle");
    project
        .run(&["bundle", "edit", "demo", "-d", "updated-desc"])
        .succeeds();
    project
        .file_contains(&format!("{DEMO}/bundle.yaml"), "updated-desc")
        .file_lacks(&format!("{DEMO}/bundle.yaml"), "acceptance fixture bundle");
}

// bundle.feature: Scenario: Items are attached and detached by name, and the
// manifest follows
#[test]
fn test_items_are_attached_and_detached_by_name_and_the_manifest_follows() {
    let project = demo();
    project
        .run(&[
            "bundle",
            "edit",
            "demo",
            "--add-fragment",
            "coding-standards",
            "--add-prompt",
            "review",
            "--add-mcp",
            "tree-sitter",
            "--add-tag",
            "golang",
        ])
        .succeeds();
    project
        .file_contains(
            &format!("{DEMO}/fragments/coding-standards.md"),
            "Add content here.",
        )
        .file_contains(
            &format!("{DEMO}/prompts/review.md"),
            "Add prompt content here.",
        )
        .file_contains(&format!("{DEMO}/bundle.yaml"), "golang");
    assert_eq!(
        project
            .read(&format!("{DEMO}/mcp/tree-sitter.yaml"))
            .as_deref(),
        Some("command: tree-sitter\n")
    );
    // Go's placeholder bytes.
    assert_eq!(
        project
            .read(&format!("{DEMO}/fragments/coding-standards.md"))
            .as_deref(),
        Some("# coding-standards\n\nAdd content here.")
    );
    assert_eq!(
        project
            .read(&format!("{DEMO}/prompts/review.md"))
            .as_deref(),
        Some("---\ndescription: review\n---\nAdd prompt content here.")
    );
    project
        .run(&[
            "bundle",
            "edit",
            "demo",
            "--remove-fragment",
            "coding-standards",
            "--remove-mcp",
            "tree-sitter",
            "--remove-tag",
            "golang",
        ])
        .succeeds();
    project
        .file_is_absent(&format!("{DEMO}/fragments/coding-standards.md"))
        .file_is_absent(&format!("{DEMO}/mcp/tree-sitter.yaml"))
        .file_lacks(&format!("{DEMO}/bundle.yaml"), "golang")
        // What she did not detach stays.
        .file_contains(
            &format!("{DEMO}/prompts/review.md"),
            "Add prompt content here.",
        );
}

// operations.UpdateBundle (documented condition): what an edit did is named,
// in order, in both formats; an add of a name already there is not a change.
#[test]
fn test_an_edit_names_what_it_changed() {
    let project = demo();
    let ran = project.run(&[
        "bundle",
        "edit",
        "demo",
        "--version",
        "2.0.0",
        "--add-tag",
        "golang",
        "--add-fragment",
        "testing",
        "--add-prompt",
        "review",
        "--add-mcp",
        "tree-sitter",
        "--format",
        "text",
    ]);
    ran.succeeds();
    let manifest = project.dir().join(DEMO).join("bundle.yaml");
    assert_eq!(
        ran.stdout,
        format!(
            "updated version\nadded tag: golang\nset fragment: testing\nset prompt: review\nset mcp: tree-sitter\nUpdated bundle: {}\n",
            manifest.display()
        )
    );
    let again = project.run(&[
        "bundle",
        "edit",
        "demo",
        "--add-fragment",
        "testing",
        "--format",
        "json",
    ]);
    again.succeeds();
    assert_eq!(reports_as(&again.json(), "status"), "no_changes");
    let removed = project.run(&[
        "bundle",
        "edit",
        "demo",
        "--remove-fragment",
        "testing",
        "--remove-tag",
        "golang",
        "--format",
        "json",
    ]);
    let doc = removed.json();
    assert_eq!(reports_as(&doc, "status"), "updated");
    assert_eq!(reports_as(&doc, "changes.0"), "removed tag: golang");
    assert_eq!(reports_as(&doc, "changes.1"), "removed fragment: testing");
}

// bundle.feature: Scenario Outline: An edit that was told to change nothing
// says so
#[test]
fn test_an_edit_that_was_told_to_change_nothing_says_so() {
    for flags in FORMATS {
        let project = demo();
        let before = project.tree(DEMO);
        let ran = with_flags(&project, &["bundle", "edit", "demo"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("No changes made.");
        } else {
            assert_eq!(reports_as(&ran.json(), "status"), "no_changes");
        }
        project.file_contains(&format!("{DEMO}/bundle.yaml"), "acceptance fixture bundle");
        assert_eq!(project.tree(DEMO), before, "a no-op edit wrote something");
    }
}

// operations.UpdateBundle (documented condition): flags that restate what is
// there change nothing, and an edit of a bundle nobody has is refused.
#[test]
fn test_an_edit_restating_what_is_there_changes_nothing_and_a_missing_bundle_is_refused() {
    let project = demo();
    let before = project.tree(DEMO);
    for args in [
        vec!["--version", "1.0.0"],
        vec!["--add-tag", "x", "--remove-tag", "x"],
        vec!["--remove-fragment", "nosuch"],
    ] {
        let mut all = vec!["bundle", "edit", "demo", "--format", "json"];
        all.extend(args);
        let ran = project.run(&all);
        ran.succeeds();
        assert_eq!(reports_as(&ran.json(), "status"), "no_changes", "{all:?}");
    }
    assert_eq!(project.tree(DEMO), before);
    project
        .run(&["bundle", "edit", "nosuch", "-d", "x"])
        .fails()
        .contains("bundle not found: nosuch");
}

// ---------------------------------------------------------------- travelling

// bundle.feature: Scenario: Export, remove, and re-import a bundle
#[test]
fn test_export_remove_and_re_import_a_bundle() {
    let project = demo();
    project
        .run(&["bundle", "export", "demo", "exported"])
        .succeeds();
    project
        .file_contains(
            "exported/demo/prompts/example.md",
            "Example prompt content. Describe what this prompt does.",
        )
        .file_contains("exported/demo/fragments/example.md", "# Example Fragment");
    project
        .run(&["bundle", "remove", "demo", "--yes"])
        .succeeds();
    project.file_is_absent(&format!("{DEMO}/bundle.yaml"));
    project
        .run(&["bundle", "import", "exported/demo", "-f"])
        .succeeds();
    project.run(&["bundle", "list"]).succeeds().contains("demo");
    project
        .file_contains(
            &format!("{DEMO}/prompts/example.md"),
            "Example prompt content. Describe what this prompt does.",
        )
        .file_contains(
            &format!("{DEMO}/fragments/example.md"),
            "# Example Fragment",
        );
}

// bundle.feature: Scenario: Export can name the destination file outright
#[test]
fn test_export_can_name_the_destination_outright() {
    let project = demo();
    project
        .run(&["bundle", "export", "demo", "-o", "handoff"])
        .succeeds();
    project.file_contains("handoff/fragments/example.md", "# Example Fragment");
}

// operations.ExportBundle / ImportBundle (documented conditions): the answer
// in each format, the whole tree travels (items, sidecars and dotfiles
// alike), and an export names a destination or is refused.
#[test]
fn test_export_and_import_answer_and_carry_the_whole_tree() {
    for flags in FORMATS {
        let project = demo();
        project.write(&format!("{DEMO}/mcp/.search.meta.yaml"), "notes: N\n");
        project.write(
            &format!("{DEMO}/mcp/search.yaml"),
            "url: https://x.example\n",
        );
        let exported = with_flags(&project, &["bundle", "export", "demo", "out"], flags);
        exported.succeeds();
        let manifest = project.dir().join(DEMO).join("bundle.yaml");
        if is_text(flags) {
            assert_eq!(
                exported.stdout,
                format!("Exported: {} -> out/demo\n", manifest.display())
            );
        } else {
            let doc = exported.json();
            assert_eq!(reports_as(&doc, "status"), "exported");
            assert_eq!(reports_as(&doc, "name"), "demo");
            assert_eq!(reports_as(&doc, "source"), manifest.to_string_lossy());
            assert_eq!(reports_as(&doc, "dest"), "out/demo");
        }
        assert_eq!(project.tree("out/demo"), project.tree(DEMO));
        project
            .run(&["bundle", "remove", "demo", "--yes"])
            .succeeds();
        let imported = with_flags(&project, &["bundle", "import", "out/demo"], flags);
        imported.succeeds();
        if is_text(flags) {
            assert_eq!(
                imported.stdout,
                format!(
                    "Imported: out/demo -> {}\n  Version: 1.0.0\n  Fragments: 1, Commands: 1, MCP: 1\n",
                    project.dir().join(DEMO).display()
                )
            );
        } else {
            let doc = imported.json();
            assert_eq!(reports_as(&doc, "status"), "imported");
            assert_eq!(reports_as(&doc, "source"), "out/demo");
            assert_eq!(reports_as(&doc, "version"), "1.0.0");
            assert_eq!(reports_as(&doc, "fragments"), "1");
            assert_eq!(reports_as(&doc, "commands"), "1");
            assert_eq!(reports_as(&doc, "mcp"), "1");
        }
        assert_eq!(project.tree(DEMO), project.tree("out/demo"));
    }
    let project = demo();
    project
        .run(&["bundle", "export", "demo"])
        .fails()
        .contains("either an output file or a destination directory must be specified");
    project
        .run(&["bundle", "export", "nosuch", "out"])
        .fails()
        .contains("bundle not found: nosuch");
    project
        .run(&["bundle", "import", "nowhere"])
        .fails()
        .contains("nowhere");
}

// bundle.feature: Scenario: Importing over an existing bundle refuses, and
// the local edit survives
#[test]
fn test_importing_over_an_existing_bundle_refuses_and_the_local_edit_survives() {
    let project = demo();
    project
        .run(&["bundle", "export", "demo", "exported"])
        .succeeds();
    project
        .run(&["bundle", "edit", "demo", "-d", "LOCAL-EDIT-MARKER"])
        .succeeds();
    let before = project.tree(DEMO);
    project
        .run(&["bundle", "import", "exported/demo"])
        .fails()
        .contains("bundle already exists")
        .contains("use --force to overwrite");
    project.file_contains(&format!("{DEMO}/bundle.yaml"), "LOCAL-EDIT-MARKER");
    assert_eq!(
        project.tree(DEMO),
        before,
        "the refused import wrote something"
    );
    project
        .run(&["bundle", "import", "exported/demo", "--force"])
        .succeeds();
    project
        .file_lacks(&format!("{DEMO}/bundle.yaml"), "LOCAL-EDIT-MARKER")
        .file_contains(&format!("{DEMO}/bundle.yaml"), "acceptance fixture bundle");
}

// operations.ImportBundle (documented condition): an import names the bundle
// by its directory (or its bundle.yaml's), and the bundle keeps that name;
// --force replaces the whole tree, an item the old copy had and the new one
// has not included.
#[test]
fn test_an_import_keeps_the_trees_own_name_and_a_forced_one_replaces_the_tree() {
    let project = demo();
    project
        .run(&["bundle", "export", "demo", "-o", "handoff"])
        .succeeds();
    project
        .run(&["bundle", "import", "handoff/bundle.yaml"])
        .succeeds();
    assert!(project.exists(".ctxloom/content/bundles/v2/handoff/bundle.yaml"));
    project.write(
        ".ctxloom/content/bundles/v2/handoff/fragments/only-here.md",
        "ONLY HERE",
    );
    project
        .run(&["bundle", "import", "handoff", "-f"])
        .succeeds();
    project.file_is_absent(".ctxloom/content/bundles/v2/handoff/fragments/only-here.md");
}

// ---------------------------------------------------------------- removing

// bundle.feature: Scenario Outline: Bare bundle remove reports what would go
// and destroys nothing
#[test]
fn test_bare_bundle_remove_reports_what_would_go_and_destroys_nothing() {
    for flags in FORMATS {
        let project = demo();
        project.fragment("demo", "testing");
        let before = project.tree(".ctxloom");
        let ran = with_flags(&project, &["bundle", "remove", "demo"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Would remove bundle \"demo\":\n  2 fragment(s), 1 command(s)\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom bundle remove demo --yes\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "preview");
            assert_eq!(reports_as(&doc, "applied"), "false");
            assert_eq!(reports_as(&doc, "target"), "bundle \"demo\"");
            assert_eq!(reports_as(&doc, "detail.0"), "2 fragment(s), 1 command(s)");
            assert_eq!(
                reports_as(&doc, "apply"),
                "ctxloom bundle remove demo --yes"
            );
        }
        project.file_contains(&format!("{DEMO}/bundle.yaml"), "acceptance fixture bundle");
        assert_eq!(
            project.tree(".ctxloom"),
            before,
            "a preview destroyed something"
        );
        let listed = with_flags(&project, &["bundle", "list"], flags);
        if !is_text(flags) {
            assert_eq!(reports_as(&listed.json(), "[name=demo].name"), "demo");
        }
    }
}

// bundle.feature: Scenario Outline: Removing a bundle with --yes takes the
// file and the listing entry
#[test]
fn test_removing_a_bundle_with_yes_takes_the_file_and_the_listing_entry() {
    for flags in FORMATS {
        let project = demo();
        project
            .run(&["bundle", "create", "keep", "-d", "kept"])
            .succeeds();
        let kept = project.tree(".ctxloom/content/bundles/v2/keep");
        let ran = with_flags(&project, &["bundle", "remove", "demo", "--yes"], flags);
        ran.succeeds();
        let manifest = project.dir().join(DEMO).join("bundle.yaml");
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                format!("Removed bundle: {}\n", manifest.display())
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "deleted");
            assert_eq!(reports_as(&doc, "name"), "demo");
            assert_eq!(reports_as(&doc, "path"), manifest.to_string_lossy());
        }
        // Exactly the target: the whole bundle's tree, items included (Go
        // left the items behind), and nothing of its neighbour.
        project.file_is_absent(DEMO);
        assert_eq!(project.tree(".ctxloom/content/bundles/v2/keep"), kept);
        let listed = with_flags(&project, &["bundle", "list", "--no-companions"], flags);
        if is_text(flags) {
            listed.lacks("demo (v");
        } else {
            let doc = listed.json();
            assert_eq!(entries(&doc, "$"), 1);
            assert_eq!(reports_as(&doc, "[name=keep].name"), "keep");
        }
    }
    let project = Project::initialized();
    project
        .run(&["bundle", "remove", "nosuch", "--yes"])
        .fails()
        .contains("bundle not found: nosuch");
    project
        .run(&["bundle", "remove", "nosuch"])
        .fails()
        .contains("bundle not found: nosuch");
}

// bundle.feature: Scenario Outline: The rm alias previews and destroys
// exactly as remove does
#[test]
fn test_the_rm_alias_previews_and_destroys_exactly_as_remove_does() {
    for flags in FORMATS {
        let project = demo();
        let ran = with_flags(&project, &["bundle", "rm", "demo"], flags);
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Nothing was removed");
        } else {
            assert_eq!(reports_as(&ran.json(), "applied"), "false");
        }
        project.file_contains(&format!("{DEMO}/bundle.yaml"), "acceptance fixture bundle");
        project.run(&["bundle", "del", "demo", "--yes"]).succeeds();
        project.file_is_absent(&format!("{DEMO}/bundle.yaml"));
    }
}
