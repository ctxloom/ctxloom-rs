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
            "content-fs-bad-name",
            "content-fs-bad-path",
            "content-fs-write-file",
            "content-fs-remove-path",
            "content-fs-make-dir"
        ]
    );
}

// ---------------------------------------------------------------- the write side (S2)

fn empty_repo() -> (Arc<Rooted>, ContentFs) {
    let fs = rooted();
    let repo = ContentFs::new(fs.clone(), Path::new(APP));
    (fs, repo)
}

fn bundle_file(name: &str, rel: &str) -> std::path::PathBuf {
    Path::new(APP)
        .join(BUNDLES_BELOW_APP_DIR)
        .join(name)
        .join(rel)
}

fn read(fs: &Rooted, name: &str, rel: &str) -> Option<Vec<u8>> {
    fs.read(&bundle_file(name, rel)).ok()
}

fn write(path: &str, bytes: &str) -> Change {
    Change::Write(Put::file(path, bytes))
}

// operations CreateBundle / UpdateBundle: the tree is files, written whole
#[test]
fn test_apply_writes_files_and_makes_their_directories() {
    let (fs, repo) = empty_repo();
    repo.apply(
        "demo",
        &[
            write("bundle.yaml", "version: 1.0.0\n"),
            write("fragments/a.md", "A"),
            write("mcp/.m.meta.yaml", "notes: n\n"),
        ],
    )
    .unwrap();
    assert_eq!(
        read(&fs, "demo", "bundle.yaml").unwrap(),
        b"version: 1.0.0\n"
    );
    assert_eq!(read(&fs, "demo", "fragments/a.md").unwrap(), b"A");
    assert_eq!(
        read(&fs, "demo", "mcp/.m.meta.yaml").unwrap(),
        b"notes: n\n"
    );
    assert_eq!(repo.bundle_names().unwrap(), ["demo"]);
}

#[test]
fn test_apply_replaces_a_file_whole_and_leaves_no_temporary_file() {
    let (fs, repo) = empty_repo();
    repo.apply("demo", &[write("bundle.yaml", "a long first version\n")])
        .unwrap();
    repo.apply("demo", &[write("bundle.yaml", "short\n")])
        .unwrap();
    assert_eq!(read(&fs, "demo", "bundle.yaml").unwrap(), b"short\n");
    let names: Vec<String> = fs
        .read_dir(&bundle_file("demo", ""))
        .unwrap()
        .into_iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["bundle.yaml"]);
}

// the plan: exec bits come from the sidecar only, so a file is made executable
// only when it is asked to be, and plain otherwise
#[test]
fn test_apply_makes_a_file_executable_only_when_asked() {
    let (fs, repo) = empty_repo();
    repo.apply(
        "demo",
        &[
            Change::Write(Put {
                path: "skills/s/scripts/run.sh".to_string(),
                bytes: b"#!/bin/sh\n".to_vec(),
                executable: true,
            }),
            write("skills/s/SKILL.md", "x"),
        ],
    )
    .unwrap();
    let meta = |rel: &str| fs.metadata(&bundle_file("demo", rel)).unwrap();
    assert!(meta("skills/s/scripts/run.sh").executable);
    assert!(!meta("skills/s/SKILL.md").executable);
    assert_eq!(
        repo.executable_files("demo")
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        ["skills/s/scripts/run.sh"]
    );
    // Rewriting the script plain takes the bit away: the write is the whole of it.
    repo.apply("demo", &[write("skills/s/scripts/run.sh", "#!/bin/sh\n")])
        .unwrap();
    assert!(!meta("skills/s/scripts/run.sh").executable);
}

#[test]
fn test_apply_removes_files_and_directories_and_forgives_what_is_not_there() {
    let (fs, repo) = empty_repo();
    repo.apply(
        "demo",
        &[
            write("bundle.yaml", "b"),
            write("fragments/a.md", "A"),
            write("fragments/a.distilled.md", "D"),
            write("skills/s/SKILL.md", "S"),
            write("skills/s/deep/x", "X"),
        ],
    )
    .unwrap();
    repo.apply(
        "demo",
        &[
            Change::Remove {
                path: "fragments/a.md".to_string(),
            },
            Change::Remove {
                path: "fragments/missing.md".to_string(),
            },
            Change::RemoveDir {
                path: "skills/s".to_string(),
            },
            Change::RemoveDir {
                path: "skills/never".to_string(),
            },
        ],
    )
    .unwrap();
    assert!(read(&fs, "demo", "fragments/a.md").is_none());
    assert_eq!(read(&fs, "demo", "fragments/a.distilled.md").unwrap(), b"D");
    assert!(read(&fs, "demo", "skills/s/SKILL.md").is_none());
    assert!(fs.metadata(&bundle_file("demo", "skills/s")).is_err());
    assert_eq!(read(&fs, "demo", "bundle.yaml").unwrap(), b"b");
}

#[test]
fn test_apply_goes_in_order_so_the_last_change_to_a_path_wins() {
    let (fs, repo) = empty_repo();
    repo.apply(
        "demo",
        &[
            write("a", "1"),
            Change::Remove {
                path: "a".to_string(),
            },
            write("b", "1"),
            write("b", "2"),
            write("c", "x"),
            Change::Remove {
                path: "c".to_string(),
            },
            write("c", "y"),
        ],
    )
    .unwrap();
    assert!(read(&fs, "demo", "a").is_none());
    assert_eq!(read(&fs, "demo", "b").unwrap(), b"2");
    assert_eq!(read(&fs, "demo", "c").unwrap(), b"y");
}

// zip-slip's cousin: a path that leaves the bundle is refused before anything is written
#[test]
fn test_apply_refuses_a_path_that_leaves_the_bundle_and_writes_nothing() {
    for bad in [
        "../x",
        "/abs",
        "a/../../b",
        "",
        ".",
        "a//b",
        "./a",
        "a/./b",
        "a\0b",
        "..",
    ] {
        let (fs, repo) = empty_repo();
        let err = repo
            .apply("demo", &[write("fine.md", "ok"), write(bad, "evil")])
            .expect_err(bad);
        assert_eq!(err.code(), "content-fs-bad-path", "{bad:?}");
        assert!(
            read(&fs, "demo", "fine.md").is_none(),
            "{bad:?} wrote the good one"
        );
        let err = repo
            .apply(
                "demo",
                &[Change::Remove {
                    path: bad.to_string(),
                }],
            )
            .expect_err(bad);
        assert_eq!(err.code(), "content-fs-bad-path");
        let err = repo
            .apply(
                "demo",
                &[Change::RemoveDir {
                    path: bad.to_string(),
                }],
            )
            .expect_err(bad);
        assert_eq!(err.code(), "content-fs-bad-path");
    }
}

#[test]
fn test_apply_will_not_write_through_a_link() {
    let (fs, repo) = empty_repo();
    fs.create_dir_all(Path::new("/outside"), Mode::from_bits(0o755))
        .unwrap();
    repo.apply("demo", &[write("bundle.yaml", "b")]).unwrap();
    fs.symlink_dir(Path::new("/outside"), &bundle_file("demo", "skills"))
        .unwrap();
    let err = repo
        .apply("demo", &[write("skills/x.md", "evil")])
        .unwrap_err();
    assert_eq!(err.code(), "content-fs-bad-path");
    assert!(fs.read(Path::new("/outside/x.md")).is_err());
    let err = repo
        .apply(
            "demo",
            &[Change::RemoveDir {
                path: "skills".to_string(),
            }],
        )
        .unwrap_err();
    assert_eq!(err.code(), "content-fs-bad-path");
    assert!(fs.metadata(Path::new("/outside")).is_ok());
}

#[test]
fn test_apply_removes_a_link_itself_when_it_is_the_last_part() {
    let (fs, repo) = empty_repo();
    fs.create_dir_all(Path::new("/outside"), Mode::from_bits(0o755))
        .unwrap();
    repo.apply("demo", &[write("bundle.yaml", "b")]).unwrap();
    fs.symlink_dir(Path::new("/outside"), &bundle_file("demo", "skills"))
        .unwrap();
    repo.apply(
        "demo",
        &[Change::Remove {
            path: "skills".to_string(),
        }],
    )
    .unwrap();
    assert!(fs.metadata(Path::new("/outside")).is_ok());
}

#[test]
fn test_a_bundle_name_that_climbs_is_refused_for_every_write() {
    let (_fs, repo) = empty_repo();
    for bad in ["../x", "/abs", "", "a/../b"] {
        assert_eq!(
            repo.apply(bad, &[]).unwrap_err().code(),
            "content-fs-bad-name",
            "{bad}"
        );
        assert_eq!(
            repo.claim(bad).unwrap_err().code(),
            "content-fs-bad-name",
            "{bad}"
        );
        assert_eq!(
            repo.remove_bundle(bad).unwrap_err().code(),
            "content-fs-bad-name",
            "{bad}"
        );
    }
}

// operations.claimNewBundleDir: one step, one winner
#[test]
fn test_claim_makes_the_directory_once() {
    let (fs, repo) = empty_repo();
    assert!(repo.claim("demo").unwrap());
    assert!(fs.metadata(&bundle_file("demo", "")).unwrap().kind == Kind::Dir);
    assert!(!repo.claim("demo").unwrap());
    // Nested names make their parents; a bundle's name is its path.
    assert!(repo.claim("team/rules").unwrap());
    assert!(!repo.claim("team/rules").unwrap());
    // A file in the way is something there.
    fs.write_atomic(
        &bundle_file("file", ""),
        b"x",
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    assert!(!repo.claim("file").unwrap());
}

#[test]
fn test_remove_bundle_takes_that_tree_and_no_other() {
    let (fs, repo) = empty_repo();
    repo.apply(
        "demo",
        &[write("bundle.yaml", "d"), write("fragments/a.md", "A")],
    )
    .unwrap();
    repo.apply("demo2", &[write("bundle.yaml", "d2")]).unwrap();
    repo.apply("team/rules", &[write("bundle.yaml", "r")])
        .unwrap();
    repo.remove_bundle("demo").unwrap();
    assert!(fs.metadata(&bundle_file("demo", "")).is_err());
    assert_eq!(read(&fs, "demo2", "bundle.yaml").unwrap(), b"d2");
    repo.remove_bundle("demo").unwrap();
    repo.remove_bundle("team/rules").unwrap();
    assert!(fs.metadata(&bundle_file("team/rules", "")).is_err());
    assert_eq!(repo.bundle_names().unwrap(), ["demo2"]);
}

#[test]
fn test_remove_bundle_will_not_follow_a_link_out() {
    let (fs, repo) = empty_repo();
    fs.create_dir_all(Path::new("/outside"), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        Path::new("/outside/keep"),
        b"k",
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    fs.create_dir_all(&bundle_file("", ""), Mode::from_bits(0o755))
        .unwrap();
    fs.symlink_dir(Path::new("/outside"), &bundle_file("evil", ""))
        .unwrap();
    repo.remove_bundle("evil").unwrap();
    assert_eq!(fs.read(Path::new("/outside/keep")).unwrap(), b"k");
}

#[test]
fn test_the_bundle_dir_is_where_the_bundle_is() {
    let (_fs, repo) = empty_repo();
    assert_eq!(
        repo.bundle_dir("team/rules"),
        "/proj/.ctxloom/content/bundles/v2/team/rules"
    );
    assert_eq!(
        repo.manifest_path("demo"),
        format!("{}/bundle.yaml", repo.bundle_dir("demo"))
    );
}

// ---------------------------------------------------------------- the user's files

fn user() -> (Arc<Rooted>, UserFilesFs) {
    let fs = Arc::new(Rooted::temp_with_cwd(Path::new("/work")).expect("a root"));
    fs.create_dir_all(Path::new("/work"), Mode::from_bits(0o755))
        .unwrap();
    let user = UserFilesFs::new(fs.clone());
    (fs, user)
}

#[test]
fn test_a_file_is_written_where_the_user_said_and_read_back() {
    let (fs, user) = user();
    assert!(user.write_file("out/dir/x.yaml", b"X", false).unwrap());
    assert_eq!(user.read_file("out/dir/x.yaml").unwrap(), b"X");
    assert_eq!(fs.read(Path::new("/work/out/dir/x.yaml")).unwrap(), b"X");
    assert!(user.exists("out/dir/x.yaml").unwrap());
    assert!(!user.exists("out/dir/nope").unwrap());
    assert!(user.is_dir("out/dir").unwrap());
    assert!(!user.is_dir("out/dir/x.yaml").unwrap());
    assert!(!user.is_dir("out/nope").unwrap());
    // An absolute path is as given.
    assert!(user.write_file("/abs/y", b"Y", false).unwrap());
    assert_eq!(fs.read(Path::new("/abs/y")).unwrap(), b"Y");
}

#[test]
fn test_a_file_is_not_replaced_unless_it_is_to_be() {
    let (_fs, user) = user();
    assert!(user.write_file("x", b"first", false).unwrap());
    assert!(!user.write_file("x", b"second", false).unwrap());
    assert_eq!(user.read_file("x").unwrap(), b"first");
    assert!(user.write_file("x", b"third", true).unwrap());
    assert_eq!(user.read_file("x").unwrap(), b"third");
}

#[test]
fn test_a_file_that_is_not_there_cannot_be_read() {
    let (_fs, user) = user();
    let err = user.read_file("nope").unwrap_err();
    assert_eq!(err.code(), "content-fs-read-file");
}

#[test]
fn test_a_tree_is_read_whole_sorted_with_its_dotfiles_and_exec_bits() {
    let (fs, user) = user();
    user.write_tree(
        "t",
        &[
            Put::file("z.md", "Z"),
            Put::file("a/.hidden.meta.yaml", "H"),
            Put {
                path: "a/run.sh".to_string(),
                bytes: b"#!/bin/sh".to_vec(),
                executable: true,
            },
            Put::file("a/b/c.txt", "C"),
        ],
    )
    .unwrap();
    assert!(
        fs.metadata(Path::new("/work/t/a/run.sh"))
            .unwrap()
            .executable
    );
    assert!(!fs.metadata(Path::new("/work/t/z.md")).unwrap().executable);
    // A link in the tree is not followed and not listed.
    fs.symlink_dir(Path::new("/work/t/a"), Path::new("/work/t/link"))
        .unwrap();
    let read = user.read_tree("t").unwrap();
    let paths: Vec<&str> = read.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(
        paths,
        ["a/.hidden.meta.yaml", "a/b/c.txt", "a/run.sh", "z.md"]
    );
    let exec: Vec<bool> = read.iter().map(|p| p.executable).collect();
    assert_eq!(exec, [false, false, true, false]);
    assert_eq!(read[1].bytes, b"C");
}

#[test]
fn test_a_tree_that_is_not_there_cannot_be_read() {
    let (_fs, user) = user();
    assert_eq!(
        user.read_tree("nope").unwrap_err().code(),
        "content-fs-list-dir"
    );
}

#[test]
fn test_write_tree_replaces_what_is_there_file_by_file() {
    let (_fs, user) = user();
    user.write_tree("t", &[Put::file("a", "1"), Put::file("b", "2")])
        .unwrap();
    user.write_tree("t", &[Put::file("a", "new")]).unwrap();
    assert_eq!(user.read_file("t/a").unwrap(), b"new");
    assert_eq!(user.read_file("t/b").unwrap(), b"2");
}

// A path that cannot be looked at is an error, not an absence.
#[test]
fn test_a_path_through_a_file_is_an_error_and_not_an_absence() {
    let (_fs, user) = user();
    assert!(user.write_file("plain", b"x", false).unwrap());
    assert!(user.is_dir("plain/child").is_err());
    let (fs, repo) = empty_repo();
    let file = bundle_file("file", "");
    fs.create_dir_all(file.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    fs.write_atomic(
        &file,
        b"x",
        file_system::Perms::Exact(Mode::from_bits(0o644)),
    )
    .unwrap();
    assert_eq!(
        repo.remove_bundle("file").unwrap_err().code(),
        "content-fs-remove-path"
    );
    repo.remove_bundle("never-there").unwrap();
}
