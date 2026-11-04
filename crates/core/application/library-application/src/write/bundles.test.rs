//! `bundle.feature`'s write scenarios at the application port, over the
//! in-memory ports: what is written, what is refused, and that a preview and a
//! no-op edit call nothing that writes.

use crate::fakes::{plain, Mem, Setup, CONFIG, ENVELOPE};
use crate::write::requests::BundleEditRequest;
use crate::write::views::Removal;
use crate::Library;

use std::sync::Arc;

const DEMO: &[(&str, &str)] = &[
    ENVELOPE,
    ("fragments/example.md", "EXAMPLE"),
    ("fragments/old.md", "OLD"),
    ("fragments/old.distilled.md", "OLD-DISTILLED"),
    ("prompts/review.md", "REVIEW"),
    ("mcp/m.yaml", "command: m\n"),
    ("mcp/.m.meta.yaml", "notes: n\n"),
    (
        "skills/s/SKILL.md",
        "---\nname: s\ndescription: d\n---\nbody",
    ),
    ("profiles/p.yaml", "bundles: [demo]\n"),
];

fn world() -> (Library, Arc<Mem>) {
    Setup {
        bundles: &[
            ("demo", DEMO),
            ("other", &[ENVELOPE, ("fragments/x.md", "X")]),
        ],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture()
}

fn err<T>(out: crate::Outcome<T>) -> String {
    format!("{:?}", out.result.err().expect("an error"))
}

fn edit(name: &str) -> BundleEditRequest {
    BundleEditRequest {
        name: name.to_string(),
        ..BundleEditRequest::default()
    }
}

// bundle.feature: Creating a bundle writes a manifest carrying its seeded content
#[test]
fn test_create_claims_the_directory_and_writes_the_envelope_and_the_seeds() {
    let (library, mem) = world();
    let made = library
        .bundle_create(&plain(), "fresh", "acceptance fixture bundle")
        .result
        .unwrap();
    assert_eq!(
        (made.status.as_str(), made.name.as_str(), made.path.as_str()),
        (
            "created",
            "fresh",
            "/p/.ctxloom/content/bundles/v2/fresh/bundle.yaml"
        )
    );
    assert_eq!(
        mem.text("fresh", "bundle.yaml").unwrap(),
        "schema_version: 4\nversion: 1.0.0\ndescription: acceptance fixture bundle\n"
    );
    assert!(mem
        .text("fresh", "fragments/example.md")
        .unwrap()
        .contains("# Example Fragment"));
    assert!(mem
        .text("fresh", "prompts/example.md")
        .unwrap()
        .contains("Example prompt content."));
    assert_eq!(mem.log(), ["claim fresh", "apply fresh"]);
    // What it wrote reads as a bundle with one of each.
    let list = library.bundle_list(&plain()).result.unwrap();
    let row = list.rows.iter().find(|r| r.name == "fresh").unwrap();
    assert_eq!((row.fragment_count, row.command_count), (1, 1));
}

#[test]
fn test_create_without_a_description_writes_an_envelope_without_one() {
    let (library, mem) = world();
    library.bundle_create(&plain(), "fresh", "").result.unwrap();
    assert_eq!(
        mem.text("fresh", "bundle.yaml").unwrap(),
        "schema_version: 4\nversion: 1.0.0\n"
    );
}

// operations.CreateBundle: an existing bundle is refused and untouched
#[test]
fn test_create_refuses_a_bundle_that_is_there_and_leaves_it_alone() {
    let (library, mem) = world();
    let out = library.bundle_create(&plain(), "demo", "x");
    assert!(err(out).contains("BundleExists"));
    assert_eq!(mem.text("demo", "fragments/example.md").unwrap(), "EXAMPLE");
    assert_eq!(mem.writes(), 0, "the claim failed, so nothing was written");
}

#[test]
fn test_create_refuses_a_name_that_climbs_and_writes_nothing() {
    let (library, mem) = world();
    for bad in ["../evil", "/abs", "a/../b", "", "a//b", "./a"] {
        assert!(
            err(library.bundle_create(&plain(), bad, "")).contains("BadBundleName"),
            "{bad:?}"
        );
    }
    assert_eq!(mem.writes(), 0);
    library
        .bundle_create(&plain(), "team/rules", "")
        .result
        .unwrap();
    assert!(mem.has_bundle("team/rules"));
}

#[test]
fn test_create_needs_a_project() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        home_only: true,
        ..Setup::default()
    }
    .fixture();
    assert!(err(library.bundle_create(&plain(), "x", "")).contains("NoProject"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_a_write_that_fails_in_the_port_leaves_no_half_made_bundle() {
    let (library, mem) = world();
    mem.refuse_apply("disk full");
    let out = library.bundle_create(&plain(), "fresh", "");
    assert!(out.result.is_err());
    assert!(!mem.has_bundle("fresh"), "the claim is given back");
    assert_eq!(mem.log(), ["claim fresh", "remove_bundle fresh"]);
}

// bundle.feature: Items are attached and detached by name, and the manifest follows
#[test]
fn test_edit_attaches_items_with_go_s_placeholders_and_names_each_change() {
    let (library, mem) = world();
    let edited = library
        .bundle_edit(
            &plain(),
            &BundleEditRequest {
                version: Some("2.0.0".to_string()),
                add_tags: vec!["golang".to_string()],
                add_fragments: vec!["coding-standards".to_string()],
                add_prompts: vec!["new".to_string()],
                add_mcp: vec!["tree-sitter".to_string()],
                ..edit("demo")
            },
        )
        .result
        .unwrap();
    assert_eq!(edited.status, "updated");
    assert_eq!(
        edited.changes,
        [
            "updated version",
            "added tag: golang",
            "set fragment: coding-standards",
            "set prompt: new",
            "set mcp: tree-sitter"
        ]
    );
    assert_eq!(
        edited.path,
        "/p/.ctxloom/content/bundles/v2/demo/bundle.yaml"
    );
    assert_eq!(
        mem.text("demo", "fragments/coding-standards.md").unwrap(),
        "# coding-standards\n\nAdd content here."
    );
    assert_eq!(
        mem.text("demo", "prompts/new.md").unwrap(),
        "---\ndescription: new\n---\nAdd prompt content here."
    );
    assert_eq!(
        mem.text("demo", "mcp/tree-sitter.yaml").unwrap(),
        "command: tree-sitter\n"
    );
    assert!(mem
        .text("demo", "bundle.yaml")
        .unwrap()
        .contains("version: 2.0.0"));
    assert_eq!(mem.log(), ["apply demo"], "one write");
}

#[test]
fn test_edit_detaches_items_with_their_other_files() {
    let (library, mem) = world();
    let edited = library
        .bundle_edit(
            &plain(),
            &BundleEditRequest {
                remove_fragments: vec!["old".to_string()],
                remove_prompts: vec!["review".to_string()],
                remove_mcp: vec!["m".to_string(), "nope".to_string()],
                ..edit("demo")
            },
        )
        .result
        .unwrap();
    assert_eq!(
        edited.changes,
        [
            "removed fragment: old",
            "removed prompt: review",
            "removed mcp: m"
        ]
    );
    for gone in [
        "fragments/old.md",
        "fragments/old.distilled.md",
        "prompts/review.md",
        "mcp/m.yaml",
        "mcp/.m.meta.yaml",
    ] {
        assert_eq!(mem.text("demo", gone), None, "{gone}");
    }
    assert_eq!(mem.text("demo", "fragments/example.md").unwrap(), "EXAMPLE");
    assert!(mem.text("demo", "skills/s/SKILL.md").is_some());
}

// operations.applyFragmentEdits: an add of a name that is there is not a change
#[test]
fn test_edit_adds_only_what_is_not_there() {
    let (library, mem) = world();
    let edited = library
        .bundle_edit(
            &plain(),
            &BundleEditRequest {
                add_fragments: vec!["example".to_string(), "fresh".to_string()],
                ..edit("demo")
            },
        )
        .result
        .unwrap();
    assert_eq!(edited.changes, ["set fragment: fresh"]);
    assert_eq!(mem.text("demo", "fragments/example.md").unwrap(), "EXAMPLE");
}

// bundle.feature: An edit that was told to change nothing says so
#[test]
fn test_an_edit_with_nothing_to_change_says_so_and_writes_nothing() {
    let (library, mem) = world();
    for request in [
        edit("demo"),
        BundleEditRequest {
            version: Some("1.0.0".to_string()),
            ..edit("demo")
        },
        BundleEditRequest {
            add_fragments: vec!["example".to_string()],
            ..edit("demo")
        },
        BundleEditRequest {
            remove_fragments: vec!["nope".to_string()],
            ..edit("demo")
        },
        BundleEditRequest {
            remove_tags: vec!["nope".to_string()],
            ..edit("demo")
        },
    ] {
        let done = library.bundle_edit(&plain(), &request).result.unwrap();
        assert_eq!(done.status, "no_changes");
        assert!(done.changes.is_empty());
        assert_eq!(done.path, "/p/.ctxloom/content/bundles/v2/demo/bundle.yaml");
    }
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_edit_refuses_a_bundle_nobody_has_and_a_name_that_is_no_item() {
    let (library, mem) = world();
    assert!(err(library.bundle_edit(&plain(), &edit("nosuch"))).contains("BundleNotFound"));
    for bad in ["a/b", ".hidden", ""] {
        let out = library.bundle_edit(
            &plain(),
            &BundleEditRequest {
                add_fragments: vec![bad.to_string()],
                ..edit("demo")
            },
        );
        assert!(err(out).contains("BadItemName"), "{bad:?}");
    }
    assert_eq!(mem.writes(), 0);
}

// bundle.feature: Removing / Bare bundle remove reports what would go
#[test]
fn test_remove_without_yes_names_the_cost_and_destroys_nothing() {
    let (library, mem) = world();
    let Removal::Preview(preview) = library
        .bundle_remove(&plain(), "demo", false)
        .result
        .unwrap()
    else {
        panic!("a preview");
    };
    assert_eq!(preview.status, "preview");
    assert!(!preview.applied);
    assert_eq!(preview.target, "bundle \"demo\"");
    assert_eq!(
        preview.detail,
        ["2 fragment(s), 1 command(s), 1 MCP server(s), 1 skill package(s), 1 profile(s)"]
    );
    assert_eq!(preview.apply, "ctxloom bundle remove demo --yes");
    assert_eq!(mem.writes(), 0);
    assert!(mem.text("demo", "bundle.yaml").is_some());
}

#[test]
fn test_remove_of_an_empty_bundle_names_no_detail() {
    let (library, _mem) = Setup {
        bundles: &[("empty", &[ENVELOPE])],
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    let Removal::Preview(preview) = library
        .bundle_remove(&plain(), "empty", false)
        .result
        .unwrap()
    else {
        panic!("a preview");
    };
    assert!(preview.detail.is_empty());
}

#[test]
fn test_remove_with_yes_takes_the_whole_tree_and_no_other() {
    let (library, mem) = world();
    let Removal::Done(done) = library
        .bundle_remove(&plain(), "demo", true)
        .result
        .unwrap()
    else {
        panic!("done");
    };
    assert_eq!(
        (done.status.as_str(), done.name.as_str(), done.path.as_str()),
        (
            "deleted",
            "demo",
            "/p/.ctxloom/content/bundles/v2/demo/bundle.yaml"
        )
    );
    assert!(!mem.has_bundle("demo"));
    assert!(mem.has_bundle("other"));
    assert_eq!(mem.text("other", "fragments/x.md").unwrap(), "X");
    assert_eq!(mem.log(), ["remove_bundle demo"]);
}

#[test]
fn test_remove_of_a_bundle_nobody_has_is_refused_with_and_without_yes() {
    let (library, mem) = world();
    for yes in [false, true] {
        assert!(err(library.bundle_remove(&plain(), "nosuch", yes)).contains("BundleNotFound"));
    }
    assert_eq!(mem.writes(), 0);
}

// bundle.feature: Export, remove, and re-import a bundle
#[test]
fn test_export_copies_the_whole_tree_under_the_bundles_own_name() {
    let (library, mem) = Setup {
        bundles: &[("demo", DEMO)],
        layers: &[CONFIG],
        executable: &["skills/s/SKILL.md"],
        ..Setup::default()
    }
    .fixture();
    let done = library
        .bundle_export(&plain(), "demo", "exported", "")
        .result
        .unwrap();
    assert_eq!(done.status, "exported");
    assert_eq!(done.name, "demo");
    assert_eq!(
        done.source,
        "/p/.ctxloom/content/bundles/v2/demo/bundle.yaml"
    );
    assert_eq!(done.dest, "exported/demo");
    let mut want: Vec<String> = DEMO
        .iter()
        .map(|(p, _)| format!("exported/demo/{p}"))
        .collect();
    want.sort();
    assert_eq!(mem.user_paths(), want);
    assert_eq!(
        mem.user_text("exported/demo/fragments/example.md").unwrap(),
        "EXAMPLE"
    );
    assert!(
        mem.user_file("exported/demo/skills/s/SKILL.md").unwrap().1,
        "the exec bit travels"
    );
    assert!(!mem.user_file("exported/demo/bundle.yaml").unwrap().1);
}

#[test]
fn test_export_with_an_output_path_names_the_tree_outright() {
    let (library, mem) = world();
    let done = library
        .bundle_export(&plain(), "demo", "", "handoff")
        .result
        .unwrap();
    assert_eq!(done.dest, "handoff");
    assert!(mem.user_text("handoff/bundle.yaml").is_some());
    assert!(mem.user_text("handoff/demo/bundle.yaml").is_none());
    // The output wins over a destination directory.
    let both = library
        .bundle_export(&plain(), "demo", "ignored", "handoff2")
        .result
        .unwrap();
    assert_eq!(both.dest, "handoff2");
}

#[test]
fn test_export_needs_a_destination_and_a_bundle() {
    let (library, mem) = world();
    assert!(err(library.bundle_export(&plain(), "demo", "", "")).contains("ExportNeedsDest"));
    assert!(err(library.bundle_export(&plain(), "nosuch", "out", "")).contains("BundleNotFound"));
    assert!(mem.user_paths().is_empty());
}

// bundle.feature: Importing over an existing bundle refuses, and the local edit survives
fn exported_demo(mem: &Mem) {
    for (path, text) in DEMO {
        mem.put_user(&format!("in/demo/{path}"), text.as_bytes(), false);
    }
}

#[test]
fn test_import_copies_a_tree_in_under_its_own_name() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    exported_demo(&mem);
    mem.put_user("in/demo/skills/s/run.sh", b"#!/bin/sh", true);
    let done = library
        .bundle_import(&plain(), "in/demo", false)
        .result
        .unwrap();
    assert_eq!(done.status, "imported");
    assert_eq!(done.source, "in/demo");
    assert_eq!(done.dest, "/p/.ctxloom/content/bundles/v2/demo");
    assert_eq!(done.version, "1.0.0");
    assert_eq!((done.fragments, done.commands, done.mcp), (2, 1, 1));
    assert_eq!(mem.text("demo", "fragments/example.md").unwrap(), "EXAMPLE");
    assert!(
        mem.file("demo", "skills/s/run.sh").unwrap().1,
        "the exec bit travels"
    );
    assert_eq!(mem.log(), ["claim demo", "apply demo"]);
}

#[test]
fn test_import_takes_the_bundle_yaml_path_as_its_directory() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    exported_demo(&mem);
    let done = library
        .bundle_import(&plain(), "in/demo/bundle.yaml", false)
        .result
        .unwrap();
    assert_eq!(done.source, "in/demo");
    assert!(mem.has_bundle("demo"));
}

#[test]
fn test_import_over_a_bundle_refuses_and_the_local_edit_survives() {
    let (library, mem) = world();
    exported_demo(&mem);
    mem.put_user("in/demo/fragments/example.md", b"SHARED", false);
    let out = library.bundle_import(&plain(), "in/demo", false);
    let said = err(out);
    assert!(
        said.contains("ImportExists") && said.contains("bundle"),
        "{said}"
    );
    assert_eq!(mem.text("demo", "fragments/example.md").unwrap(), "EXAMPLE");
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_a_forced_import_replaces_the_whole_tree() {
    let (library, mem) = world();
    exported_demo(&mem);
    mem.put_user("in/demo/fragments/example.md", b"SHARED", false);
    library
        .bundle_import(&plain(), "in/demo", true)
        .result
        .unwrap();
    assert_eq!(mem.text("demo", "fragments/example.md").unwrap(), "SHARED");
    // A file only the old copy had is gone: the tree is replaced, not merged.
    let (library2, mem2) = world();
    for (path, text) in DEMO
        .iter()
        .filter(|(p, _)| *p != "fragments/old.md" && *p != "fragments/old.distilled.md")
    {
        mem2.put_user(&format!("in/demo/{path}"), text.as_bytes(), false);
    }
    library2
        .bundle_import(&plain(), "in/demo", true)
        .result
        .unwrap();
    assert_eq!(mem2.text("demo", "fragments/old.md"), None);
    assert_eq!(mem2.text("demo", "fragments/old.distilled.md"), None);
    assert_eq!(
        mem2.text("demo", "fragments/example.md").unwrap(),
        "EXAMPLE"
    );
}

#[test]
fn test_import_refuses_what_is_no_bundle_or_does_not_read() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    assert!(err(library.bundle_import(&plain(), "nowhere", false)).contains("no directory"));
    mem.put_user("loose/file.md", b"x", false);
    assert!(err(library.bundle_import(&plain(), "loose", false)).contains("NotABundle"));
    mem.put_user("bad/bundle.yaml", b"version: [unclosed\n", false);
    assert!(err(library.bundle_import(&plain(), "bad", false)).contains("ImportUnreadable"));
    mem.put_user(
        "unclaimed/bundle.yaml",
        b"schema_version: 4\nversion: 1.0.0\n",
        false,
    );
    mem.put_user("unclaimed/fragments/sub/file.txt", b"x", false);
    assert!(err(library.bundle_import(&plain(), "unclaimed", false)).contains("ImportUnreadable"));
    assert_eq!(mem.writes(), 0);
}

#[test]
fn test_import_will_not_take_a_name_that_is_no_bundles() {
    let (library, mem) = Setup {
        layers: &[CONFIG],
        ..Setup::default()
    }
    .fixture();
    mem.put_user(
        "dir/.hidden/bundle.yaml",
        b"schema_version: 4\nversion: 1.0.0\n",
        false,
    );
    // The name is the directory's: ".hidden" is not a bundle's.
    assert!(err(library.bundle_import(&plain(), "dir/.hidden", false)).contains("BadBundleName"));
    assert_eq!(mem.writes(), 0);
}
