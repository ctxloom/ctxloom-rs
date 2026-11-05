//! The fakes are the oracle the use cases are held to: what they answer is
//! held here.

use ctxloom_ports::ContentRepo;
use deps_ports::{BundleSlot, CachePath, DepsFiles, Remotes, Repository};

use super::*;

const URL: &str = "file:///srv/origin.git";

fn repo(url: &str) -> Repository {
    Repository {
        url: url.to_string(),
        cache: CachePath::new(vec!["x".to_string()]).expect("a plain name"),
    }
}

#[test]
fn test_a_repository_is_read_only_once_cloned() {
    let remotes = FakeRemotes::default();
    remotes.serve(URL);
    let sha = remotes.commit(URL, &demo("1.0.0", "one"));
    remotes.tag(URL, "v1.0.0", &sha);
    assert!(remotes.tags(&repo(URL)).is_err());
    remotes.ensure(&repo(URL)).expect("clones");
    assert_eq!(remotes.tags(&repo(URL)), Ok(vec!["v1.0.0".to_string()]));
    assert!(remotes.tags(&repo("file:///srv/never.git")).is_err());
}

#[test]
fn test_a_branch_resolves_to_its_commit() {
    let remotes = FakeRemotes::default();
    remotes.serve(URL);
    let first = remotes.commit(URL, &demo("1.0.0", "one"));
    remotes.commit(URL, &demo("2.0.0", "two"));
    remotes.branch(URL, "topic", &first);
    remotes.ensure(&repo(URL)).expect("clones");
    remotes.refresh(&repo(URL)).expect("fetches");
    assert_eq!(remotes.resolve_ref(&repo(URL), "topic"), Ok(first));
}

fn slot(identity: &str) -> BundleSlot {
    let path = || CachePath::new(vec!["x".to_string()]).expect("a plain name");
    BundleSlot {
        identity: identity.to_string(),
        repository: path(),
        bundle: path(),
    }
}

#[test]
fn test_the_files_say_where_they_are() {
    let files = FakeFiles::default();
    assert_eq!(files.registry_path(), "/p/.ctxloom/remotes.yaml");
    assert_eq!(files.lock_path(), "/p/.ctxloom/lock.yaml");
}

#[test]
fn test_a_prune_names_only_identities_no_tree_of_which_is_kept() {
    let files = FakeFiles::default();
    let tree = RepoTree::new();
    files.install(&slot("kept"), "a", &tree).expect("installs");
    files.install(&slot("gone"), "b", &tree).expect("installs");
    // `kept` is locked at a commit other than the installed one.
    let gone = files
        .prune(&[("kept".to_string(), "other".to_string())])
        .expect("prunes");
    assert_eq!(gone, vec!["gone".to_string()]);
    assert!(files.installed_identities().is_empty());
}

#[test]
fn test_a_place_exists_as_a_directory_or_as_a_placed_bundle() {
    let files = FakeFiles::default();
    assert!(!files.exists("/d"));
    files.directory("/d");
    assert!(files.exists("/d"));
    assert!(!files.exists("/d/x"));
    let files = FakeFiles::default();
    files
        .copy_bundle("/e", "b", &BTreeMap::new(), false)
        .expect("copies");
    assert!(files.exists("/e/.ctxloom/content/bundles/v2/b"));
    assert!(!files.exists("/e"));
}

#[test]
fn test_the_content_has_a_root_only_with_bundles() {
    let content = FakeContent::default();
    assert!(!content.has_root().expect("answers"));
    content.bundle("project", &[ENVELOPE]);
    assert!(content.has_root().expect("answers"));
    assert_eq!(
        content.manifest_path("demo"),
        "/p/.ctxloom/content/bundles/v2/demo/bundle.yaml"
    );
    assert!(content
        .executable_files("demo")
        .expect("answers")
        .is_empty());
}
