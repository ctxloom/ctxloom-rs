//! Go's acceptance scenarios for the item nouns' write side (S2):
//! `tests/acceptance/features/cli/fragment.feature`, `command.feature` and
//! `editing.feature` at release/0.7 ca14d6046, ported: each test names its
//! scenario. The bytes and answers are Go's own (captured from a Go ctxloom
//! run in a scratch project).
//!
//! Not here, each by what it needs: the `ctxloom://` resource assertions (the
//! MCP cut list is open, D8), `materialize` (S9), `distill` (S7). The edit
//! that re-distills has no distiller until S7: the default edit saves the
//! content RAW and says so, which is the scenario "the distillation backend
//! cannot start" of editing.feature.

use super::library::{reports_as, Editor, Project};

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

fn demo(project: &Project) {
    project.bundle("demo");
}

// ---------------------------------------------------------------- fragment.feature

// fragment.feature: Scenario: Alice creates a fragment and it lands in the
// manifest, the listing, and the agent's resource (the manifest and the
// listing; the resource is MCP's)
#[test]
fn test_alice_creates_a_fragment_and_it_lands_in_the_manifest_and_the_listing() {
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
        .run(&["fragment", "create", "demo", "testing"])
        .succeeds();
    // Go's placeholder bytes.
    assert_eq!(
        project
            .read(&format!("{DEMO}/fragments/testing.md"))
            .as_deref(),
        Some("# testing\n\nAdd content here.")
    );
    project
        .run(&["fragment", "list"])
        .succeeds()
        .contains("testing");
}

// operations.AddItem (documented condition): the answer in each format; a
// name already there is refused and its content survives; a name that is no
// single path segment, and a bundle nobody has, are refused and write nothing.
#[test]
fn test_creating_an_item_answers_and_refuses_what_it_must() {
    for flags in FORMATS {
        let project = Project::initialized();
        demo(&project);
        let manifest = project.dir().join(DEMO).join("bundle.yaml");
        let ran = with_flags(&project, &["fragment", "create", "demo", "testing"], flags);
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Created fragment \"testing\" in bundle \"demo\"\nEdit with: ctxloom fragment edit demo#fragments/testing\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "created");
            assert_eq!(reports_as(&doc, "bundle"), "demo");
            assert_eq!(reports_as(&doc, "name"), "testing");
            assert_eq!(reports_as(&doc, "path"), manifest.to_string_lossy());
        }
    }
    let project = Project::initialized();
    demo(&project);
    project.fragment("demo", "kept");
    project
        .run(&["fragment", "create", "demo", "kept"])
        .fails()
        .contains("fragment already exists: kept");
    project.file_contains(&format!("{DEMO}/fragments/kept.md"), "FRAGMENT-BODY-kept");
    let before = project.tree(DEMO);
    project.run(&["fragment", "create", "demo", "a/b"]).fails();
    project
        .run(&["fragment", "create", "nobundle", "x"])
        .fails()
        .contains("bundle not found: nobundle");
    assert_eq!(project.tree(DEMO), before);
}

// fragment.feature: Scenario Outline: Bare fragment remove reports and
// destroys nothing
#[test]
fn test_bare_fragment_remove_reports_and_destroys_nothing() {
    for flags in FORMATS {
        let project = Project::initialized();
        demo(&project);
        project.fragment("demo", "testing");
        let before = project.tree(".ctxloom");
        let ran = with_flags(
            &project,
            &["fragment", "remove", "demo#fragments/testing"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Would remove fragment \"testing\" from bundle \"demo\".\n\nNothing was removed. Re-run with --yes to apply:\n  ctxloom fragment remove demo#fragments/testing --yes\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "preview");
            assert_eq!(reports_as(&doc, "applied"), "false");
            assert_eq!(
                reports_as(&doc, "target"),
                "fragment \"testing\" from bundle \"demo\""
            );
            assert_eq!(
                reports_as(&doc, "apply"),
                "ctxloom fragment remove demo#fragments/testing --yes"
            );
        }
        project.file_contains(
            &format!("{DEMO}/fragments/testing.md"),
            "FRAGMENT-BODY-testing",
        );
        assert_eq!(project.tree(".ctxloom"), before);
    }
}

// fragment.feature: Scenario: Removing a fragment removes it from the
// manifest, the listing, and the agent's resource (the file and the listing)
#[test]
fn test_removing_a_fragment_removes_exactly_it() {
    for flags in FORMATS {
        let project = Project::initialized();
        demo(&project);
        project.fragment("demo", "testing");
        project.fragment("demo", "other");
        // Its distilled form goes with it; its neighbour's does not.
        project.write(
            &format!("{DEMO}/fragments/testing.distilled.md"),
            "DISTILLED",
        );
        project.write(
            &format!("{DEMO}/fragments/other.distilled.md"),
            "OTHER DISTILLED",
        );
        let others: Vec<_> = project
            .tree(DEMO)
            .into_iter()
            .filter(|(path, _)| !path.starts_with("fragments/testing"))
            .collect();
        let ran = with_flags(
            &project,
            &["fragment", "remove", "demo#fragments/testing", "--yes"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                "Removed fragment \"testing\" from bundle \"demo\"\n"
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "deleted");
            assert_eq!(reports_as(&doc, "bundle"), "demo");
            assert_eq!(reports_as(&doc, "name"), "testing");
        }
        project.file_is_absent(&format!("{DEMO}/fragments/testing.md"));
        project.file_is_absent(&format!("{DEMO}/fragments/testing.distilled.md"));
        let after: Vec<_> = project.tree(DEMO).into_iter().collect();
        assert_eq!(after, others, "more than the target went");
        project
            .run(&["fragment", "list"])
            .succeeds()
            .lacks("testing");
    }
    let project = Project::initialized();
    demo(&project);
    for yes in [vec![], vec!["--yes"]] {
        let mut args = vec!["fragment", "remove", "demo#fragments/nope"];
        args.extend(yes);
        project
            .run(&args)
            .fails()
            .contains("fragment not found: nope");
    }
}

// ---------------------------------------------------------------- command.feature

// command.feature: Scenario: Alice creates a command and it is listed and
// exposed to the agent (the file and the listing)
#[test]
fn test_alice_creates_a_command_and_it_is_listed() {
    let project = Project::initialized();
    demo(&project);
    project
        .run(&["command", "create", "demo", "review"])
        .succeeds();
    assert_eq!(
        project
            .read(&format!("{DEMO}/prompts/review.md"))
            .as_deref(),
        Some("# review\n\nAdd content here.")
    );
    project
        .run(&["command", "list"])
        .succeeds()
        .contains("review");
    let json = project.run(&["command", "create", "demo", "second", "--format", "text"]);
    assert_eq!(
        json.stdout,
        "Created command \"second\" in bundle \"demo\"\nEdit with: ctxloom command edit demo#commands/second\n"
    );
}

// command.feature: Scenario Outline: Bare command remove reports and
// destroys nothing
#[test]
fn test_bare_command_remove_reports_and_destroys_nothing() {
    for flags in FORMATS {
        let project = Project::initialized();
        demo(&project);
        project.command("demo", "review");
        let before = project.tree(".ctxloom");
        let ran = with_flags(
            &project,
            &["command", "remove", "demo#commands/review"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Nothing was removed");
        } else {
            assert_eq!(reports_as(&ran.json(), "applied"), "false");
        }
        ran.contains("ctxloom command remove demo#commands/review --yes");
        project.file_contains(&format!("{DEMO}/prompts/review.md"), "COMMAND-BODY-review");
        assert_eq!(project.tree(".ctxloom"), before);
    }
}

// command.feature: Scenario: Removing a command removes it from the manifest,
// the listing, and the agent's resource (the file and the listing)
#[test]
fn test_removing_a_command_removes_exactly_it() {
    let project = Project::initialized();
    demo(&project);
    project.command("demo", "review");
    let others = project.tree(".ctxloom/content/bundles/v2/demo/fragments");
    project
        .run(&["command", "remove", "demo#commands/review", "--yes"])
        .succeeds();
    project.file_is_absent(&format!("{DEMO}/prompts/review.md"));
    project.run(&["command", "list"]).succeeds().lacks("review");
    assert_eq!(project.tree(&format!("{DEMO}/fragments")), others);
    let doc = {
        project.command("demo", "again");
        project
            .run(&[
                "command",
                "remove",
                "demo#commands/again",
                "--yes",
                "--format",
                "json",
            ])
            .json()
    };
    assert_eq!(reports_as(&doc, "status"), "deleted");
}

// command.feature: Scenario Outline: Editing with --no-distill warns instead
// of leaving a stale distillation unmentioned
#[test]
fn test_editing_with_no_distill_warns_instead_of_leaving_a_stale_distillation_unmentioned() {
    for flags in FORMATS {
        let project = Project::with_editor(Editor::Marker);
        demo(&project);
        project.command("demo", "review");
        project.write(
            &format!("{DEMO}/prompts/review.distilled.md"),
            "STALE DISTILLED",
        );
        let ran = with_flags(
            &project,
            &["command", "edit", "demo#commands/review", "--no-distill"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("distilled form not refreshed (--no-distill)")
                .contains("Updated command \"review\" in bundle \"demo\"")
                .contains("ctxloom bundle push demo [remote]");
        } else {
            assert_eq!(reports_as(&ran.json(), "distilled"), "false");
        }
        project.file_contains(&format!("{DEMO}/prompts/review.md"), "EDITED-BY-TEST");
        // Never stale: the old distillation does not outlive the content.
        project.file_is_absent(&format!("{DEMO}/prompts/review.distilled.md"));
    }
}

// command.feature: Scenario: An editor that returns nothing is refused rather
// than emptying the command
#[test]
fn test_an_editor_that_returns_nothing_is_refused_rather_than_emptying_the_command() {
    let project = Project::with_editor(Editor::Empties);
    demo(&project);
    project.command("demo", "review");
    project
        .run(&["command", "edit", "demo#commands/review"])
        .fails()
        .contains("refusing to overwrite");
    project.file_contains(&format!("{DEMO}/prompts/review.md"), "COMMAND-BODY-review");
}

// ---------------------------------------------------------------- editing.feature

// editing.feature: Scenario: Editing a fragment lands the change across axes
// (the file: the distillation backend cannot start, so the content is saved
// RAW and the command says so)
#[test]
fn test_editing_a_fragment_lands_the_change_across_axes() {
    let project = Project::with_editor(Editor::Marker);
    demo(&project);
    project.fragment("demo", "testing");
    project
        .run(&["fragment", "edit", "demo#fragments/testing"])
        .fails()
        .contains("content saved RAW");
    project.file_contains(&format!("{DEMO}/fragments/testing.md"), "EDITED-BY-TEST");
}

// editing.feature: Scenario: Editing a prompt lands the change in the bundle
#[test]
fn test_editing_a_prompt_lands_the_change_in_the_bundle() {
    let project = Project::with_editor(Editor::Marker);
    demo(&project);
    project.command("demo", "review");
    project
        .run(&["command", "edit", "demo#commands/review"])
        .fails()
        .contains("content saved RAW");
    project.file_contains(&format!("{DEMO}/prompts/review.md"), "EDITED-BY-TEST");
}

// cli/item_crud.go editItem (documented condition): what the editor round
// trip keeps and drops. The front matter stays, minus the hash of the content
// it described; the body is what the editor returned, byte for byte; an item
// marked no_distill needs no flag; nothing to change says so and writes
// nothing; an item nobody has is refused.
#[test]
fn test_an_edit_keeps_the_front_matter_and_says_when_nothing_changed() {
    let project = Project::with_editor(Editor::Marker);
    demo(&project);
    project.write(
        &format!("{DEMO}/fragments/withmeta.md"),
        "---\ntags:\n  - t\ncontent_hash: sha256:abc\nno_distill: true\n---\nBODY",
    );
    let ran = project.run(&[
        "fragment",
        "edit",
        "demo#fragments/withmeta",
        "--format",
        "text",
    ]);
    ran.succeeds();
    assert_eq!(
        ran.stdout,
        "Updated fragment \"withmeta\" in bundle \"demo\"\n\nBundle modified. To publish changes:\n  ctxloom bundle push demo [remote]\n"
    );
    assert_eq!(
        project
            .read(&format!("{DEMO}/fragments/withmeta.md"))
            .as_deref(),
        Some("---\ntags:\n  - t\nno_distill: true\n---\nBODY\nEDITED-BY-TEST\n")
    );
    project.fragment("demo", "plain");
    let json = project.run(&[
        "fragment",
        "edit",
        "demo#fragments/plain",
        "--no-distill",
        "--format",
        "json",
    ]);
    let doc = json.json();
    assert_eq!(reports_as(&doc, "status"), "updated");
    assert_eq!(reports_as(&doc, "bundle"), "demo");
    assert_eq!(reports_as(&doc, "name"), "plain");
    assert_eq!(
        project
            .read(&format!("{DEMO}/fragments/plain.md"))
            .as_deref(),
        Some("FRAGMENT-BODY-plain\nEDITED-BY-TEST\n")
    );
    project
        .run(&["fragment", "edit", "demo#fragments/nope", "--no-distill"])
        .fails()
        .contains("fragment \"nope\": item not found");
    project
        .run(&["fragment", "edit", "demo#commands/x", "--no-distill"])
        .fails()
        .contains("selects a commands/, not a fragments/");
    project
        .run(&["fragment", "edit", "nobundle#fragments/x", "--no-distill"])
        .fails()
        .contains("bundle not found: nobundle");
}

// cli/item_crud.go editItem (documented condition): an editor that changes
// nothing says so, in each format, and the item is not rewritten.
#[test]
fn test_an_editor_that_changes_nothing_says_so() {
    for flags in FORMATS {
        // The home's editor is `true`, which leaves the file as it found it.
        let project = Project::initialized();
        demo(&project);
        project.fragment("demo", "testing");
        let before = project.tree(DEMO);
        let ran = with_flags(
            &project,
            &["fragment", "edit", "demo#fragments/testing", "--no-distill"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            assert_eq!(ran.stdout, "No changes made.\n");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "unchanged");
            assert_eq!(reports_as(&doc, "distilled"), "false");
        }
        assert_eq!(project.tree(DEMO), before);
    }
}
