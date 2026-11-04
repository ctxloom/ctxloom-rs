//! `skill.feature`'s write scenarios at the application port.

use std::sync::Arc;

use library::write::archive::{pack, Entry};

use crate::fakes::{plain, Mem, Setup, CONFIG, ENVELOPE};
use crate::write::views::Removal;
use crate::Library;

const SKILL_MD: &str = "---\nname: reviewer\ndescription: SKILL-MARKER\n---\n\n# reviewer\n";

fn world() -> (Library, Arc<Mem>) {
    Setup {
        bundles: &[
            (
                "vault",
                &[
                    ENVELOPE,
                    ("skills/reviewer/SKILL.md", SKILL_MD),
                    ("skills/reviewer/scripts/run.sh", "#!/bin/sh\n"),
                    ("skills/reviewer/assets/blob.bin", "BLOB"),
                    (
                        "skills/.reviewer.meta.yaml",
                        "tags:\n  - t\nexecutable:\n  - scripts/run.sh\n",
                    ),
                    (
                        "skills/planner/SKILL.md",
                        "---\nname: planner\ndescription: d\n---\n",
                    ),
                ],
            ),
            ("landed", &[ENVELOPE]),
        ],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture()
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

fn entry(path: &str, bytes: &str, executable: bool) -> Entry {
    Entry {
        path: path.to_string(),
        bytes: bytes.as_bytes().to_vec(),
        executable,
    }
}

// skill.feature: Alice authors a skill package and its listing and show reflect the real tree
#[test]
fn test_create_scaffolds_the_package_with_go_s_skill_md() {
    let (library, mem) = world();
    let done = library
        .skill_create(&plain(), "landed", "fresh", "SKILL-MARKER-fresh")
        .result
        .unwrap();
    assert_eq!(
        (
            done.status.as_str(),
            done.bundle.as_str(),
            done.name.as_str(),
            done.dir.as_str()
        ),
        (
            "created",
            "landed",
            "fresh",
            "/p/.ctxloom/content/bundles/v2/landed/skills/fresh"
        )
    );
    assert_eq!(
        mem.text("landed", "skills/fresh/SKILL.md").unwrap(),
        "---\nname: fresh\ndescription: SKILL-MARKER-fresh\n---\n\n# fresh\n\nTODO: describe what this skill does and how to use it.\n"
    );
    assert_eq!(mem.log(), ["apply landed"]);
    let shown = library
        .skill_show(&plain(), "landed#skills/fresh")
        .result
        .unwrap();
    assert_eq!(shown.description, "SKILL-MARKER-fresh");
}

#[test]
fn test_create_with_no_description_writes_the_default_one() {
    let (library, mem) = world();
    library
        .skill_create(&plain(), "landed", "plain", "")
        .result
        .unwrap();
    assert!(mem
        .text("landed", "skills/plain/SKILL.md")
        .unwrap()
        .contains("TODO: describe when an agent should use this skill."));
}

#[test]
fn test_create_refuses_what_is_there_a_bad_name_and_a_missing_bundle() {
    let (library, mem) = world();
    assert!(err(library.skill_create(&plain(), "vault", "reviewer", "d")).contains("SkillExists"));
    for bad in ["a/b", ".x", "..", "", "a\\b"] {
        assert!(
            err(library.skill_create(&plain(), "vault", bad, "d")).contains("BadSkillName"),
            "{bad:?}"
        );
    }
    assert!(err(library.skill_create(&plain(), "nobundle", "x", "d")).contains("BundleNotFound"));
    assert_eq!(mem.writes(), 0);
}

// skill.feature: Removing a skill drops its on-disk directory
#[test]
fn test_remove_previews_then_takes_the_package_and_its_sidecar_only() {
    let (library, mem) = world();
    let Removal::Preview(p) = library
        .skill_remove(&plain(), "vault#skills/reviewer", false)
        .result
        .unwrap()
    else {
        panic!("a preview")
    };
    assert_eq!(p.target, "skill \"reviewer\" from bundle \"vault\"");
    assert_eq!(p.apply, "ctxloom skill remove vault#skills/reviewer --yes");
    assert_eq!(mem.writes(), 0);
    let Removal::Done(done) = library
        .skill_remove(&plain(), "vault#skills/reviewer", true)
        .result
        .unwrap()
    else {
        panic!("done")
    };
    assert_eq!(
        (
            done.status.as_str(),
            done.bundle.as_str(),
            done.name.as_str(),
            done.dir.as_str()
        ),
        (
            "removed",
            "vault",
            "reviewer",
            "/p/.ctxloom/content/bundles/v2/vault/skills/reviewer"
        )
    );
    assert_eq!(
        mem.paths("vault"),
        ["bundle.yaml", "skills/planner/SKILL.md"]
    );
}

// skill.feature: Removing a skill nobody created is refused, not silently reported as done
#[test]
fn test_remove_of_a_skill_nobody_created_is_refused() {
    let (library, mem) = world();
    for yes in [false, true] {
        assert!(
            err(library.skill_remove(&plain(), "vault#skills/never-created", yes))
                .contains("SkillNotFound")
        );
    }
    assert!(err(library.skill_remove(&plain(), "vault", true)).contains("SkillReference"));
    assert!(
        err(library.skill_remove(&plain(), "vault#fragments/x", true)).contains("SkillReference")
    );
    assert_eq!(mem.writes(), 0);
}

// skill.feature: A skill's files survive export and import byte-for-byte
#[test]
fn test_export_packs_the_package_with_exec_bits_from_the_sidecar_only() {
    let (library, mem) = world();
    // A file made executable on disk but not declared travels as plain.
    mem.set_exec_for_test("vault", "skills/reviewer/assets/blob.bin");
    let done = library
        .skill_export(&plain(), "vault#skills/reviewer", "reviewer.zip")
        .result
        .unwrap();
    assert_eq!(
        (done.name.as_str(), done.zip_path.as_str()),
        ("reviewer", "reviewer.zip")
    );
    let (bytes, _) = mem.user_file("reviewer.zip").unwrap();
    assert_eq!(done.bytes, bytes.len() as u64);
    let got = library::write::archive::unpack(&bytes, &library::write::archive::Limits::default())
        .unwrap();
    assert_eq!(got.top, "reviewer");
    let listed: Vec<(&str, bool)> = got
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.executable))
        .collect();
    assert_eq!(
        listed,
        [
            ("SKILL.md", false),
            ("assets/blob.bin", false),
            ("scripts/run.sh", true)
        ]
    );
}

#[test]
fn test_export_defaults_to_the_skills_name_and_never_replaces_a_file() {
    let (library, mem) = world();
    let done = library
        .skill_export(&plain(), "vault#skills/reviewer", "")
        .result
        .unwrap();
    assert_eq!(done.zip_path, "reviewer.zip");
    assert!(mem.user_file("reviewer.zip").is_some());
    let before = mem.user_file("reviewer.zip");
    assert!(
        err(library.skill_export(&plain(), "vault#skills/reviewer", "")).contains("ExportExists")
    );
    assert_eq!(mem.user_file("reviewer.zip"), before);
    assert!(
        err(library.skill_export(&plain(), "vault#skills/never", "x.zip"))
            .contains("SkillNotFound")
    );
    assert!(mem.user_file("x.zip").is_none());
}

fn archive(entries: &[Entry]) -> Vec<u8> {
    pack("reviewer", entries).unwrap()
}

#[test]
fn test_import_lands_the_files_whole_and_records_the_executables_in_the_sidecar() {
    let (library, mem) = world();
    mem.put_user(
        "reviewer.zip",
        &archive(&[
            entry("SKILL.md", SKILL_MD, false),
            entry("scripts/run.sh", "#!/bin/sh\n", true),
            entry("a.bin", "A", false),
        ]),
        false,
    );
    let done = library
        .skill_import(&plain(), "reviewer.zip", "landed", false)
        .result
        .unwrap();
    assert_eq!(
        (
            done.status.as_str(),
            done.bundle.as_str(),
            done.name.as_str(),
            done.file_count
        ),
        ("imported", "landed", "reviewer", 3)
    );
    assert_eq!(
        done.dir,
        "/p/.ctxloom/content/bundles/v2/landed/skills/reviewer"
    );
    assert_eq!(
        mem.text("landed", "skills/reviewer/SKILL.md").unwrap(),
        SKILL_MD
    );
    assert_eq!(
        mem.file("landed", "skills/reviewer/scripts/run.sh")
            .unwrap(),
        (b"#!/bin/sh\n".to_vec(), true)
    );
    assert!(!mem.file("landed", "skills/reviewer/a.bin").unwrap().1);
    assert_eq!(
        mem.text("landed", "skills/.reviewer.meta.yaml").unwrap(),
        "executable:\n  - scripts/run.sh\n"
    );
}

#[test]
fn test_an_import_with_no_executables_writes_no_sidecar() {
    let (library, mem) = world();
    mem.put_user(
        "r.zip",
        &archive(&[entry("SKILL.md", SKILL_MD, false)]),
        false,
    );
    library
        .skill_import(&plain(), "r.zip", "landed", false)
        .result
        .unwrap();
    assert_eq!(mem.text("landed", "skills/.reviewer.meta.yaml"), None);
}

// plan §10 S2: an import over an existing item refuses, and the local edit survives
#[test]
fn test_import_over_a_skill_refuses_unless_forced_and_force_replaces_the_package() {
    let (library, mem) = world();
    mem.put_user(
        "r.zip",
        &archive(&[entry(
            "SKILL.md",
            "---\nname: reviewer\ndescription: NEW\n---\n",
            false,
        )]),
        false,
    );
    let before = mem.paths("vault");
    let said = err(library.skill_import(&plain(), "r.zip", "vault", false));
    assert!(
        said.contains("ImportExists") && said.contains("skill"),
        "{said}"
    );
    assert_eq!(mem.paths("vault"), before);
    assert_eq!(
        mem.text("vault", "skills/reviewer/SKILL.md").unwrap(),
        SKILL_MD
    );
    assert_eq!(mem.writes(), 0);
    library
        .skill_import(&plain(), "r.zip", "vault", true)
        .result
        .unwrap();
    assert!(mem
        .text("vault", "skills/reviewer/SKILL.md")
        .unwrap()
        .contains("NEW"));
    assert_eq!(
        mem.text("vault", "skills/reviewer/scripts/run.sh"),
        None,
        "the package is replaced, not merged"
    );
    assert_eq!(
        mem.text("vault", "skills/.reviewer.meta.yaml").unwrap(),
        "tags:\n  - t\n",
        "the sidecar's other keys stay, the executables go"
    );
    assert!(mem.text("vault", "skills/planner/SKILL.md").is_some());
}

#[test]
fn test_import_refuses_an_archive_that_is_not_one_a_package_without_skill_md_or_a_bad_name() {
    let (library, mem) = world();
    mem.put_user("junk.zip", b"this is not an archive", false);
    assert!(
        err(library.skill_import(&plain(), "junk.zip", "landed", false)).contains("Unrecognized")
    );
    mem.put_user(
        "nomd.zip",
        &archive(&[entry("README.md", "x", false)]),
        false,
    );
    assert!(
        err(library.skill_import(&plain(), "nomd.zip", "landed", false)).contains("SkillInvalid")
    );
    mem.put_user(
        "nofront.zip",
        &archive(&[entry("SKILL.md", "no front matter", false)]),
        false,
    );
    assert!(
        err(library.skill_import(&plain(), "nofront.zip", "landed", false))
            .contains("SkillInvalid")
    );
    mem.put_user(
        "bad.zip",
        &pack(".hidden", &[entry("SKILL.md", SKILL_MD, false)]).unwrap(),
        false,
    );
    assert!(
        err(library.skill_import(&plain(), "bad.zip", "landed", false)).contains("BadSkillName")
    );
    assert!(err(library.skill_import(&plain(), "nosuch.zip", "landed", false)).contains("no file"));
    mem.put_user(
        "ok.zip",
        &archive(&[entry("SKILL.md", SKILL_MD, false)]),
        false,
    );
    assert!(
        err(library.skill_import(&plain(), "ok.zip", "nobundle", false)).contains("BundleNotFound")
    );
    assert_eq!(mem.writes(), 0);
}
