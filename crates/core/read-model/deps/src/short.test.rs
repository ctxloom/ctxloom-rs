use super::*;

fn remotes(name: &str) -> Option<String> {
    match name {
        "origin" => Some("file:///srv/origin.git".to_string()),
        "gh" => Some("https://github.com/o/r".to_string()),
        _ => None,
    }
}

fn none_local(_: &str) -> bool {
    false
}

// shortname.go CanonicalizeShortRef: "<alias>/<bundle-path>[#<sel>/<item>]"
#[test]
fn test_a_short_reference_expands_to_the_remote_s_canonical_reference() {
    for (short, canonical) in [
        (
            "origin/demo",
            "ctxloom+file:///srv/origin.git//bundles/demo",
        ),
        (
            "origin/team/demo",
            "ctxloom+file:///srv/origin.git//bundles/team/demo",
        ),
        (
            "gh/demo#profiles/dev",
            "ctxloom+git://github.com/o/r//bundles/demo#profiles/dev",
        ),
        (
            "origin/demo@main",
            "ctxloom+file:///srv/origin.git//bundles/demo@main",
        ),
        (
            "origin/demo@v1.2.0#fragments/a",
            "ctxloom+file:///srv/origin.git//bundles/demo@v1.2.0#fragments/a",
        ),
    ] {
        assert_eq!(expand(short, &remotes, &none_local), canonical, "{short}");
    }
}

#[test]
fn test_what_is_not_a_short_reference_is_left_as_it_is() {
    for raw in [
        "demo",
        "demo#profiles/dev",
        "nowhere/demo",
        "origin",
        "origin/",
        "/demo",
        "ctxloom+local:demo",
        "ctxloom+file:///srv/origin.git//bundles/demo",
    ] {
        assert_eq!(expand(raw, &remotes, &none_local), raw, "{raw}");
    }
}

// A fetch address needs no registry: it is returned in the canonical
// spelling (its repository path as written).
#[test]
fn test_a_self_contained_address_is_respelled_without_the_registry() {
    assert_eq!(
        expand(
            "file:///srv/origin.git@bundles/demo",
            &|_| None,
            &none_local
        ),
        "ctxloom+file:///srv/origin.git//bundles/demo"
    );
    assert_eq!(
        expand(
            "file:///srv/origin.git@bundles/demo#profiles/dev",
            &|_| None,
            &none_local
        ),
        "ctxloom+file:///srv/origin.git//bundles/demo#profiles/dev"
    );
}

// Decision E: a project bundle spelled like a remote's wins.
#[test]
fn test_a_local_bundle_of_the_same_spelling_wins_over_a_remote() {
    let local = |base: &str| base == "origin/demo";
    assert_eq!(expand("origin/demo", &remotes, &local), "origin/demo");
    assert_eq!(
        expand("origin/other", &remotes, &local),
        "ctxloom+file:///srv/origin.git//bundles/other"
    );
}

// reference.go ResolveRefString: a sibling named from a profile read out of a
// repository is that repository's, at the commit the profile was read at.
#[test]
fn test_a_sibling_reference_in_a_remote_profile_is_pinned_to_its_own_commit() {
    let within =
        |reference: &str, sha: &str| within_repository(reference, "file:///srv/r.git", sha);
    for (reference, sha, want) in [
        (
            "demo",
            "abc1234",
            "ctxloom+file:///srv/r.git//bundles/demo@abc1234",
        ),
        (
            "lang/go#fragments/x",
            "abc1234",
            "ctxloom+file:///srv/r.git//bundles/lang/go@abc1234#fragments/x",
        ),
        (
            "demo@v1",
            "abc1234",
            "ctxloom+file:///srv/r.git//bundles/demo@v1",
        ),
        ("demo", "", "ctxloom+file:///srv/r.git//bundles/demo"),
    ] {
        assert_eq!(within(reference, sha), want, "{reference}");
    }
}

#[test]
fn test_a_reference_that_names_its_own_source_is_left_alone() {
    let within = |reference: &str| within_repository(reference, "file:///srv/r.git", "abc1234");
    for reference in [
        "ctxloom+git://h/o/r//bundles/x",
        "ctxloom+file:///elsewhere.git//bundles/x@v1",
        "file:///elsewhere.git@bundles/x",
        "ctxloom+local:x",
        "ctxloom:local@bundles/x",
    ] {
        assert_eq!(within(reference), reference, "{reference}");
    }
    assert_eq!(
        within_repository("demo", "", "abc1234"),
        "demo",
        "no source, no meaning"
    );
    assert_eq!(within_repository("demo", "bundles", "abc1234"), "demo");
}

// A reference that is already self-contained is never read as an alias, even
// where every alias would resolve.
#[test]
fn test_a_self_contained_reference_is_not_taken_for_an_alias() {
    let any = |_: &str| Some("https://github.com/x/y".to_string());
    for reference in [
        "https://h/o/r",
        "file:///srv/r.git",
        "ctxloom:thing/here",
        "ctxloom+local:origin/demo",
    ] {
        assert_eq!(
            expand(reference, &any, &none_local),
            reference,
            "{reference}"
        );
    }
}

#[test]
fn test_an_empty_alias_or_bundle_is_not_an_alias_reference() {
    let any = |_: &str| Some("https://github.com/x/y".to_string());
    for reference in ["/demo", "origin/"] {
        assert_eq!(
            expand(reference, &any, &none_local),
            reference,
            "{reference}"
        );
    }
}
