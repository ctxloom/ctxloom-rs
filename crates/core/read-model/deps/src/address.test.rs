use super::*;

// remotes: `remote create`'s URL formats (cli/remote.go): shorthand, full
// URLs, ssh converted to https, file URLs.
#[test]
fn test_a_remote_is_stored_under_the_address_its_url_formats_stand_for() {
    for (raw, stored) in [
        ("alice/ctxloom", "https://github.com/alice/ctxloom"),
        (
            "https://github.com/alice/ctxloom",
            "https://github.com/alice/ctxloom",
        ),
        (
            "https://git.example.com/corp/ctxloom",
            "https://git.example.com/corp/ctxloom",
        ),
        (
            "git@github.com:alice/ctxloom.git",
            "https://github.com/alice/ctxloom.git",
        ),
        ("file:///srv/bundles.git", "file:///srv/bundles.git"),
        ("  https://h/o/r  ", "https://h/o/r"),
    ] {
        assert_eq!(stored_url(raw).as_deref(), Ok(stored), "{raw}");
    }
}

// remote.feature / remotes.go requireRepository: an address that names no
// repository is refused where it is written.
#[test]
fn test_an_address_that_names_no_repository_is_refused() {
    for raw in ["", "bundles", "f", "~/bundles.git", "https://", "git@host"] {
        let refused = stored_url(raw).expect_err(raw);
        assert!(
            matches!(&refused, DepsError::NoRepository { url, .. } if url == raw.trim()),
            "{raw}: {refused:?}"
        );
    }
}

// refuri.CanonicalRepoURL: one spelling per repository.
#[test]
fn test_two_spellings_of_one_repository_share_a_key() {
    let spellings = [
        "https://github.com/o/r",
        "https://GitHub.com/o/r/",
        "https://github.com/o/r.git",
        "git@github.com:o/r.git",
        "o/r",
    ];
    let keys: Vec<_> = spellings
        .iter()
        .map(|s| repository_key(&stored_url(s).expect(s)))
        .collect();
    assert!(
        keys.iter().all(|k| k.as_deref() == Some("github.com/o/r")),
        "{keys:?}"
    );
    assert_eq!(
        repository_key("file:///srv/r.git").as_deref(),
        Some("file:///srv/r")
    );
    assert!(same_repository("https://h/o/r", "git@h:o/r.git"));
    assert!(!same_repository("https://h/o/r", "https://h/o/s"));
    assert!(!same_repository("bundles", "bundles"));
    assert!(!same_repository("https://h/o/r", "file:///h/o/r"));
}

// RepoCache.RepoDirForURL: every spelling of one repository caches in one
// directory, and a degenerate address caches nowhere.
#[test]
fn test_a_clone_is_cached_under_the_repository_s_host_and_path() {
    let segments = |url: &str| cache_segments(url);
    assert_eq!(
        segments("https://github.com/o/r.git"),
        Some(vec![
            "github.com".to_string(),
            "o".to_string(),
            "r".to_string()
        ])
    );
    assert_eq!(
        segments("file:///tmp/a/r/remote.git"),
        Some(["tmp", "a", "r", "remote"].map(String::from).to_vec())
    );
    assert_eq!(
        segments("git@github.com:o/r.git"),
        segments("https://github.com/o/r")
    );
    assert_eq!(segments("https://"), None);
    assert_eq!(segments("bundles"), None);
    // The cache root itself is never an answer, and nothing climbs out of it.
    assert_eq!(segments("file:///"), None);
    assert_eq!(
        segments("file:///a/../../etc/x"),
        Some(["a", "etc", "x"].map(String::from).to_vec())
    );
}

// Reference.CanonicalString: the lock key is the version-less canonical
// reference; the repository path is kept as written, ".git" included (Go's
// `profile show` of `https://h/o/r.git@bundles/x`).
#[test]
fn test_the_canonical_reference_to_a_bundle_of_a_repository() {
    for (url, bundle, want) in [
        (
            "file:///tmp/x/remote.git",
            "demo",
            "ctxloom+file:///tmp/x/remote.git//bundles/demo",
        ),
        (
            "https://github.com/o/r",
            "demo",
            "ctxloom+git://github.com/o/r//bundles/demo",
        ),
        (
            "https://github.com/o/r.git",
            "a/b",
            "ctxloom+git://github.com/o/r.git//bundles/a/b",
        ),
        (
            "https://Example.com/o/r.git",
            "x",
            "ctxloom+git://example.com/o/r.git//bundles/x",
        ),
    ] {
        assert_eq!(canonical_ref(url, bundle).as_deref(), Some(want), "{url}");
    }
    assert_eq!(canonical_ref("bundles", "x"), None);
    assert_eq!(canonical_ref("file:///x", ""), None);
}

#[test]
fn test_a_local_path_is_told_from_an_address() {
    for yes in ["/srv/bundles.git", "./bundles.git", "../bundles.git"] {
        assert!(is_local_path(yes), "{yes}");
    }
    for no in [
        "bundles",
        "alice/ctxloom",
        "file:///x",
        "https://h/o/r",
        "~/x",
        "",
    ] {
        assert!(!is_local_path(no), "{no}");
    }
}

fn detail(raw: &str) -> String {
    match stored_url(raw).expect_err(raw) {
        DepsError::NoRepository { detail, .. } => detail,
        other => panic!("{raw}: {other:?}"),
    }
}

// An address refused says which way it went wrong.
#[test]
fn test_a_refused_address_says_which_way_it_went_wrong() {
    for raw in ["", "bundles"] {
        assert!(detail(raw).contains("bare word"), "{raw}");
    }
    for raw in [
        "ftp://h/o/r",
        "a:b",
        "git@h:",
        "u@:o/r",
        "a/b@h:o/r",
        "owner/",
        "git@host://x",
    ] {
        assert!(
            detail(raw).contains("https, http or file URL"),
            "{raw}: {}",
            detail(raw)
        );
    }
    assert!(detail("~/x").contains("absolute path"));
    assert!(detail("/abs/path").contains("resolved to the repository"));
    assert!(detail("https://").contains("no host and path"));
}

#[test]
fn test_a_path_on_this_machine_is_recognised() {
    for raw in ["/a", "./a", "../a", ".", ".."] {
        assert!(is_local_path(raw), "{raw}");
    }
    for raw in ["a", "a/b", "...", ".a", "~"] {
        assert!(!is_local_path(raw), "{raw}");
    }
}

// Go's ParseOwnerRepo: a forge API's path segments.
#[test]
fn test_an_http_address_names_an_owner_and_a_repository() {
    for (url, want) in [
        ("https://github.com/acme/hub", ("acme", "hub")),
        ("https://github.com/acme/hub.git", ("acme", "hub")),
        ("https://github.com/acme/hub/", ("acme", "hub")),
        ("http://ghe.corp.test:8080/acme/hub.git", ("acme", "hub")),
    ] {
        assert_eq!(
            owner_repo(url),
            Some((want.0.to_string(), want.1.to_string())),
            "{url}"
        );
    }
}

#[test]
fn test_an_address_that_is_not_an_owner_and_a_repository_names_none() {
    for url in [
        "https://github.com/acme",
        "https://github.com/acme/hub/extra",
        "https://github.com/",
        "https://github.com",
        "file:///srv/acme/hub.git",
        "ssh://git@github.com/acme/hub",
        "acme/hub",
        "",
    ] {
        assert_eq!(owner_repo(url), None, "{url}");
    }
}
