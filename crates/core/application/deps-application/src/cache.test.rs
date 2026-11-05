use std::sync::Mutex;

use super::*;

fn segments(url: &str) -> Option<Vec<String>> {
    repository(url).ok().map(|r| r.cache.segments().to_vec())
}

fn of(parts: &[&str]) -> Option<Vec<String>> {
    Some(parts.iter().map(|s| s.to_string()).collect())
}

// The layout the cache has always had: host and path of an https address,
// the path of a file address, `.git` and trailing slashes gone.
#[test]
fn test_a_repository_is_cached_under_its_host_and_path() {
    let table: &[(&str, &[&str])] = &[
        ("https://github.com/o/r", &["github.com", "o", "r"]),
        ("https://github.com/o/r.git", &["github.com", "o", "r"]),
        ("https://github.com/o/r/", &["github.com", "o", "r"]),
        ("http://example.test/o/r", &["example.test", "o", "r"]),
        ("git@github.com:o/r.git", &["github.com", "o", "r"]),
        ("file:///tmp/a/r/remote.git", &["tmp", "a", "r", "remote"]),
        ("file:///repos/demo", &["repos", "demo"]),
        ("file:///a/../../etc/x", &["a", "etc", "x"]),
    ];
    for (url, expected) in table {
        assert_eq!(segments(url), of(expected), "{url}");
    }
}

#[test]
fn test_a_repository_keeps_the_address_it_is_fetched_from() {
    let r = repository("https://github.com/o/r.git").expect("a repository");
    assert_eq!(r.url, "https://github.com/o/r.git");
}

#[test]
fn test_an_address_that_names_no_repository_has_no_cache_place() {
    for url in [
        "https://",
        "bundles",
        "file:///",
        "",
        "ssh://git@github.com/o/r.git",
    ] {
        let why = repository(url).expect_err(url);
        assert_eq!(
            why,
            format!("refusing to cache {url:?}: it names no repository")
        );
    }
}

/// Remotes that note what they were handed and answer a fixed word.
#[derive(Default)]
struct Spy(Mutex<Vec<String>>);

impl Spy {
    fn note(&self, what: &str, repo: &Repository) {
        self.0.lock().expect("the notes").push(format!(
            "{what} {} {}",
            repo.url,
            repo.cache.segments().join("/")
        ));
    }

    fn seen(&self) -> Vec<String> {
        self.0.lock().expect("the notes").clone()
    }
}

impl Remotes for Spy {
    fn local_repository(&self, path: &str) -> Result<String, Why> {
        Ok(path.to_string())
    }
    fn ensure(&self, repo: &Repository) -> Result<(), Why> {
        self.note("ensure", repo);
        Ok(())
    }
    fn refresh(&self, repo: &Repository) -> Result<(), Why> {
        self.note("refresh", repo);
        Ok(())
    }
    fn tags(&self, repo: &Repository) -> Result<Vec<String>, Why> {
        self.note("tags", repo);
        Ok(vec!["t".to_string()])
    }
    fn resolve_ref(&self, repo: &Repository, name: &str) -> Result<String, Why> {
        self.note(&format!("resolve_ref {name}"), repo);
        Ok("ref".to_string())
    }
    fn resolve_tag(&self, repo: &Repository, name: &str) -> Result<String, Why> {
        self.note(&format!("resolve_tag {name}"), repo);
        Ok("tag".to_string())
    }
    fn default_branch(&self, repo: &Repository) -> Result<String, Why> {
        self.note("default_branch", repo);
        Ok("trunk".to_string())
    }
    fn read_bundle(
        &self,
        repo: &Repository,
        sha: &str,
        bundle: &str,
    ) -> Result<Option<RepoTree>, Why> {
        self.note(&format!("read_bundle {sha} {bundle}"), repo);
        Ok(Some(RepoTree::new()))
    }
    fn list_bundles(&self, repo: &Repository) -> Result<Vec<String>, Why> {
        self.note("list_bundles", repo);
        Ok(vec!["b".to_string()])
    }
    fn has_bundle(&self, repo: &Repository, bundle: &str) -> Result<bool, Why> {
        self.note(&format!("has_bundle {bundle}"), repo);
        Ok(true)
    }
    fn publish(
        &self,
        repo: &Repository,
        files: &BTreeMap<String, Vec<u8>>,
        message: &str,
    ) -> Result<String, Why> {
        self.note(&format!("publish {} {message}", files.len()), repo);
        Ok("commit".to_string())
    }
}

#[test]
fn test_every_call_to_the_remotes_hands_over_the_place_worked_out_from_the_address() {
    let spy = Spy::default();
    let remotes = ByAddress(&spy);
    let url = "https://github.com/o/r.git";
    let files = BTreeMap::from([("f".to_string(), vec![1u8])]);
    assert_eq!(remotes.ensure(url), Ok(()));
    assert_eq!(remotes.refresh(url), Ok(()));
    assert_eq!(remotes.tags(url), Ok(vec!["t".to_string()]));
    assert_eq!(remotes.resolve_ref(url, "n"), Ok("ref".to_string()));
    assert_eq!(remotes.resolve_tag(url, "n"), Ok("tag".to_string()));
    assert_eq!(remotes.default_branch(url), Ok("trunk".to_string()));
    assert_eq!(
        remotes.read_bundle(url, "sha", "b"),
        Ok(Some(RepoTree::new()))
    );
    assert_eq!(remotes.list_bundles(url), Ok(vec!["b".to_string()]));
    assert_eq!(remotes.has_bundle(url, "b"), Ok(true));
    assert_eq!(remotes.publish(url, &files, "m"), Ok("commit".to_string()));
    let place = format!("{url} github.com/o/r");
    let expected: Vec<String> = [
        "ensure",
        "refresh",
        "tags",
        "resolve_ref n",
        "resolve_tag n",
        "default_branch",
        "read_bundle sha b",
        "list_bundles",
        "has_bundle b",
        "publish 1 m",
    ]
    .iter()
    .map(|what| format!("{what} {place}"))
    .collect();
    assert_eq!(spy.seen(), expected);
}

#[test]
fn test_an_address_that_names_no_repository_never_reaches_the_remotes() {
    let spy = Spy::default();
    let remotes = ByAddress(&spy);
    let refused = "refusing to cache \"bundles\": it names no repository".to_string();
    assert_eq!(remotes.ensure("bundles"), Err(refused.clone()));
    assert_eq!(remotes.refresh("bundles"), Err(refused.clone()));
    assert_eq!(remotes.tags("bundles"), Err(refused.clone()));
    assert_eq!(remotes.resolve_ref("bundles", "n"), Err(refused.clone()));
    assert_eq!(remotes.resolve_tag("bundles", "n"), Err(refused.clone()));
    assert_eq!(remotes.default_branch("bundles"), Err(refused.clone()));
    assert_eq!(
        remotes.read_bundle("bundles", "s", "b"),
        Err(refused.clone())
    );
    assert_eq!(remotes.list_bundles("bundles"), Err(refused.clone()));
    assert_eq!(remotes.has_bundle("bundles", "b"), Err(refused.clone()));
    assert_eq!(
        remotes.publish("bundles", &BTreeMap::new(), "m"),
        Err(refused)
    );
    assert!(spy.seen().is_empty());
}

#[test]
fn test_a_bundle_slot_is_its_repository_s_place_then_the_bundle_s_path() {
    let remote = RemoteRef::parse("ctxloom+git://example.test/o/r//bundles/a/../b/./c")
        .expect("a remote reference");
    let slot = slot(&remote).expect("a slot");
    assert_eq!(slot.identity, remote.identity);
    assert_eq!(slot.repository.segments(), ["example.test", "o", "r"]);
    assert_eq!(slot.bundle.segments(), ["b", "c"]);
}

#[test]
fn test_a_slot_of_an_identity_that_is_no_remote_bundle_is_refused() {
    for identity in ["demo", "ctxloom+local:demo", ""] {
        let why = slot_of(identity).expect_err(identity);
        assert_eq!(why, format!("{identity} is not a remote bundle reference"));
    }
    assert!(slot_of("ctxloom+git://example.test/o/r//bundles/demo").is_ok());
}

#[test]
fn test_a_bundle_path_that_climbs_names_no_slot() {
    assert!(slot_of("ctxloom+git://example.test/o/r//bundles/../../x").is_err());
}
