use super::*;

const A_SHA: &str = "1111111aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B_SHA: &str = "2222222bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn file(data: &str, executable: bool) -> TreeFile {
    TreeFile {
        data: data.as_bytes().to_vec(),
        executable,
    }
}

fn tree(files: &[(&str, &str, bool)]) -> Tree {
    files
        .iter()
        .map(|(path, data, exec)| (path.to_string(), file(data, *exec)))
        .collect()
}

const ENVELOPE: &str = "schema_version: 4\nversion: 1.0.0\n";

// pin_change.go diffPin: a first pin is everything the bundle carries.
#[test]
fn test_a_first_pin_lists_every_item_and_file() {
    let to = tree(&[
        ("bundle.yaml", ENVELOPE, false),
        ("fragments/a.md", "a\n", false),
    ]);
    let (change, warnings) = diff_pin(
        "ctxloom+git://h/r//bundles/demo",
        "https://h/r",
        None,
        Side {
            sha: A_SHA,
            version: "",
            tree: &to,
        },
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(change.from_sha, "");
    assert_eq!(change.to_sha, A_SHA);
    assert_eq!(change.items.len(), 1);
    assert_eq!(
        (change.items[0].kind, change.items[0].change),
        ("fragment", Change::Added)
    );
    let paths: Vec<_> = change
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.change))
        .collect();
    assert_eq!(
        paths,
        vec![
            ("bundle.yaml", Change::Added),
            ("fragments/a.md", Change::Added)
        ]
    );
}

#[test]
fn test_a_move_lists_what_differs_between_the_two_trees() {
    let from = tree(&[
        ("bundle.yaml", ENVELOPE, false),
        ("fragments/a.md", "a\n", false),
        ("fragments/gone.md", "g\n", false),
    ]);
    let to = tree(&[
        ("bundle.yaml", ENVELOPE, false),
        ("fragments/a.md", "A\n", false),
    ]);
    let (change, _) = diff_pin(
        "ctxloom+git://h/r//bundles/demo",
        "https://h/r",
        Some(Side {
            sha: A_SHA,
            version: "v1",
            tree: &from,
        }),
        Side {
            sha: B_SHA,
            version: "v2",
            tree: &to,
        },
    );
    assert_eq!(
        (change.from_sha.as_str(), change.from_version.as_str()),
        (A_SHA, "v1")
    );
    assert_eq!(
        (change.to_sha.as_str(), change.to_version.as_str()),
        (B_SHA, "v2")
    );
    let items: Vec<_> = change
        .items
        .iter()
        .map(|i| (i.name.as_str(), i.change))
        .collect();
    assert_eq!(
        items,
        vec![("a", Change::Modified), ("gone", Change::Removed)]
    );
    let files: Vec<_> = change
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.change))
        .collect();
    assert_eq!(
        files,
        vec![
            ("fragments/a.md", Change::Modified),
            ("fragments/gone.md", Change::Removed)
        ]
    );
}

// diffTreeFiles / isScript: only a script gets a diff: an executable file, or
// one under scripts/.
#[test]
fn test_only_a_script_is_shown_as_a_diff() {
    let from = tree(&[
        ("bundle.yaml", ENVELOPE, false),
        (
            "skills/runner/SKILL.md",
            "---\nname: runner\ndescription: d\n---\nbefore\n",
            false,
        ),
        (
            "skills/runner/scripts/run.sh",
            "#!/bin/sh\necho ONE\n",
            false,
        ),
        ("tools/x", "one\n", true),
        ("docs/plain.txt", "one\n", false),
    ]);
    let to = tree(&[
        ("bundle.yaml", ENVELOPE, false),
        (
            "skills/runner/SKILL.md",
            "---\nname: runner\ndescription: d\n---\nafter\n",
            false,
        ),
        (
            "skills/runner/scripts/run.sh",
            "#!/bin/sh\necho TWO\n",
            false,
        ),
        ("tools/x", "two\n", true),
        ("docs/plain.txt", "two\n", false),
    ]);
    let (change, _) = diff_pin(
        "i",
        "u",
        Some(Side {
            sha: A_SHA,
            version: "",
            tree: &from,
        }),
        Side {
            sha: B_SHA,
            version: "",
            tree: &to,
        },
    );
    let diffed: Vec<_> = change
        .files
        .iter()
        .filter(|f| !f.diff.is_empty())
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(diffed, vec!["skills/runner/scripts/run.sh", "tools/x"]);
    let script = change
        .files
        .iter()
        .find(|f| f.path.ends_with("run.sh"))
        .expect("the script");
    assert!(
        script.diff.contains("-echo ONE\n+echo TWO\n"),
        "{}",
        script.diff
    );
}

// A file whose only change is its exec bit is a change.
#[test]
fn test_a_file_that_became_executable_is_modified() {
    let from = tree(&[("bundle.yaml", ENVELOPE, false), ("t", "x\n", false)]);
    let to = tree(&[("bundle.yaml", ENVELOPE, false), ("t", "x\n", true)]);
    let (change, _) = diff_pin(
        "i",
        "u",
        Some(Side {
            sha: A_SHA,
            version: "",
            tree: &from,
        }),
        Side {
            sha: B_SHA,
            version: "",
            tree: &to,
        },
    );
    assert_eq!(change.files.len(), 1);
    assert_eq!(change.files[0].change, Change::Modified);
}

// pinChanges: an entry whose content cannot be read as a bundle is still
// listed, by its header and files, and the reason is warned.
#[test]
fn test_a_tree_that_is_not_a_bundle_is_listed_by_its_files_with_a_warning() {
    let to = tree(&[("README.md", "hi\n", false)]);
    let (change, unreadable) = diff_pin(
        "ctxloom+git://h/r//bundles/demo",
        "https://h/r",
        None,
        Side {
            sha: A_SHA,
            version: "",
            tree: &to,
        },
    );
    assert!(change.items.is_empty());
    assert_eq!(change.files.len(), 1);
    assert_eq!(unreadable.len(), 1);
    assert_eq!(unreadable[0].identity, "ctxloom+git://h/r//bundles/demo");
    assert_eq!(unreadable[0].sha, A_SHA);
}

// The disclosure's json carries the same fields as Go's.
#[test]
fn test_the_change_serializes_as_gos_does() {
    let to = tree(&[
        ("bundle.yaml", ENVELOPE, false),
        ("mcp/srv.yaml", "command: srv\nenv:\n  K: v\n", false),
    ]);
    let (change, _) = diff_pin(
        "id",
        "u",
        None,
        Side {
            sha: A_SHA,
            version: "",
            tree: &to,
        },
    );
    let json = serde_json::to_value(&change).expect("json");
    assert_eq!(json["identity"], "id");
    assert_eq!(json["from_sha"], "");
    assert_eq!(json["to_sha"], A_SHA);
    assert_eq!(json["items"][0]["kind"], "mcp");
    assert_eq!(json["items"][0]["change"], "added");
    assert_eq!(json["items"][0]["exec"]["before"], serde_json::Value::Null);
    assert_eq!(json["items"][0]["exec"]["after"]["command"], "srv");
    assert_eq!(
        json["items"][0]["exec"]["after"]["env"]["K"],
        fingerprint("v")
    );
    assert_eq!(json["files"][0]["path"], "bundle.yaml");
    assert!(json["files"][0].get("diff").is_none());
}
