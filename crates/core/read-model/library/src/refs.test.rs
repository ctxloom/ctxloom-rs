//! Ported from Go's `internal/shared/refuri/refuri_test.go` and
//! `internal/core/ident` (the selector), at 860336c4e.

use super::*;

angzarr_presentation::english_for!(&[crate::EN_US.1]);

// refuri_test.go: TestHasScheme_KnowsEveryClass
#[test]
fn test_has_scheme_knows_every_class() {
    assert_eq!(
        SourceClass::ALL,
        [
            SourceClass::Git,
            SourceClass::File,
            SourceClass::Local,
            SourceClass::Companion
        ]
    );
    for class in SourceClass::ALL {
        let raw = match class {
            SourceClass::Git => "ctxloom+git://host/repo//bundles/name".to_string(),
            SourceClass::File => "ctxloom+file:///repo//bundles/name".to_string(),
            other => format!("{SCHEME_PREFIX}{}:name", other.name()),
        };
        assert!(has_scheme(&raw), "{raw}");
        let parts = Parts::parse(&raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
        assert_eq!(parts.class, class, "{raw}");
    }
}

// refuri_test.go: TestHasScheme_RejectsNonMembers
#[test]
fn test_has_scheme_rejects_non_members() {
    for raw in [
        "",
        "dev",
        "lang/go",
        "ctxloom:local@bundles/dev",
        "ctxloom:companion@ltk",
        "https://github.com/acme/repo@bundles/dev",
        "git@github.com:acme/repo@bundles/dev",
        "file:///srv/repo@bundles/dev",
    ] {
        assert!(!has_scheme(raw), "{raw:?}");
    }
}

// refuri_test.go: TestParse_RoundTripsEveryClass
#[test]
fn test_parse_round_trips_every_class() {
    for raw in [
        "ctxloom+git://github.com/acme/repo//bundles/lang/go",
        "ctxloom+git://github.com/acme/repo//bundles/tooling@v1.2.3",
        "ctxloom+git://github.com/acme/repo//bundles/tooling#fragments/x",
        "ctxloom+file:///srv/repo//bundles/tooling",
        "ctxloom+local:my-tools#profiles/dev",
        "ctxloom+companion:ltk@v2",
    ] {
        let parts = Parts::parse(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
        assert_eq!(parts.render(true), raw, "render must invert parse");
        let again = Parts::parse(&parts.render(true)).unwrap();
        assert_eq!(parts, again, "parse, render, parse is stable");
    }
}

// refuri_test.go: TestHasScheme_AdmitsUnknownClassesSoParseCanRefuseThem
#[test]
fn test_has_scheme_admits_unknown_classes_so_parse_can_refuse_them() {
    for raw in [
        "ctxloom+registry:dev",
        "ctxloom+localish:dev",
        "ctxloom+local",
    ] {
        assert!(has_scheme(raw), "{raw} claims the family");
        assert!(
            matches!(Parts::parse(raw), Err(RefError::Syntax { .. })),
            "{raw} names no known class and must be refused"
        );
    }
}

// refuri_test.go: TestBuilders_RenderCanonicalStrings
#[test]
fn test_builders_render_canonical_strings() {
    let cases = [
        (
            Parts::git("github.com", "/acme/repo", "tooling"),
            "ctxloom+git://github.com/acme/repo//bundles/tooling",
        ),
        (
            Parts::file("/srv/repo", "lang/go"),
            "ctxloom+file:///srv/repo//bundles/lang/go",
        ),
        (Parts::local("my-tools"), "ctxloom+local:my-tools"),
        (Parts::companion("ltk"), "ctxloom+companion:ltk"),
    ];
    for (minted, want) in cases {
        let parts = minted.unwrap_or_else(|e| panic!("{want}: {e}"));
        assert_eq!(parts.render(true), want);
    }
}

// refuri_test.go: TestMint_RefusesWhatParseRefuses
#[test]
fn test_mint_refuses_what_parse_refuses() {
    assert!(
        Parts::local("").is_err(),
        "an empty bundle name must be refused"
    );
    let no_host = Parts {
        class: SourceClass::Git,
        host: String::new(),
        repo_path: "/acme/repo".to_string(),
        bundle: "x".to_string(),
        version: String::new(),
        fragment: String::new(),
    };
    assert!(
        no_host.mint().is_err(),
        "ctxloom+git without a host must be refused"
    );
    // A name carrying the version delimiter survives as data rather than
    // silently splitting into name + version.
    let at = Parts::local("na@me").expect("a literal @ in a bundle name must round-trip");
    assert_eq!(at.bundle, "na@me");
    assert_eq!(at.version, "");
}

/// Every construct a version may be: Masterminds semver's constraints and
/// ctxloom's `kind:` prefixes.
const CONSTRAINT_VERSIONS: [&str; 24] = [
    "1.2.3",
    "=1.2.3",
    "!=1.2.3",
    ">1.2",
    "<2",
    ">=1.2",
    "=>1.2",
    "<=2",
    "=<2",
    "~1.2",
    "~>1.2",
    "^1.2",
    "1.x",
    "1.X",
    "1.*",
    "*",
    ">=1.2, <2",
    ">=1.2 <2",
    "^1 || ^2",
    "1.2 - 1.4",
    "1.2.3-rc.1+build.5",
    "version:^1.2",
    "tag:v1.0",
    "branch:feature/x",
];

// refuri_test.go: TestParse_VersionIsDecodedAndRoundTrips
#[test]
fn test_parse_version_is_decoded_and_round_trips() {
    let bases = [
        "ctxloom+git://github.com/o/r//bundles/x",
        "ctxloom+file:///srv/has space/repo//bundles/x",
        "ctxloom+local:x",
    ];
    for base in bases {
        for version in CONSTRAINT_VERSIONS.into_iter().chain(["sha:deadbeef"]) {
            let typed = format!("{base}@{version}");
            let parts = Parts::parse(&typed).unwrap_or_else(|e| panic!("{typed}: {e}"));
            assert_eq!(parts.version, version, "typed {typed}");

            let escaped = format!("{base}@{}", escape_path(version));
            let from_escaped = Parts::parse(&escaped).unwrap_or_else(|e| panic!("{escaped}: {e}"));
            assert_eq!(parts, from_escaped, "{escaped} must parse like {typed}");

            let back = Parts::parse(&parts.render(true)).unwrap();
            assert_eq!(parts, back, "parse of render of {typed}");
        }
        // An "@" inside a version can only arrive escaped; decoded, it must
        // be re-escaped on render or the next parse splits at the wrong "@".
        let with_at = Parts::parse(&format!("{base}@branch:a%40b")).unwrap();
        assert_eq!(with_at.version, "branch:a@b");
        assert_eq!(with_at, Parts::parse(&with_at.render(true)).unwrap());
    }
}

// refuri_test.go: TestParse_DotSegmentsInAVersionNeverReachTheBundle
#[test]
fn test_dot_segments_in_a_version_never_reach_the_bundle() {
    for version in ["a/../b", "a/./b", "../b", "a/.."] {
        let raw = format!("ctxloom+git://github.com/o/r//bundles/x@{version}");
        let parts = Parts::parse(&raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
        assert_eq!(
            parts.bundle, "x",
            "the version {version:?} moved the bundle name"
        );
        assert_eq!(parts.version, version, "{raw}");
        assert_eq!(parts, Parts::parse(&parts.render(true)).unwrap());
    }
}

// refgrammar: a canonical reference that is not well formed is refused, and
// the refusal says why (Go's `malformed bundle reference: ...`).
#[test]
fn test_a_malformed_reference_is_refused_with_the_reason() {
    for (raw, why) in [
        ("", "empty"),
        ("ctxloom+git://host/repo/bundles/x", "missing \"//\""),
        ("ctxloom+git:///acme/repo//bundles/x", "requires a host"),
        ("ctxloom+file://host/repo//bundles/x", "takes no host"),
        ("ctxloom+git://h/acme/repo//bundles/x%2Fy", "encoded slash"),
        ("ctxloom+git://h/acme/repo//bundles/x?q=1", "query string"),
        (
            "ctxloom+git://h/acme/repo//tools/x",
            "must begin \"bundles/\"",
        ),
    ] {
        let err = Parts::parse(raw).expect_err(raw);
        let said = err.english();
        assert!(said.contains("malformed bundle reference"), "{raw}: {said}");
        assert!(said.contains(why), "{raw}: {said}");
    }
}

// ident/itemref.go ParseSelector: `<kind>/<name>`; commands may be spelled
// `commands` or `prompts`.
#[test]
fn test_a_selector_is_a_kind_and_a_name() {
    assert_eq!(
        parse_selector("fragments/testing").unwrap(),
        (ItemKind::Fragment, "testing".to_string())
    );
    assert_eq!(
        parse_selector("commands/review").unwrap(),
        (ItemKind::Prompt, "review".to_string())
    );
    assert_eq!(
        parse_selector("prompts/review").unwrap(),
        (ItemKind::Prompt, "review".to_string())
    );
    assert_eq!(
        parse_selector("mcp/tree-sitter").unwrap(),
        (ItemKind::Mcp, "tree-sitter".to_string())
    );
    assert_eq!(
        parse_selector("hooks/pre_tool/guard").unwrap(),
        (ItemKind::Hook, "pre_tool/guard".to_string())
    );
    assert_eq!(
        parse_selector("skills/reviewer").unwrap(),
        (ItemKind::Skill, "reviewer".to_string())
    );
}

// ident/itemref.go ParseSelector: the refusals, in Go's words.
#[test]
fn test_a_bad_selector_is_refused_in_go_s_words() {
    let no_name = parse_selector("fragments").expect_err("no name");
    assert_eq!(
        no_name.english(),
        "selector \"fragments\" must be <kind>/<name>"
    );
    let empty_name = parse_selector("fragments/").expect_err("empty name");
    assert!(matches!(empty_name, RefError::Selector { .. }));
    let unknown = parse_selector("widgets/testing").expect_err("unknown kind");
    assert_eq!(
        unknown.english(),
        "unknown item kind \"widgets\" (want fragments|commands|mcp|hooks|skills)"
    );
}

// ItemKind: the directory is the canonical item reference's segment, and a
// command's is `prompts` even though its selector says `commands`.
#[test]
fn test_a_commands_directory_is_prompts() {
    assert_eq!(ItemKind::Prompt.dir(), "prompts");
    assert_eq!(ItemKind::Fragment.dir(), "fragments");
    assert_eq!(
        Parts::parse("ctxloom+local:alpha#prompts/review")
            .unwrap()
            .fragment,
        "prompts/review"
    );
}

fn host_of(authority: &str) -> Result<String, String> {
    Parts::parse(&format!("ctxloom+git://{authority}/r//bundles/x"))
        .map(|p| p.host)
        .map_err(|e| e.english())
}

// refuri.go canonicalGitHost (url.Parse, Hostname, Port, CanonicalAuthority):
// the host folds to lower case, the https port drops, any other port stays, an
// IPv6 literal is bracketed.
#[test]
fn test_a_git_host_is_folded_and_a_default_port_dropped() {
    for (authority, want) in [
        ("HOST", "host"),
        ("Host.COM:443", "host.com"),
        ("host.com:8080", "host.com:8080"),
        ("[::1]:8080", "[::1]:8080"),
        ("[::1]", "[::1]"),
        ("[2001:DB8::1]", "[2001:db8::1]"),
        ("[2001:DB8::1]:443", "[2001:db8::1]"),
    ] {
        assert_eq!(host_of(authority), Ok(want.to_string()), "{authority}");
    }
}

// refuri.go canonicalGitHost: Go's url.Parse takes an empty port for none,
// refuses a port that is not digits, and a host that is nothing.
#[test]
fn test_a_git_authority_with_an_odd_port_or_no_host_is_judged_as_go_judges_it() {
    assert_eq!(host_of("host:"), Ok("host".to_string()));
    assert_eq!(host_of("[::1]:"), Ok("[::1]".to_string()));
    let bad_port = host_of("host:abc").expect_err("a port of letters");
    assert!(bad_port.contains("invalid port"), "{bad_port}");
    for no_host in [":80", "[]:80", "[]"] {
        let e = host_of(no_host).expect_err(no_host);
        assert!(e.contains("requires a host"), "{no_host}: {e}");
    }
}

// refuri.go Parse: a control character is refused where it stands
#[test]
fn test_a_control_character_is_refused_where_it_stands() {
    for raw in ["ctxloom+local:a\u{7f}b", "ctxloom+local:a\tb"] {
        let e = Parts::parse(raw).expect_err("control");
        assert_eq!(
            e.english(),
            "malformed bundle reference: control character at byte 15",
            "{raw:?}"
        );
    }
}

// refuri.go Parse: the repository of a file reference, with and without the
// authority slashes
#[test]
fn test_a_file_reference_needs_a_repository_and_a_git_reference_a_host() {
    assert_eq!(
        Parts::parse("ctxloom+file:////bundles/x")
            .expect_err("no repo")
            .english(),
        "malformed bundle reference: empty repository path"
    );
    let parts = Parts::parse("ctxloom+file:/repo//bundles/x").expect("path form");
    assert_eq!(
        (parts.bundle.as_str(), parts.repo_path.as_str()),
        ("x", "/repo")
    );
    assert_eq!(
        Parts::parse("ctxloom+git:/repo//bundles/x")
            .expect_err("no host")
            .english(),
        "malformed bundle reference: ctxloom+git requires a host"
    );
}

// refuri.go: escapes of unreserved characters are decoded, the rest keep their
// escape with the hex in capitals; a bad or cut escape is refused
#[test]
fn test_percent_escapes_are_normalised_or_refused() {
    assert_eq!(decode_unreserved_escapes("%41"), Ok("A".to_string()));
    assert_eq!(
        decode_unreserved_escapes("ab%41cd"),
        Ok("abAcd".to_string())
    );
    assert_eq!(
        decode_unreserved_escapes("%7e%2d%5F"),
        Ok("~-_".to_string())
    );
    assert_eq!(
        decode_unreserved_escapes("%ab%Ef%3a%3A"),
        Ok("%AB%EF%3A%3A".to_string())
    );
    assert_eq!(decode_unreserved_escapes("%AF1"), Ok("%AF1".to_string()));
    assert_eq!(
        decode_unreserved_escapes("%4"),
        Err("truncated percent-escape at byte 0".to_string())
    );
    assert_eq!(
        decode_unreserved_escapes("a%4"),
        Err("truncated percent-escape at byte 1".to_string())
    );
    assert_eq!(
        decode_unreserved_escapes("%zz1"),
        Err("invalid percent-escape \"%zz\" at byte 0".to_string())
    );
    assert_eq!(
        Parts::parse("ctxloom+local:a%41%7e%2d%5f").map(|p| p.bundle),
        Ok("aA~-_".to_string())
    );
}

// Go's url.PathUnescape
#[test]
fn test_path_unescape_decodes_every_escape_and_refuses_a_bad_one() {
    assert_eq!(path_unescape("%41%42"), Ok("AB".to_string()));
    assert_eq!(path_unescape("x%2Fy"), Ok("x/y".to_string()));
    assert_eq!(path_unescape("%7E"), Ok("~".to_string()));
    assert_eq!(path_unescape("%e2%82%ac"), Ok("\u{20ac}".to_string()));
    assert_eq!(path_unescape("+a"), Ok("+a".to_string()));
    for (raw, shown) in [("%zz", "%zz"), ("%4", "%4"), ("a%", "%")] {
        assert_eq!(
            path_unescape(raw),
            Err(format!("invalid URL escape {shown:?}")),
            "{raw}"
        );
    }
}

// remote.IsFetchAddressRef: an http(s) or file URL, or an scp-like address
#[test]
fn test_a_fetch_address_is_a_url_or_an_scp_form() {
    for yes in ["https://a/b", "http://a/b", "file://a", "u@h:p"] {
        assert!(is_fetch_address(yes), "{yes}");
    }
    for no in ["ftp://x", "h:p", "a/b@c:d", "u@h:p://x", "u@h/x:p"] {
        assert!(!is_fetch_address(no), "{no}");
    }
    assert!(is_scp_form("u@h:p"));
    assert!(!is_scp_form("h:p"));
    assert!(!is_scp_form("a/b@c:d"));
    assert!(!is_scp_form("https://u@h:p"));
}

// remote.CanonicalSpelling and repository_key: the same repository whatever
// its spelling
#[test]
fn test_a_fetch_address_is_respelled_canonically_and_keys_by_repository() {
    for (raw, want, key) in [
        (
            "file:///repo@bundles/x@v1",
            "ctxloom+file:///repo//bundles/x@v1",
            "file:///repo",
        ),
        (
            "file:///repo.git/@bundles/x#fragments/a",
            "ctxloom+file:///repo//bundles/x#fragments/a",
            "file:///repo",
        ),
        (
            "https://Github.com/o/r.git@bundles/x/y@abc",
            "ctxloom+git://github.com/o/r//bundles/x/y@abc",
            "github.com/o/r",
        ),
        (
            "git@github.com:o/r.git@bundles/x",
            "ctxloom+git://github.com/o/r//bundles/x",
            "github.com/o/r",
        ),
        (
            "https://github.com/o/r",
            "https://github.com/o/r",
            "github.com/o/r",
        ),
        (
            "https://github.com/o/r@notbundles/x",
            "https://github.com/o/r@notbundles/x",
            "github.com/o/r",
        ),
    ] {
        assert_eq!(canonical_spelling(raw), want, "{raw}");
        assert_eq!(repository_key(raw).as_deref(), Some(key), "{raw}");
    }
    assert_eq!(
        repository_key("ctxloom+git://github.com/o/r//bundles/x#fragments/a").as_deref(),
        Some("github.com/o/r")
    );
    assert_eq!(
        repository_key("ctxloom+file:///p/r//bundles/x").as_deref(),
        Some("file:///p/r")
    );
    for none in [
        "ctxloom+local:x",
        "ctxloom+companion:x",
        "x",
        "ctxloom+nope:x",
    ] {
        assert_eq!(repository_key(none), None, "{none}");
    }
    assert!(same_repository(
        "git@github.com:o/r.git@bundles/x",
        "ctxloom+git://github.com/o/r//bundles/y"
    ));
    assert!(!same_repository("ctxloom+local:x", "ctxloom+local:x"));
}
