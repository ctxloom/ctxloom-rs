//! The project's files over a rooted filesystem.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use deps_ports::{CopyOutcome, DepsFiles, RepoFile, RepoTree};
use file_system::{Fs, Rooted};
use os::Mode;

use super::*;

const IDENTITY: &str = "ctxloom+git://example.test/o/r//bundles/demo";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const NEXT: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn files() -> (ProjectFiles, Arc<Rooted>) {
    let fs = Arc::new(Rooted::temp().expect("a temporary root"));
    let files = ProjectFiles::new(fs.clone(), PathBuf::from("/proj/.ctxloom"));
    (files, fs)
}

fn put(fs: &Rooted, path: &str, text: &str) {
    let at = Path::new(path);
    fs.create_dir_all(at.parent().expect("a parent"), Mode::from_bits(0o755))
        .expect("a directory");
    fs.write_atomic(at, text.as_bytes(), Perms::Exact(Mode::from_bits(0o644)))
        .expect("a file");
}

fn tree(files: &[(&str, &str, bool)]) -> RepoTree {
    files
        .iter()
        .map(|(path, text, executable)| {
            (
                path.to_string(),
                RepoFile {
                    data: text.as_bytes().to_vec(),
                    executable: *executable,
                },
            )
        })
        .collect()
}

fn demo_tree(text: &str) -> RepoTree {
    tree(&[
        ("bundle.yaml", "version: 1\n", false),
        ("fragments/a.md", text, false),
        ("scripts/run.sh", "#!/bin/sh\n", true),
    ])
}

// remote.feature: Registering a first remote outside any project creates a
// usable home registry
#[test]
fn test_the_registry_is_read_when_there_and_written_atomically_into_a_made_directory() {
    let (files, fs) = files();
    assert_eq!(files.read_registry(), Ok(None));
    files.write_registry("schema_version: 1\n").expect("writes");
    assert_eq!(
        files.read_registry(),
        Ok(Some("schema_version: 1\n".to_string()))
    );
    assert_eq!(files.registry_path(), "/proj/.ctxloom/remotes.yaml");
    let dir = fs
        .metadata(Path::new("/proj/.ctxloom"))
        .expect("the app dir");
    assert_eq!(
        dir.mode.map(|m| m.bits() & 0o700),
        Some(0o700),
        "usable by its owner"
    );
    files
        .write_registry("schema_version: 1\nnote: x\n")
        .expect("rewrites");
    assert_eq!(
        files.read_registry(),
        Ok(Some("schema_version: 1\nnote: x\n".to_string()))
    );
}

#[test]
fn test_the_lock_is_a_file_of_its_own() {
    let (files, _) = files();
    assert_eq!(files.read_lock(), Ok(None));
    assert_eq!(files.lock_path(), "/proj/.ctxloom/lock.yaml");
    files.write_lock("schema_version: 3\n").expect("writes");
    assert_eq!(
        files.read_lock(),
        Ok(Some("schema_version: 3\n".to_string()))
    );
    assert_eq!(files.read_registry(), Ok(None));
}

#[test]
fn test_a_file_that_cannot_be_read_is_an_error_not_an_absence() {
    let (files, fs) = files();
    fs.create_dir_all(
        Path::new("/proj/.ctxloom/lock.yaml"),
        Mode::from_bits(0o755),
    )
    .expect("a directory in its place");
    assert!(files.read_lock().is_err());
}

#[test]
fn test_an_installed_tree_is_read_back_with_its_modes_and_without_its_marker() {
    let (files, _) = files();
    assert!(!files.is_installed(IDENTITY, SHA));
    assert_eq!(files.read_installed(IDENTITY, SHA), Ok(None));
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    assert!(files.is_installed(IDENTITY, SHA));
    assert!(!files.is_installed(IDENTITY, NEXT));
    let read = files
        .read_installed(IDENTITY, SHA)
        .expect("reads")
        .expect("a tree");
    assert_eq!(read, demo_tree("A\n"));
    assert!(!read.contains_key(IDENTITY_MARKER));
    assert_eq!(
        files.installed_path(IDENTITY, SHA),
        format!("/proj/.ctxloom/cache/bundles/example.test/o/r/demo/{SHA}")
    );
}

#[test]
fn test_installing_a_new_commit_replaces_what_was_installed_for_the_identity() {
    let (files, _) = files();
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    let other = "ctxloom+git://example.test/o/r//bundles/other";
    files
        .install(other, SHA, &demo_tree("O\n"))
        .expect("installs");
    files
        .install(IDENTITY, NEXT, &demo_tree("B\n"))
        .expect("installs");
    assert!(
        !files.is_installed(IDENTITY, SHA),
        "the old commit's tree went"
    );
    assert!(files.is_installed(IDENTITY, NEXT));
    assert!(
        files.is_installed(other, SHA),
        "another identity's is untouched"
    );
    // Installing again at the same commit replaces its contents.
    files
        .install(IDENTITY, NEXT, &demo_tree("C\n"))
        .expect("installs");
    let read = files
        .read_installed(IDENTITY, NEXT)
        .expect("reads")
        .expect("a tree");
    assert_eq!(read["fragments/a.md"].data, b"C\n");
}

#[test]
fn test_an_identity_that_is_no_remote_bundle_cannot_be_installed() {
    let (files, _) = files();
    for identity in ["demo", "ctxloom+local:demo"] {
        let refused = files
            .install(identity, SHA, &demo_tree("A\n"))
            .expect_err(identity);
        assert!(
            refused.contains("is not a remote bundle reference"),
            "{refused}"
        );
        assert!(!files.is_installed(identity, SHA));
    }
    assert_eq!(files.installed_path("demo", SHA), "");
}

#[test]
fn test_a_bundle_named_by_a_path_installs_below_it() {
    let (files, _) = files();
    let nested = "ctxloom+file:///srv/r.git//bundles/team/rules";
    files
        .install(nested, SHA, &demo_tree("A\n"))
        .expect("installs");
    assert!(files.is_installed(nested, SHA));
    assert!(
        files
            .installed_path(nested, SHA)
            .contains("/srv/r/team/rules/"),
        "{}",
        files.installed_path(nested, SHA)
    );
}

// pull.go pruneUnlockedCheckouts
#[test]
fn test_pruning_removes_what_the_lock_does_not_name_and_says_which_identities_went() {
    let (files, _) = files();
    let other = "ctxloom+git://example.test/o/r//bundles/other";
    let third = "ctxloom+git://example.test/p/q//bundles/third";
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    files
        .install(other, SHA, &demo_tree("O\n"))
        .expect("installs");
    files
        .install(third, SHA, &demo_tree("T\n"))
        .expect("installs");
    let gone = files
        .prune(&[(IDENTITY.to_string(), SHA.to_string())])
        .expect("prunes");
    assert_eq!(gone.len(), 2);
    assert!(gone.contains(&other.to_string()) && gone.contains(&third.to_string()));
    assert!(files.is_installed(IDENTITY, SHA));
    assert!(!files.is_installed(other, SHA) && !files.is_installed(third, SHA));
    // The emptied directories went too: nothing of the other repository stays.
    assert!(!files.exists("/proj/.ctxloom/cache/bundles/example.test/p"));
    assert!(files.exists("/proj/.ctxloom/cache/bundles/example.test/o/r/demo"));
}

#[test]
fn test_pruning_a_commit_of_a_kept_identity_removes_it_without_naming_the_identity() {
    let (files, fs) = files();
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    // A second tree of the same identity, as a half-finished move would leave.
    let beside = format!("/proj/.ctxloom/cache/bundles/example.test/o/r/demo/{NEXT}");
    put(&fs, &format!("{beside}/bundle.yaml"), "v: 2\n");
    put(&fs, &format!("{beside}/{IDENTITY_MARKER}"), IDENTITY);
    let gone = files
        .prune(&[(IDENTITY.to_string(), SHA.to_string())])
        .expect("prunes");
    assert!(gone.is_empty(), "{gone:?}");
    assert!(files.is_installed(IDENTITY, SHA));
    assert!(!files.exists(&beside));
}

#[test]
fn test_pruning_with_nothing_installed_is_nothing() {
    let (files, _) = files();
    assert_eq!(files.prune(&[]), Ok(Vec::new()));
}

// bundle.feature: the authored bundles
#[test]
fn test_an_authored_bundle_is_found_read_and_removed_whole() {
    let (files, fs) = files();
    assert_eq!(files.local_bundle("demo"), Ok(None));
    let root = "/proj/.ctxloom/content/bundles/v2/demo";
    put(&fs, &format!("{root}/bundle.yaml"), "description: d\n");
    put(&fs, &format!("{root}/fragments/example.md"), "# Example\n");
    put(
        &fs,
        &format!("{root}/skills/.s.meta.yaml"),
        "executable: []\n",
    );
    assert_eq!(
        files.local_bundle("demo"),
        Ok(Some(format!("{root}/bundle.yaml")))
    );
    let read = files.read_local_bundle("demo").expect("reads");
    let paths: Vec<&str> = read.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec!["bundle.yaml", "fragments/example.md", "skills/.s.meta.yaml"]
    );
    assert_eq!(read["fragments/example.md"], b"# Example\n");
    files.remove_local_bundle("demo").expect("removes");
    assert_eq!(files.local_bundle("demo"), Ok(None));
    assert!(!files.exists(root));
}

#[test]
fn test_a_bundle_name_cannot_climb_out_of_the_bundles_directory() {
    let (files, _) = files();
    for name in ["", "..", "../x", "a/../b", "/etc", "./x"] {
        assert!(files.local_bundle(name).is_err(), "{name:?}");
        assert!(files.read_local_bundle(name).is_err(), "{name:?}");
        assert!(files.remove_local_bundle(name).is_err(), "{name:?}");
    }
}

#[test]
fn test_a_directory_is_told_from_a_file_and_from_nothing() {
    let (files, fs) = files();
    put(&fs, "/somewhere/file.txt", "x");
    assert!(files.is_directory("/somewhere"));
    assert!(!files.is_directory("/somewhere/file.txt"));
    assert!(!files.is_directory("/nowhere"));
    assert!(files.exists("/somewhere/file.txt") && files.exists("/somewhere"));
    assert!(!files.exists("/nowhere"));
}

fn contents() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        ("bundle.yaml".to_string(), b"description: d\n".to_vec()),
        ("fragments/example.md".to_string(), b"# Example\n".to_vec()),
    ])
}

// bundle.feature: Moving a bundle into another project lands it in that
// project's content tree
#[test]
fn test_a_copy_into_a_project_lands_in_its_content_tree() {
    let (files, fs) = files();
    put(&fs, "/other/.ctxloom/.keep", "");
    let landed = files
        .copy_bundle("/other", "demo", &contents(), false)
        .expect("copies");
    let dest = "/other/.ctxloom/content/bundles/v2/demo";
    assert_eq!(landed, CopyOutcome::Copied(dest.to_string()));
    assert_eq!(
        fs.read(Path::new(&format!("{dest}/fragments/example.md")))
            .expect("a file"),
        b"# Example\n"
    );
}

#[test]
fn test_a_copy_into_an_app_dir_or_a_plain_directory() {
    let (files, fs) = files();
    fs.create_dir_all(Path::new("/x/.ctxloom"), Mode::from_bits(0o755))
        .expect("a directory");
    fs.create_dir_all(Path::new("/plain"), Mode::from_bits(0o755))
        .expect("a directory");
    assert_eq!(
        files.copy_bundle("/x/.ctxloom", "demo", &contents(), false),
        Ok(CopyOutcome::Copied(
            "/x/.ctxloom/content/bundles/v2/demo".to_string()
        ))
    );
    assert_eq!(
        files.copy_bundle("/plain", "demo", &contents(), false),
        Ok(CopyOutcome::Copied("/plain/demo".to_string()))
    );
}

#[test]
fn test_a_copy_onto_a_bundle_is_refused_unless_forced_and_force_replaces_it_whole() {
    let (files, fs) = files();
    fs.create_dir_all(Path::new("/plain"), Mode::from_bits(0o755))
        .expect("a directory");
    files
        .copy_bundle("/plain", "demo", &contents(), false)
        .expect("copies");
    put(&fs, "/plain/demo/stale.md", "stale");
    assert_eq!(
        files.copy_bundle("/plain", "demo", &contents(), false),
        Ok(CopyOutcome::Exists("/plain/demo".to_string()))
    );
    assert!(files.exists("/plain/demo/stale.md"), "refused: untouched");
    assert_eq!(
        files.copy_bundle("/plain", "demo", &contents(), true),
        Ok(CopyOutcome::Copied("/plain/demo".to_string()))
    );
    assert!(
        !files.exists("/plain/demo/stale.md"),
        "the old tree went whole"
    );
}

#[test]
fn test_a_copy_onto_the_bundles_own_directory_is_refused() {
    let (files, fs) = files();
    put(
        &fs,
        "/proj/.ctxloom/content/bundles/v2/demo/bundle.yaml",
        "d: 1\n",
    );
    assert_eq!(
        files.copy_bundle("/proj", "demo", &contents(), true),
        Ok(CopyOutcome::Itself(
            "/proj/.ctxloom/content/bundles/v2/demo".to_string()
        ))
    );
    assert_eq!(
        files.read_local_bundle("demo").expect("reads")["bundle.yaml"],
        b"d: 1\n",
        "even forced, it is not overwritten"
    );
}

#[test]
fn test_a_commit_is_named_by_its_abbreviation_too() {
    let (files, fs) = files();
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    let short = "/proj/.ctxloom/cache/bundles/example.test/o/r/demo/abc1234";
    put(&fs, &format!("{short}/bundle.yaml"), "v: 2\n");
    put(&fs, &format!("{short}/{IDENTITY_MARKER}"), IDENTITY);
    files
        .prune(&[(IDENTITY.to_string(), SHA.to_string())])
        .expect("prunes");
    assert!(!files.exists(short));
    assert!(files.is_installed(IDENTITY, SHA));
}

#[test]
fn test_pruning_everything_leaves_the_cache_root() {
    let (files, _) = files();
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    let gone = files.prune(&[]).expect("prunes");
    assert_eq!(gone, vec![IDENTITY.to_string()]);
    assert!(!files.exists("/proj/.ctxloom/cache/bundles/example.test"));
    assert!(files.exists("/proj/.ctxloom/cache/bundles"));
}

#[test]
fn test_a_bundle_path_is_cleaned_before_it_names_a_directory_and_one_that_climbs_names_none() {
    let (files, _) = files();
    let cleaned = files.installed_path("ctxloom+git://example.test/o/r//bundles/a/../b/./c", SHA);
    assert!(
        cleaned.ends_with(&format!("/example.test/o/r/b/c/{SHA}")),
        "{cleaned}"
    );
    assert_eq!(
        files.installed_path("ctxloom+git://example.test/o/r//bundles/../../x", SHA),
        ""
    );
}

#[test]
fn test_a_copy_onto_the_bundle_itself_is_known_by_another_spelling_of_its_place() {
    let (files, fs) = files();
    put(
        &fs,
        "/proj/.ctxloom/content/bundles/v2/demo/bundle.yaml",
        "d: 1\n",
    );
    fs.create_dir_all(Path::new("/proj/sub"), Mode::from_bits(0o755))
        .expect("a directory");
    assert_eq!(
        files.copy_bundle("/proj/sub/..", "demo", &contents(), false),
        Ok(CopyOutcome::Itself(
            "/proj/sub/../.ctxloom/content/bundles/v2/demo".to_string()
        ))
    );
}

#[test]
fn test_installing_a_commit_replaces_the_trees_of_other_commits_by_any_abbreviation() {
    let (files, fs) = files();
    let base = "/proj/.ctxloom/cache/bundles/example.test/o/r/demo";
    put(&fs, &format!("{base}/abc1234/bundle.yaml"), "v: 0\n");
    put(&fs, &format!("{base}/notes/keep.txt"), "mine\n");
    files
        .install(IDENTITY, SHA, &demo_tree("A\n"))
        .expect("installs");
    assert!(!files.exists(&format!("{base}/abc1234")));
    assert!(
        files.exists(&format!("{base}/notes/keep.txt")),
        "not a commit"
    );
    assert!(files.is_installed(IDENTITY, SHA));
}
