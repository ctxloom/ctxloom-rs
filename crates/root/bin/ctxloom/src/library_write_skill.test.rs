//! Go's acceptance scenarios for the skill noun's write side (S2),
//! `tests/acceptance/features/cli/skill.feature` at release/0.7 ca14d6046,
//! ported: each test names its scenario. Not here: `materialize` (S9).
//!
//! Plan over Go (D15, §8.2 "exec bits only from the sidecar's `executable:`"):
//! a package's executable files are the sidecar's `executable:` list, in the
//! archive and out of it; an archive's own mode bits put nothing on disk, and
//! an import over a skill that is there refuses (Go replaced it in silence).

use super::library::{reports_as, Project};
use crate::tests::files;

const FORMATS: [&[&str]; 3] = [&[], &["--format", "json"], &["--format", "text"]];

fn with_flags(project: &Project, base: &[&str], flags: &[&str]) -> super::Ran {
    let mut args = base.to_vec();
    args.extend_from_slice(flags);
    project.run(&args)
}

fn is_text(flags: &[&str]) -> bool {
    flags.contains(&"text")
}

const VAULT: &str = ".ctxloom/content/bundles/v2/vault";
const LANDED: &str = ".ctxloom/content/bundles/v2/landed";

/// Go's `Alice's project has a directory-form bundle "vault"` (and `a
/// directory-form bundle "landed" exists`).
fn project_with_bundles() -> Project {
    let project = Project::initialized();
    for (name, dir) in [("vault", VAULT), ("landed", LANDED)] {
        project.write(
            &format!("{dir}/bundle.yaml"),
            &format!("version: \"1.0.0\"\ndescription: {name} bundle\n"),
        );
    }
    project
}

/// Go's `Alice adds an executable scripts/run.sh carrying the marker "<m>" to
/// the skill`: the script written executable on disk, and (the plan's rule)
/// declared in the skill's sidecar when `declared`.
fn add_script(project: &Project, marker: &str, declared: bool) {
    let script = format!("{VAULT}/skills/reviewer/scripts/run.sh");
    project.write(&script, &format!("#!/bin/sh\n# {marker}\necho {marker}\n"));
    project.make_executable(&script);
    if declared {
        project.write(
            &format!("{VAULT}/skills/.reviewer.meta.yaml"),
            "executable:\n  - scripts/run.sh\n",
        );
    }
}

fn create_reviewer(project: &Project) {
    project
        .run(&[
            "skill",
            "create",
            "vault",
            "reviewer",
            "-d",
            "SKILL-MARKER-reviewer-9f3c21",
        ])
        .succeeds();
}

// skill.feature: Scenario: Alice authors a skill package and its listing and
// show reflect the real tree (the create: Go's SKILL.md, bytes and answers)
#[test]
fn test_alice_authors_a_skill_package_and_its_listing_and_show_reflect_the_real_tree() {
    for flags in FORMATS {
        let project = project_with_bundles();
        let ran = with_flags(
            &project,
            &[
                "skill",
                "create",
                "vault",
                "reviewer",
                "-d",
                "SKILL-MARKER-reviewer-9f3c21",
            ],
            flags,
        );
        ran.succeeds();
        let dir = project.dir().join(VAULT).join("skills/reviewer");
        if is_text(flags) {
            assert_eq!(
                ran.stdout,
                format!(
                    "Created skill \"reviewer\" in bundle \"vault\"\n  {0}\nEdit {0}/SKILL.md to describe the skill\n",
                    dir.display()
                )
            );
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "status"), "created");
            assert_eq!(reports_as(&doc, "bundle"), "vault");
            assert_eq!(reports_as(&doc, "name"), "reviewer");
            assert_eq!(reports_as(&doc, "dir"), dir.to_string_lossy());
        }
        assert_eq!(
            project.read(&format!("{VAULT}/skills/reviewer/SKILL.md")).as_deref(),
            Some("---\nname: reviewer\ndescription: SKILL-MARKER-reviewer-9f3c21\n---\n\n# reviewer\n\nTODO: describe what this skill does and how to use it.\n")
        );
    }
    let project = project_with_bundles();
    create_reviewer(&project);
    add_script(&project, "SCRIPT-MARKER-reviewer-2b7e40", true);
    let shown = project.run(&["skill", "show", "vault#skills/reviewer", "--format", "json"]);
    shown.succeeds();
    let files = shown.json()["files"].clone();
    let paths: Vec<&str> = files
        .as_array()
        .expect("files")
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert_eq!(paths, ["SKILL.md", "scripts/run.sh"]);
    project
        .run(&["skill", "list"])
        .succeeds()
        .contains("reviewer")
        .contains("vault");
}

// operations skill create (documented condition): the default description,
// and the refusals that write nothing: a skill that is there, a bundle that
// is not, a name that is no single path segment (Go wrote `a/b` and left the
// bundle unreadable).
#[test]
fn test_creating_a_skill_refuses_what_it_must_and_writes_nothing() {
    let project = project_with_bundles();
    project
        .run(&["skill", "create", "vault", "plain"])
        .succeeds();
    project
        .file_contains(
            &format!("{VAULT}/skills/plain/SKILL.md"),
            "TODO: describe when an agent should use this skill.",
        )
        .file_contains(&format!("{VAULT}/skills/plain/SKILL.md"), "name: plain");
    let before = project.tree(VAULT);
    project
        .run(&["skill", "create", "vault", "plain", "-d", "other"])
        .fails()
        .contains("item already exists");
    for bad in ["a/b", ".hidden", "..", ""] {
        project.run(&["skill", "create", "vault", bad]).fails();
    }
    project
        .run(&["skill", "create", "nobundle", "x"])
        .fails()
        .contains("bundle not found: nobundle");
    assert_eq!(project.tree(VAULT), before);
    project.run(&["skill", "list"]).succeeds();
}

// skill.feature: Scenario Outline: Removing a skill drops its on-disk
// directory
#[test]
fn test_removing_a_skill_drops_its_on_disk_directory() {
    for flags in FORMATS {
        let project = project_with_bundles();
        create_reviewer(&project);
        project
            .run(&["skill", "create", "vault", "planner"])
            .succeeds();
        add_script(&project, "SCRIPT-MARKER-reviewer-2b7e40", true);
        let before = project.tree(&format!("{VAULT}/skills"));
        let ran = with_flags(
            &project,
            &["skill", "remove", "vault#skills/reviewer"],
            flags,
        );
        ran.succeeds();
        if is_text(flags) {
            ran.contains("Nothing was removed");
        } else {
            let doc = ran.json();
            assert_eq!(reports_as(&doc, "applied"), "false");
            assert_eq!(
                reports_as(&doc, "apply"),
                "ctxloom skill remove vault#skills/reviewer --yes"
            );
        }
        ran.contains("ctxloom skill remove vault#skills/reviewer --yes");
        assert_eq!(
            project.tree(&format!("{VAULT}/skills")),
            before,
            "a preview destroyed something"
        );
        project
            .run(&["skill", "list"])
            .succeeds()
            .contains("reviewer");
        let applied = project.run(&[
            "skill",
            "remove",
            "vault#skills/reviewer",
            "--yes",
            "--format",
            "json",
        ]);
        applied.succeeds();
        let doc = applied.json();
        assert_eq!(reports_as(&doc, "name"), "reviewer");
        assert_eq!(reports_as(&doc, "bundle"), "vault");
        project.file_is_absent(&format!("{VAULT}/skills/reviewer"));
        // Exactly the target: the package and its sidecar; the neighbour stays.
        project.file_is_absent(&format!("{VAULT}/skills/.reviewer.meta.yaml"));
        project.file_contains(&format!("{VAULT}/skills/planner/SKILL.md"), "name: planner");
        project.run(&["skill", "list"]).succeeds().lacks("reviewer");
    }
}

// skill.feature: Scenario: Removing a skill nobody created is refused, not
// silently reported as done
#[test]
fn test_removing_a_skill_nobody_created_is_refused_not_silently_reported_as_done() {
    let project = project_with_bundles();
    project
        .run(&["skill", "remove", "vault#skills/never-created", "--yes"])
        .fails()
        .contains("never-created");
}

// skill.feature: Scenario: A skill's files survive export and import
// byte-for-byte
#[test]
fn test_a_skills_files_survive_export_and_import_byte_for_byte() {
    for flags in FORMATS {
        let project = project_with_bundles();
        create_reviewer(&project);
        add_script(&project, "SCRIPT-MARKER-reviewer-2b7e40", true);
        project.write_bytes(
            &format!("{VAULT}/skills/reviewer/assets/blob.bin"),
            &[0, 255, 1, 254, 10, 13, 0],
        );
        let exported = with_flags(
            &project,
            &[
                "skill",
                "export",
                "vault#skills/reviewer",
                "-o",
                "reviewer.zip",
            ],
            flags,
        );
        exported.succeeds();
        if !is_text(flags) {
            let doc = exported.json();
            assert_eq!(reports_as(&doc, "name"), "reviewer");
            assert_eq!(reports_as(&doc, "zip_path"), "reviewer.zip");
            let bytes: usize = reports_as(&doc, "bytes").parse().expect("a size");
            assert_eq!(
                project.read_bytes("reviewer.zip").expect("the zip").len(),
                bytes
            );
        }
        let imported = with_flags(
            &project,
            &["skill", "import", "reviewer.zip", "--bundle", "landed"],
            flags,
        );
        imported.succeeds();
        let dir = project.dir().join(LANDED).join("skills/reviewer");
        if is_text(flags) {
            assert_eq!(
                imported.stdout,
                format!(
                    "Imported skill \"reviewer\" into bundle \"landed\" (3 file(s))\n  {}\n",
                    dir.display()
                )
            );
        } else {
            let doc = imported.json();
            assert_eq!(reports_as(&doc, "status"), "imported");
            assert_eq!(reports_as(&doc, "bundle"), "landed");
            assert_eq!(reports_as(&doc, "name"), "reviewer");
            assert_eq!(reports_as(&doc, "dir"), dir.to_string_lossy());
            assert_eq!(reports_as(&doc, "file_count"), "3");
        }
        assert_eq!(
            project.tree(&format!("{LANDED}/skills/reviewer")),
            project.tree(&format!("{VAULT}/skills/reviewer")),
            "the imported files differ from the authored ones"
        );
        // The exec bit travels by the sidecar: declared on the landed copy,
        // and the script is the only file that has it.
        project.file_contains(
            &format!("{LANDED}/skills/.reviewer.meta.yaml"),
            "scripts/run.sh",
        );
        assert!(project.is_executable(&format!("{LANDED}/skills/reviewer/scripts/run.sh")));
        assert!(!project.is_executable(&format!("{LANDED}/skills/reviewer/SKILL.md")));
        assert!(!project.is_executable(&format!("{LANDED}/skills/reviewer/assets/blob.bin")));
        project.file_is_absent("reviewer.zip.bak");
        project.run(&["bundle", "view", "landed"]).succeeds();
        let _ = files::fs().remove_file(&project.dir().join("reviewer.zip"));
    }
}

// plan over Go (§8.2: exec bits only from the sidecar's `executable:`): a
// script that is executable on disk but not declared in the sidecar travels
// as a plain file, and lands as one.
#[test]
fn test_an_exec_bit_on_disk_but_not_in_the_sidecar_does_not_travel() {
    let project = project_with_bundles();
    create_reviewer(&project);
    add_script(&project, "SCRIPT-MARKER-reviewer-2b7e40", false);
    assert!(project.is_executable(&format!("{VAULT}/skills/reviewer/scripts/run.sh")));
    project
        .run(&[
            "skill",
            "export",
            "vault#skills/reviewer",
            "-o",
            "reviewer.zip",
        ])
        .succeeds();
    project
        .run(&["skill", "import", "reviewer.zip", "--bundle", "landed"])
        .succeeds();
    assert!(!project.is_executable(&format!("{LANDED}/skills/reviewer/scripts/run.sh")));
    project.file_is_absent(&format!("{LANDED}/skills/.reviewer.meta.yaml"));
}

// plan over Go (§10 S2: an import over an existing item refuses, and the
// local edit survives): Go replaced the skill in silence.
#[test]
fn test_importing_over_an_existing_skill_refuses_and_the_local_edit_survives() {
    let project = project_with_bundles();
    create_reviewer(&project);
    project
        .run(&[
            "skill",
            "export",
            "vault#skills/reviewer",
            "-o",
            "reviewer.zip",
        ])
        .succeeds();
    project
        .run(&["skill", "import", "reviewer.zip", "--bundle", "landed"])
        .succeeds();
    project.write(
        &format!("{LANDED}/skills/reviewer/notes.md"),
        "LOCAL-EDIT-MARKER",
    );
    let before = project.tree(LANDED);
    project
        .run(&["skill", "import", "reviewer.zip", "--bundle", "landed"])
        .fails()
        .contains("skill already exists")
        .contains("use --force to overwrite");
    assert_eq!(project.tree(LANDED), before);
    project
        .run(&[
            "skill",
            "import",
            "reviewer.zip",
            "--bundle",
            "landed",
            "--force",
        ])
        .succeeds();
    project.file_is_absent(&format!("{LANDED}/skills/reviewer/notes.md"));
}

// operations skill export/import (documented conditions): export refuses to
// overwrite a file that is there and names a skill that is not; import
// refuses what is no archive, a missing archive and a missing bundle, and
// writes nothing.
#[test]
fn test_export_and_import_refuse_what_they_must_and_write_nothing() {
    let project = project_with_bundles();
    create_reviewer(&project);
    project
        .run(&["skill", "export", "vault#skills/never", "-o", "never.zip"])
        .fails()
        .contains("never");
    project.file_is_absent("never.zip");
    project
        .run(&["skill", "export", "vault#skills/reviewer"])
        .succeeds();
    assert!(project.exists("reviewer.zip"), "the default is <name>.zip");
    project
        .run(&["skill", "export", "vault#skills/reviewer"])
        .fails()
        .contains("already exists");
    project.write("junk.zip", "this is not an archive");
    let before = project.tree(LANDED);
    project
        .run(&["skill", "import", "junk.zip", "--bundle", "landed"])
        .fails()
        .contains("unrecognized format");
    project
        .run(&["skill", "import", "nosuch.zip", "--bundle", "landed"])
        .fails()
        .contains("nosuch.zip");
    project
        .run(&["skill", "import", "reviewer.zip", "--bundle", "nobundle"])
        .fails()
        .contains("bundle not found: nobundle");
    assert_eq!(project.tree(LANDED), before);
}
