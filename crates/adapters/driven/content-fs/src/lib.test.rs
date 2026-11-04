//! Ported from Go's `internal/adapters/content` tests (`tree_test.go`,
//! `treewalk_characterization_test.go` and the golden
//! `testdata/golden/treewalk.txt`) and `internal/core/paths/bundlewalk.go`
//! (860336c4e), on Go's own fixture tree (a copy in `testdata/tree`).

use std::collections::BTreeMap;
use std::path::Path;

use file_system::{Fs, Rooted};
use os::Mode;

use angzarr_diagnostic::Diagnostic;

use super::*;

/// Go's golden: every file of each fixture bundle, with its size
/// (`  file <path> size=<n> sha=<hex>` under `bundle <name>`).
const TREEWALK: &str = include_str!("../testdata/treewalk.txt");

macro_rules! fixture {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!("../testdata/tree/", $path)).as_slice())),*]
    };
}

const FIXTURE: &[(&str, &[u8])] = fixture![
    "code-quality/fragments/solid.distilled.md",
    "code-quality/fragments/solid.md",
    "code-quality/fragments/tricky.md",
    "code-quality/hooks/pre_tool/audit.yaml",
    "code-quality/hooks/pre_tool/guard.yaml",
    "code-quality/hooks/session_start/greet.yaml",
    "code-quality/mcp/.postgres.meta.yaml",
    "code-quality/mcp/postgres.yaml",
    "code-quality/profiles/strict.yaml",
    "code-quality/prompts/review.md",
    "code-quality/skills/.code-reviewer.meta.yaml",
    "code-quality/skills/code-reviewer/SKILL.md",
    "code-quality/skills/code-reviewer/references/checklist.md",
    "code-quality/skills/code-reviewer/scripts/run.sh",
    "tooling/mcp/redis.yaml",
];

const APP: &str = "/proj/.ctxloom";

fn rooted() -> Arc<Rooted> {
    Arc::new(Rooted::temp().expect("a temporary root"))
}

fn put(fs: &Rooted, path: &str, bytes: &[u8]) {
    let path = Path::new(APP).join(BUNDLES_BELOW_APP_DIR).join(path);
    fs.create_dir_all(path.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        &path,
        bytes,
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
}

/// Go's fixture tree with the envelopes Go's golden test gives it.
fn fixture_repo() -> (Arc<Rooted>, ContentFs) {
    let fs = rooted();
    for (path, bytes) in FIXTURE {
        put(&fs, path, bytes);
    }
    put(
        &fs,
        "code-quality/bundle.yaml",
        b"version: 1.0.0\ndescription: golden corpus code-quality\n",
    );
    put(
        &fs,
        "tooling/bundle.yaml",
        b"version: 1.0.0\ndescription: golden corpus tooling\n",
    );
    let repo = ContentFs::new(fs.clone(), Path::new(APP));
    (fs, repo)
}

/// The golden's `file` lines for `bundle`: path to size.
fn golden_files(bundle: &str) -> BTreeMap<String, usize> {
    let mut sizes = BTreeMap::new();
    let mut in_bundle = false;
    for line in TREEWALK.lines() {
        if let Some(name) = line.strip_prefix("bundle ") {
            in_bundle = name == bundle;
        } else if in_bundle {
            if let Some(rest) = line.trim_start().strip_prefix("file ") {
                let mut parts = rest.split_whitespace();
                let path = parts.next().unwrap();
                let size = parts.next().unwrap().strip_prefix("size=").unwrap();
                sizes.insert(path.to_string(), size.parse().unwrap());
            }
        }
    }
    sizes
}

// tree_test.go: TestTreeStore_Bundles
#[test]
fn test_the_bundles_are_the_directories_holding_a_manifest() {
    let (_fs, repo) = fixture_repo();
    assert_eq!(repo.bundle_names().unwrap(), ["code-quality", "tooling"]);
}

// treewalk.txt: every file of the bundle is in the tree, dot-prefixed sidecars
// included, with the size Go saw. (The golden holds no `bundle.yaml`: Go's
// golden test walks the item tree alone, and the envelope is this fixture's.)
#[test]
fn test_a_tree_holds_every_file_go_s_walk_saw() {
    let (_fs, repo) = fixture_repo();
    for bundle in ["code-quality", "tooling"] {
        let tree = repo.read_tree(bundle).unwrap();
        let mut seen: BTreeMap<String, usize> = tree
            .iter()
            .filter(|(path, _)| path.as_str() != MANIFEST)
            .map(|(path, bytes)| (path.clone(), bytes.len()))
            .collect();
        let golden = golden_files(bundle);
        assert!(!golden.is_empty(), "{bundle}: the golden lists files");
        assert_eq!(seen, golden, "{bundle}");
        seen.clear();
        assert!(
            tree.contains_key(MANIFEST),
            "{bundle}: the envelope is in the tree"
        );
    }
}

// paths.go MetaPath trap: the walk lists directories and never globs, so a
// dot-prefixed sidecar is a file of its item.
#[test]
fn test_a_dot_prefixed_sidecar_is_in_the_tree() {
    let (_fs, repo) = fixture_repo();
    let tree = repo.read_tree("code-quality").unwrap();
    assert!(tree.contains_key("skills/.code-reviewer.meta.yaml"));
    assert!(tree.contains_key("mcp/.postgres.meta.yaml"));
    assert_eq!(
        tree["skills/code-reviewer/scripts/run.sh"],
        b"#!/bin/sh\necho \"reviewing\"\n"
    );
}

// bundlewalk.go ClassifyBundleWalkEntry: a bundle may be nested in a path,
// and the walk stops at a bundle: its items are not further bundles.
#[test]
fn test_a_bundle_can_be_nested_and_its_items_are_not_bundles() {
    let fs = rooted();
    for path in [
        "team/rules/bundle.yaml",
        "team/rules/fragments/one.md",
        // An item that holds a manifest-named file of its own is still an item.
        "team/rules/profiles/coordinator/bundle.yaml",
        "plain/bundle.yaml",
        "not-a-bundle/readme.txt",
    ] {
        put(&fs, path, b"x: 1\n");
    }
    let repo = ContentFs::new(fs.clone(), Path::new(APP));
    assert_eq!(repo.bundle_names().unwrap(), ["plain", "team/rules"]);
    let tree = repo.read_tree("team/rules").unwrap();
    assert!(tree.contains_key("profiles/coordinator/bundle.yaml"));
}

// bundlewalk.go: a FILE is never a bundle, whatever its name
#[test]
fn test_a_file_is_never_a_bundle() {
    let fs = rooted();
    put(&fs, "file-called-bundle", b"not a directory\n");
    put(&fs, "real/bundle.yaml", b"version: 1.0.0\n");
    let repo = ContentFs::new(fs.clone(), Path::new(APP));
    assert_eq!(repo.bundle_names().unwrap(), ["real"]);
}

// environment.md: a read creates nothing, and a project with no bundles
// directory has no bundles
#[test]
fn test_without_a_bundles_directory_there_are_no_bundles_and_nothing_is_made() {
    let fs = rooted();
    let repo = ContentFs::new(fs.clone(), Path::new(APP));
    assert!(!repo.has_root().unwrap());
    assert_eq!(repo.bundle_names().unwrap(), Vec::<String>::new());
    assert!(!fs.exists(Path::new(APP)).unwrap(), "reading made nothing");
    put(&fs, "x/bundle.yaml", b"version: 1.0.0\n");
    assert!(ContentFs::new(fs, Path::new(APP)).has_root().unwrap());
}

// the manifest path shown to the user
#[test]
fn test_the_manifest_path_is_shown_as_the_caller_names_it() {
    let (_fs, repo) = fixture_repo();
    assert_eq!(
        repo.manifest_path("code-quality"),
        "/proj/.ctxloom/content/bundles/v2/code-quality/bundle.yaml"
    );
}

// a name that climbs is no bundle
#[test]
fn test_a_name_that_climbs_is_refused() {
    let (_fs, repo) = fixture_repo();
    let err = repo.read_tree("../../etc").expect_err("climbs");
    assert_eq!(err.code(), "content-fs-bad-name");
}

// skill.go skillManifestMode: a skill file is executable when any execute bit
// is set on disk, whatever a sidecar declares; the listing is read off the
// directory.
#[test]
fn test_the_executable_files_are_those_with_an_execute_bit() {
    let fs = rooted();
    put(&fs, "vault/bundle.yaml", b"version: 1.0.0\n");
    put(
        &fs,
        "vault/skills/reviewer/SKILL.md",
        b"---\nname: reviewer\n---\n",
    );
    let script = Path::new(APP)
        .join(BUNDLES_BELOW_APP_DIR)
        .join("vault/skills/reviewer/scripts/run.sh");
    fs.create_dir_all(script.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        &script,
        b"#!/bin/sh\n",
        file_system::Perms::Exact(Mode::from_bits(0o755)),
    )
    .unwrap();
    let repo = ContentFs::new(fs, Path::new(APP));
    let found: Vec<String> = repo
        .executable_files("vault")
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(found, ["skills/reviewer/scripts/run.sh"]);
}

// bundlewalk.go: a name is no bundle's path when it is empty, absolute, or
// has a `.` or `..` step; each alone is enough.
#[test]
fn test_each_kind_of_bad_name_is_refused_on_its_own() {
    let (_fs, repo) = fixture_repo();
    for name in ["", "/etc", "a/../b", "a/./b", "..", "."] {
        let err = repo.read_tree(name).expect_err(name);
        assert_eq!(err.code(), "content-fs-bad-name", "{name:?}");
    }
    assert!(repo.read_tree("code-quality").is_ok());
}

// A bundles directory that cannot even be looked at (a file stands where a
// directory should) is an error naming it, not an absent directory.
#[test]
fn test_a_bundles_directory_that_cannot_be_inspected_is_an_error() {
    let fs = rooted();
    let blocker = Path::new(APP).join("content");
    fs.create_dir_all(blocker.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        &blocker,
        b"in the way",
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    let repo = ContentFs::new(fs, Path::new(APP));
    let err = repo.has_root().expect_err("not a directory");
    assert_eq!(err.code(), "content-fs-list-root");
    assert!(err.args().iter().any(|(k, _)| *k == "ext_err_txt"));
}

// The errors say their code and their args.
#[test]
fn test_the_errors_carry_their_args_and_their_samples_are_all_of_them() {
    for error in [
        ContentError::ListRoot {
            path: "/p".to_string(),
            detail: "d".to_string(),
        },
        ContentError::ListDir {
            path: "/p".to_string(),
            detail: "d".to_string(),
        },
        ContentError::ReadFile {
            path: "/p".to_string(),
            detail: "d".to_string(),
        },
    ] {
        assert_eq!(error.args().len(), 2);
        assert!(error.to_string().contains(error.code()));
    }
    let bad = ContentError::BadName {
        name: "../x".to_string(),
    };
    assert_eq!(bad.args().len(), 1);
    assert!(bad.to_string().contains("content-fs-bad-name"));
    let codes: Vec<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    assert_eq!(
        codes,
        [
            "content-fs-list-root",
            "content-fs-list-dir",
            "content-fs-read-file",
            "content-fs-bad-name"
        ]
    );
}
