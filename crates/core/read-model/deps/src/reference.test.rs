use super::*;

fn parsed(raw: &str) -> RemoteRef {
    RemoteRef::parse(raw).unwrap_or_else(|| panic!("{raw} is a remote reference"))
}

// reference.go: a canonical reference to a file repository.
#[test]
fn test_a_canonical_reference_to_a_file_repository_is_taken_apart() {
    let r = parsed("ctxloom+file:///tmp/x/remote.git//bundles/team/demo@main#profiles/reach");
    assert_eq!(
        r.identity,
        "ctxloom+file:///tmp/x/remote.git//bundles/team/demo"
    );
    assert_eq!(r.url, "file:///tmp/x/remote.git");
    assert_eq!(r.bundle, "team/demo");
    assert_eq!(r.constraint, "main");
    assert_eq!(r.item, "profiles/reach");
    assert_eq!(
        r.bundle_ref(),
        "ctxloom+file:///tmp/x/remote.git//bundles/team/demo@main"
    );
}

#[test]
fn test_a_canonical_reference_to_a_git_repository_is_fetched_over_https() {
    let r = parsed("ctxloom+git://github.com/o/r//bundles/demo");
    assert_eq!(r.url, "https://github.com/o/r");
    assert_eq!(r.identity, "ctxloom+git://github.com/o/r//bundles/demo");
    assert_eq!(r.constraint, "");
    assert_eq!(r.item, "");
    assert_eq!(r.bundle_ref(), "ctxloom+git://github.com/o/r//bundles/demo");
}

// A fetch address keeps the repository path as written, ".git" included: the
// lock key of `file:///x/remote.git@bundles/demo` names the directory that is
// there (Go's `profile show` of the same spelling).
#[test]
fn test_a_fetch_address_keeps_its_repository_path_as_written() {
    for (raw, identity, url, bundle, constraint) in [
        (
            "file:///srv/repo.git@bundles/x",
            "ctxloom+file:///srv/repo.git//bundles/x",
            "file:///srv/repo.git",
            "x",
            "",
        ),
        (
            "file:///srv/repo.git/@bundles/x@v1",
            "ctxloom+file:///srv/repo.git//bundles/x",
            "file:///srv/repo.git",
            "x",
            "v1",
        ),
        (
            "https://Github.com/o/r.git@bundles/x/y@abc1234",
            "ctxloom+git://github.com/o/r.git//bundles/x/y",
            "https://github.com/o/r.git",
            "x/y",
            "abc1234",
        ),
        (
            "git@github.com:o/r@bundles/x",
            "ctxloom+git://github.com/o/r//bundles/x",
            "https://github.com/o/r",
            "x",
            "",
        ),
    ] {
        let r = parsed(raw);
        assert_eq!(r.identity, identity, "{raw}");
        assert_eq!(r.url, url, "{raw}");
        assert_eq!(r.bundle, bundle, "{raw}");
        assert_eq!(r.constraint, constraint, "{raw}");
    }
}

#[test]
fn test_a_reference_that_is_not_remote_is_none() {
    for raw in [
        "",
        "demo",
        "team/rules",
        "ctxloom+local:demo",
        "ctxloom+companion:ltk",
        "ctxloom:local@bundles/demo",
        "ctxloom+nope://x//bundles/y",
        "https://github.com/o/r",
        "file:///srv/repo.git@fragments/x",
    ] {
        assert_eq!(RemoteRef::parse(raw), None, "{raw}");
    }
}

#[test]
fn test_two_spellings_of_one_bundle_share_an_identity() {
    let a = parsed("ctxloom+file:///srv/r.git//bundles/x@v1");
    let b = parsed("file:///srv/r.git@bundles/x@v2");
    assert_eq!(a.identity, b.identity);
    assert_ne!(a.constraint, b.constraint);
}
