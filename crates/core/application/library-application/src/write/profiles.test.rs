//! `profile.feature`'s write scenarios at the application port.

use std::sync::Arc;

use library::write::profile::ProfileEdit;

use crate::fakes::{plain, Mem, Setup, CONFIG, ENVELOPE};
use crate::write::requests::ProfileCreateRequest;
use crate::write::views::{ProfileEdited, Removal};
use crate::Library;

const DEV: &str = "description: acceptance fixture profile\nbundles:\n  - demo\n";

fn world() -> (Library, Arc<Mem>) {
    let (library, mem) = Setup {
        bundles: &[
            ("demo", &[ENVELOPE, ("fragments/a.md", "A")]),
            (
                "project",
                &[
                    ENVELOPE,
                    ("profiles/dev.yaml", DEV),
                    ("profiles/base.yaml", "bundles:\n  - demo\n"),
                ],
            ),
        ],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    mem.set_editor(
        |_, content| Ok(content.replace("acceptance fixture profile", "EDITED-BY-TEST")),
    );
    (library, mem)
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

fn create(name: &str) -> ProfileCreateRequest {
    ProfileCreateRequest {
        name: name.to_string(),
        bundles: vec!["demo".to_string()],
        ..ProfileCreateRequest::default()
    }
}

// profile.feature: Creating a profile writes its definition and it is listed back
#[test]
fn test_create_writes_the_definition_in_the_project_bundle() {
    let (library, mem) = world();
    let made = library
        .profile_create(
            &plain(),
            &ProfileCreateRequest {
                description: "day-to-day-work".to_string(),
                parents: vec!["base".to_string()],
                llm: "claude-fast".to_string(),
                ..create("new")
            },
        )
        .result
        .unwrap();
    assert_eq!(made.name, "new");
    assert_eq!(made.parents, ["base"]);
    assert_eq!(made.bundles, ["demo"]);
    assert_eq!(
        made.path,
        "/p/.ctxloom/content/bundles/v2/project/profiles/new.yaml"
    );
    assert_eq!(
        mem.text("project", "profiles/new.yaml").unwrap(),
        "description: day-to-day-work\nparents:\n  - base\nllm: claude-fast\nbundles:\n  - demo\n"
    );
    let listed = library.profile_list(&plain()).result.unwrap();
    assert!(listed
        .rows
        .iter()
        .any(|r| r.name == "new" && r.description == "day-to-day-work"));
}

#[test]
fn test_the_project_bundle_is_made_the_first_time_and_only_then() {
    let (library, mem) = Setup {
        bundles: &[("demo", &[ENVELOPE])],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    library
        .profile_create(&plain(), &create("first"))
        .result
        .unwrap();
    assert_eq!(
        mem.text("project", "bundle.yaml").unwrap(),
        "schema_version: 4\nversion: 1.0.0\n"
    );
    assert_eq!(mem.log(), ["claim project", "apply project"]);
    library
        .profile_create(&plain(), &create("second"))
        .result
        .unwrap();
    assert_eq!(
        mem.log(),
        ["claim project", "apply project", "apply project"]
    );
}

#[test]
fn test_create_can_target_another_local_bundle() {
    let (library, mem) = world();
    let made = library
        .profile_create(
            &plain(),
            &ProfileCreateRequest {
                target: "demo".to_string(),
                ..create("elsewhere")
            },
        )
        .result
        .unwrap();
    assert_eq!(
        made.path,
        "/p/.ctxloom/content/bundles/v2/demo/profiles/elsewhere.yaml"
    );
    assert!(mem.text("demo", "profiles/elsewhere.yaml").is_some());
    let out = library.profile_create(
        &plain(),
        &ProfileCreateRequest {
            target: "nosuch".to_string(),
            ..create("x")
        },
    );
    assert!(err(out).contains("BundleNotFound"));
}

// profile.feature: A profile that names neither a bundle nor a parent is refused
#[test]
fn test_create_refuses_what_composes_nothing_and_what_it_cannot_name() {
    let (library, mem) = world();
    let hollow = ProfileCreateRequest {
        name: "hollow".to_string(),
        ..ProfileCreateRequest::default()
    };
    assert!(err(library.profile_create(&plain(), &hollow)).contains("ProfileEmpty"));
    assert!(err(library.profile_create(&plain(), &create("dev"))).contains("ProfileExists"));
    assert!(err(library.profile_create(&plain(), &create("bad#name"))).contains("BadProfileName"));
    for bad in ["a/b", "a\\b", ".x", ""] {
        assert!(
            err(library.profile_create(&plain(), &create(bad))).contains("ProfileNameSegment"),
            "{bad:?}"
        );
    }
    let llm = ProfileCreateRequest {
        llm: "zzz".to_string(),
        ..create("x")
    };
    assert!(err(library.profile_create(&plain(), &llm)).contains("UnknownLlm"));
    assert_eq!(mem.writes(), 0);
}

// profile.feature: Inheriting from a profile that does not exist is refused at creation
#[test]
fn test_create_refuses_a_parent_that_does_not_exist_but_not_a_remote_one() {
    let (library, mem) = world();
    let bad = ProfileCreateRequest {
        parents: vec!["no-such-base".to_string()],
        ..create("x")
    };
    assert!(err(library.profile_create(&plain(), &bad)).contains("ParentNotFound"));
    assert_eq!(mem.writes(), 0);
    let remote = ProfileCreateRequest {
        parents: vec!["ctxloom+git://github.com/o/r//bundles/b@branch:main#profiles/p".to_string()],
        ..create("y")
    };
    library.profile_create(&plain(), &remote).result.unwrap();
    let good = ProfileCreateRequest {
        parents: vec!["base".to_string()],
        ..create("z")
    };
    library.profile_create(&plain(), &good).result.unwrap();
}

// profile.feature: A bundle is attached to a profile and detached again
#[test]
fn test_modify_names_its_changes_and_touches_only_those_lines() {
    let (library, mem) = world();
    let before = mem.text("project", "profiles/base.yaml").unwrap();
    let done = library
        .profile_modify(
            &plain(),
            "dev",
            &ProfileEdit {
                description: Some("updated-desc".to_string()),
                add_bundles: vec!["house-style".to_string()],
                add_parents: vec!["base".to_string()],
                ..ProfileEdit::default()
            },
        )
        .result
        .unwrap();
    assert_eq!(done.name, "dev");
    assert_eq!(
        done.changes,
        [
            "updated description",
            "added parent: base",
            "added bundle: house-style"
        ]
    );
    assert_eq!(
        mem.text("project", "profiles/dev.yaml").unwrap(),
        "description: updated-desc\nbundles:\n  - demo\n  - house-style\nparents:\n  - base\n"
    );
    assert_eq!(mem.text("project", "profiles/base.yaml").unwrap(), before);
    assert_eq!(mem.log(), ["apply project"]);
}

// profile.feature: A modify that was told to change nothing says so
#[test]
fn test_a_modify_with_nothing_to_change_says_so_and_writes_nothing() {
    let (library, mem) = world();
    for edit in [
        ProfileEdit::default(),
        ProfileEdit {
            add_bundles: vec!["demo".to_string()],
            ..ProfileEdit::default()
        },
        ProfileEdit {
            remove_bundles: vec!["nope".to_string()],
            ..ProfileEdit::default()
        },
        ProfileEdit {
            description: Some("acceptance fixture profile".to_string()),
            ..ProfileEdit::default()
        },
    ] {
        let done = library
            .profile_modify(&plain(), "dev", &edit)
            .result
            .unwrap();
        assert!(done.changes.is_empty());
    }
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_modify_refuses_a_profile_nobody_has_a_parent_nobody_has_and_a_label_nobody_knows() {
    let (library, mem) = world();
    let none = ProfileEdit::default();
    assert!(err(library.profile_modify(&plain(), "nosuch", &none)).contains("NotFound"));
    let parent = ProfileEdit {
        add_parents: vec!["nosuch".to_string()],
        ..none.clone()
    };
    assert!(err(library.profile_modify(&plain(), "dev", &parent)).contains("ParentNotFound"));
    let llm = ProfileEdit {
        llm: Some("zzz".to_string()),
        ..none.clone()
    };
    assert!(err(library.profile_modify(&plain(), "dev", &llm)).contains("UnknownLlm"));
    // An empty llm clears; a known one is set.
    let known = ProfileEdit {
        llm: Some("claude-fast".to_string()),
        ..none
    };
    assert_eq!(
        library
            .profile_modify(&plain(), "dev", &known)
            .result
            .unwrap()
            .changes,
        ["set llm to \"claude-fast\""]
    );
    assert_eq!(mem.writes(), 1);
}

// profile.feature: Editing a profile in the configured editor round-trips into the file
#[test]
fn test_edit_keeps_what_the_editor_left_if_it_still_reads() {
    let (library, mem) = world();
    let done = library.profile_edit(&plain(), "dev").result.unwrap();
    assert_eq!(
        done,
        ProfileEdited::Updated {
            path: "/p/.ctxloom/content/bundles/v2/project/profiles/dev.yaml".to_string()
        }
    );
    assert_eq!(
        mem.text("project", "profiles/dev.yaml").unwrap(),
        "description: EDITED-BY-TEST\nbundles:\n  - demo\n"
    );
    assert_eq!(mem.edits(), [("dev.yaml".to_string(), DEV.to_string())]);
}

#[test]
fn test_an_edit_that_changes_nothing_or_breaks_the_profile_writes_nothing() {
    let (library, mem) = world();
    mem.set_editor(|_, content| Ok(content.to_string()));
    assert_eq!(
        library.profile_edit(&plain(), "dev").result.unwrap(),
        ProfileEdited::Unchanged
    );
    mem.set_editor(|_, content| Ok(format!("{content}\nEDITED-BY-TEST\n")));
    assert!(err(library.profile_edit(&plain(), "dev")).contains("Syntax"));
    mem.set_editor(|_, _| Ok("not_a_profile_key: 1\n".to_string()));
    assert!(err(library.profile_edit(&plain(), "dev")).contains("UnknownKey"));
    assert!(err(library.profile_edit(&plain(), "nosuch")).contains("NotFound"));
    assert_eq!(mem.writes(), 0);
}

// profile.feature: Export, remove, and re-import a profile
#[test]
fn test_export_writes_the_definition_where_asked() {
    let (library, mem) = world();
    let done = library
        .profile_export(&plain(), "dev", "pexport")
        .result
        .unwrap();
    assert_eq!(
        done.source,
        "/p/.ctxloom/content/bundles/v2/project/profiles/dev.yaml"
    );
    assert_eq!(done.dest, "pexport/dev.yaml");
    assert_eq!(mem.user_text("pexport/dev.yaml").unwrap(), DEV);
    assert!(err(library.profile_export(&plain(), "nosuch", "x")).contains("NotFound"));
}

#[test]
fn test_import_puts_the_definition_in_the_project_bundle_under_the_files_name() {
    let (library, mem) = world();
    mem.put_user("pexport/shared.yaml", DEV.as_bytes(), false);
    let done = library
        .profile_import(&plain(), "pexport/shared.yaml", false, "")
        .result
        .unwrap();
    assert_eq!(done.source, "pexport/shared.yaml");
    assert_eq!(
        done.dest,
        "/p/.ctxloom/content/bundles/v2/project/profiles/shared.yaml"
    );
    assert_eq!(mem.text("project", "profiles/shared.yaml").unwrap(), DEV);
    mem.put_user("other.yaml", DEV.as_bytes(), false);
    let into = library
        .profile_import(&plain(), "other.yaml", false, "demo")
        .result
        .unwrap();
    assert_eq!(
        into.dest,
        "/p/.ctxloom/content/bundles/v2/demo/profiles/other.yaml"
    );
}

// profile.feature: Importing over an existing profile refuses, and the local edit survives
#[test]
fn test_import_over_a_profile_refuses_unless_forced_and_the_local_edit_survives() {
    let (library, mem) = world();
    mem.put_user(
        "pexport/dev.yaml",
        b"description: SHARED\nbundles:\n  - demo\n",
        false,
    );
    let said = err(library.profile_import(&plain(), "pexport/dev.yaml", false, ""));
    assert!(
        said.contains("ImportExists") && said.contains("profile"),
        "{said}"
    );
    assert_eq!(mem.text("project", "profiles/dev.yaml").unwrap(), DEV);
    assert_eq!(mem.writes(), 0);
    library
        .profile_import(&plain(), "pexport/dev.yaml", true, "")
        .result
        .unwrap();
    assert!(mem
        .text("project", "profiles/dev.yaml")
        .unwrap()
        .contains("SHARED"));
}

#[test]
fn test_import_refuses_what_is_no_profile_and_what_is_not_there() {
    let (library, mem) = world();
    mem.put_user(
        "bad.yaml",
        b"bundles: [demo]\nnot_a_profile_key: 1\n",
        false,
    );
    assert!(err(library.profile_import(&plain(), "bad.yaml", false, "")).contains("UnknownKey"));
    assert!(err(library.profile_import(&plain(), "nosuch.yaml", false, "")).contains("no file"));
    mem.put_user("odd name.yaml", DEV.as_bytes(), false);
    mem.put_user(".hidden.yaml", DEV.as_bytes(), false);
    assert!(
        err(library.profile_import(&plain(), ".hidden.yaml", false, ""))
            .contains("ProfileNameSegment")
    );
    assert!(
        err(library.profile_import(&plain(), "odd name.yaml", false, "nosuch"))
            .contains("BundleNotFound")
    );
    assert_eq!(mem.writes(), 0);
}

// profile.feature: Bare profile remove reports what would go and destroys nothing
#[test]
fn test_remove_without_yes_names_the_bundles_and_destroys_nothing() {
    let (library, mem) = world();
    let Removal::Preview(p) = library
        .profile_remove(&plain(), "dev", false)
        .result
        .unwrap()
    else {
        panic!("a preview");
    };
    assert_eq!(p.target, "profile \"dev\"");
    assert_eq!(p.detail, ["1 bundle(s)"]);
    assert_eq!(p.apply, "ctxloom profile remove dev --yes");
    assert_eq!(mem.writes(), 0);
    assert!(err(library.profile_remove(&plain(), "nosuch", false)).contains("NotFound"));
}

#[test]
fn test_remove_with_yes_takes_that_profile_file_only() {
    let (library, mem) = world();
    let Removal::Done(done) = library
        .profile_remove(&plain(), "dev", true)
        .result
        .unwrap()
    else {
        panic!("done");
    };
    assert_eq!(
        (done.status.as_str(), done.profile.as_str()),
        ("deleted", "dev")
    );
    assert_eq!(mem.text("project", "profiles/dev.yaml"), None);
    assert!(mem.text("project", "profiles/base.yaml").is_some());
    assert!(err(library.profile_remove(&plain(), "dev", true)).contains("NotFound"));
}
