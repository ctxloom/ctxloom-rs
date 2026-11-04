use std::collections::BTreeMap;

use super::*;

/// A repository: its tags and branches, by name to commit.
struct Repo {
    tags: BTreeMap<&'static str, &'static str>,
    branches: BTreeMap<&'static str, &'static str>,
    default: &'static str,
    broken: bool,
}

impl Repo {
    /// A repository whose tags are `names` (each at a commit made of its
    /// position's digit), and whose branches are `main` and `dev`.
    fn tagged(names: &[&'static str]) -> Repo {
        let mut repo = Repo::new();
        repo.tags = names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, SHAS[i]))
            .collect();
        repo
    }

    fn new() -> Repo {
        Repo {
            tags: BTreeMap::from([
                ("v1.0.0", "1111111111111111111111111111111111111111"),
                ("v1.2.0", "2222222222222222222222222222222222222222"),
                ("release", "3333333333333333333333333333333333333333"),
            ]),
            branches: BTreeMap::from([
                ("main", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                ("dev", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                ("release", "cccccccccccccccccccccccccccccccccccccccc"),
            ]),
            default: "main",
            broken: false,
        }
    }
}

/// A commit name of forty copies of the digit `i + 1`.
const SHAS: [&str; 6] = [
    "1111111111111111111111111111111111111111",
    "2222222222222222222222222222222222222222",
    "3333333333333333333333333333333333333333",
    "4444444444444444444444444444444444444444",
    "5555555555555555555555555555555555555555",
    "6666666666666666666666666666666666666666",
];

impl RepoVersions for Repo {
    fn tags(&self) -> Result<Vec<String>, String> {
        if self.broken {
            return Err("tags: unreachable".to_string());
        }
        Ok(self.tags.keys().map(|t| t.to_string()).collect())
    }

    fn resolve_ref(&self, name: &str) -> Result<String, String> {
        if let Some(sha) = self.branches.get(name) {
            return Ok(sha.to_string());
        }
        self.resolve_tag(name)
    }

    fn resolve_tag(&self, name: &str) -> Result<String, String> {
        self.tags
            .get(name)
            .map(|s| s.to_string())
            .ok_or_else(|| format!("unknown revision {name}"))
    }

    fn default_branch(&self) -> Result<String, String> {
        Ok(self.default.to_string())
    }
}

// version_constraint.go: the four kinds and their lockfile names.
#[test]
fn test_a_kind_has_a_name_and_is_a_pin_or_not() {
    for (kind, name, pin) in [
        (Kind::Sha, "sha", true),
        (Kind::Tag, "tag", true),
        (Kind::Version, "version", false),
        (Kind::Branch, "branch", false),
    ] {
        assert_eq!(kind.name(), name);
        assert_eq!(Kind::parse(name), Some(kind));
        assert_eq!(kind.is_pin(), pin, "{name}");
    }
    assert_eq!(Kind::parse("nope"), None);
    assert_eq!(Kind::parse(""), None);
}

#[test]
fn test_a_commit_name_is_seven_to_forty_lowercase_hex_digits() {
    assert!(looks_like_commit("1234567"));
    assert!(looks_like_commit(&"a".repeat(40)));
    assert!(!looks_like_commit("123456"));
    assert!(!looks_like_commit(&"a".repeat(41)));
    assert!(!looks_like_commit("ABCDEF1"));
    assert!(!looks_like_commit("1234567g"));
    assert!(!looks_like_commit("v1.2.3"));
    assert!(!looks_like_commit(""));
}

#[test]
fn test_a_kind_is_inferred_from_the_shape_of_the_expression() {
    for (expr, kind) in [
        ("", Some(Kind::Branch)),
        ("abc1234", Some(Kind::Sha)),
        ("1.2.3", Some(Kind::Version)),
        ("v1.2.3", Some(Kind::Version)),
        ("^1.2", Some(Kind::Version)),
        ("~1.2.3", Some(Kind::Version)),
        (">=1.0.0", Some(Kind::Version)),
        ("1.x", Some(Kind::Version)),
        ("main", None),
        ("release", None),
        ("feature/x", None),
        ("2024-05-01", None),
        ("1.x.y", None),
    ] {
        assert_eq!(infer_kind(expr), kind, "{expr:?}");
    }
}

#[test]
fn test_an_explicit_prefix_overrides_the_shape() {
    assert_eq!(split_prefix("branch:1.0"), (Some(Kind::Branch), "1.0"));
    assert_eq!(split_prefix("tag:main"), (Some(Kind::Tag), "main"));
    assert_eq!(split_prefix("sha:abc1234"), (Some(Kind::Sha), "abc1234"));
    assert_eq!(split_prefix("version:^1"), (Some(Kind::Version), "^1"));
    assert_eq!(split_prefix("main"), (None, "main"));
    assert_eq!(split_prefix(""), (None, ""));
}

// LockEntry.SelectorKind
#[test]
fn test_an_entry_s_kind_is_the_recorded_one_else_derived() {
    assert_eq!(entry_kind(Some(Kind::Tag), "", ""), Kind::Tag);
    assert_eq!(entry_kind(None, "v1.2.0", "^1"), Kind::Version);
    assert_eq!(entry_kind(None, "", "abc1234"), Kind::Sha);
    assert_eq!(entry_kind(None, "", ""), Kind::Branch);
    assert_eq!(entry_kind(None, "", "main"), Kind::Branch);
    assert_eq!(entry_kind(None, "", "1.2.3"), Kind::Version);
}

// ResolveConstraint: branch (empty is the default branch)
#[test]
fn test_an_empty_selector_tracks_the_default_branch_tip() {
    let got = resolve("", &Repo::new()).expect("resolves");
    assert_eq!(got.sha, "a".repeat(40));
    assert_eq!(got.kind, Kind::Branch);
    assert_eq!(got.version, "");
}

#[test]
fn test_a_branch_selector_resolves_to_the_branch_tip() {
    let got = resolve("branch:dev", &Repo::new()).expect("resolves");
    assert_eq!(
        (got.sha.as_str(), got.kind),
        (&*"b".repeat(40), Kind::Branch)
    );
    assert!(resolve("branch:nope", &Repo::new()).is_err());
}

// A bare commit name is concrete.
#[test]
fn test_a_commit_selector_is_resolved_verbatim_through_the_repository() {
    let got = resolve("abc1234", &PassThrough).expect("resolves");
    assert_eq!(got.sha, "abc1234");
    assert_eq!(got.kind, Kind::Sha);
    assert_eq!(got.version, "");
}

struct PassThrough;

impl RepoVersions for PassThrough {
    fn tags(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn resolve_ref(&self, name: &str) -> Result<String, String> {
        Ok(name.to_string())
    }
    fn resolve_tag(&self, name: &str) -> Result<String, String> {
        Ok(name.to_string())
    }
    fn default_branch(&self) -> Result<String, String> {
        Ok("main".to_string())
    }
}

// A tag wins a bare name (a pin); a branch otherwise (a track); an explicit
// prefix overrides.
#[test]
fn test_a_bare_name_is_a_tag_when_the_repository_has_one_else_a_branch() {
    let repo = Repo::new();
    let tag = resolve("release", &repo).expect("resolves");
    assert_eq!((tag.sha.as_str(), tag.kind), (&*"3".repeat(40), Kind::Tag));
    assert_eq!(tag.version, "release");
    let branch = resolve("dev", &repo).expect("resolves");
    assert_eq!(
        (branch.sha.as_str(), branch.kind),
        (&*"b".repeat(40), Kind::Branch)
    );
    assert_eq!(branch.version, "");
    let forced = resolve("branch:release", &repo).expect("resolves");
    assert_eq!(forced.sha, "c".repeat(40));
    let forced = resolve("tag:release", &repo).expect("resolves");
    assert_eq!(
        (forced.sha.as_str(), forced.kind),
        (&*"3".repeat(40), Kind::Tag)
    );
}

#[test]
fn test_a_failed_tag_listing_is_not_read_as_not_a_tag() {
    let mut repo = Repo::new();
    repo.broken = true;
    let refused = resolve("release", &repo).expect_err("the listing failed");
    assert!(
        matches!(refused, ResolveError::Repository { .. }),
        "{refused:?}"
    );
}

#[test]
fn test_an_exact_version_resolves_to_its_tag() {
    let repo = Repo::new();
    for expr in ["1.2.0", "v1.2.0", "=1.2.0", "version:1.2.0"] {
        let got = resolve(expr, &repo).unwrap_or_else(|e| panic!("{expr}: {e}"));
        assert_eq!(got.sha, "2".repeat(40), "{expr}");
        assert_eq!(got.version, "v1.2.0", "{expr}");
        assert_eq!(got.kind, Kind::Version, "{expr}");
    }
    assert_eq!(
        resolve("9.9.9", &repo),
        Err(ResolveError::NoTag {
            expr: "9.9.9".to_string()
        })
    );
}

// version_constraint_test.go: TestResolveConstraint_SemverRangePicksHighest
// (owner, 2026-11-04: ranges are semver ranges, the Cargo spelling).
#[test]
fn test_a_version_range_resolves_to_the_newest_tag_it_allows() {
    let repo = Repo::tagged(&["v1.0.0", "v1.2.0", "v1.3.0", "v2.0.0"]);
    for (expr, tag, sha) in [
        ("^1.2", "v1.3.0", "3"),
        ("^1", "v1.3.0", "3"),
        ("~1.2", "v1.2.0", "2"),
        (">=1.0, <2", "v1.3.0", "3"),
        (">=1.0.0", "v2.0.0", "4"),
        ("*", "v2.0.0", "4"),
        ("1.x", "v1.3.0", "3"),
        ("version:^1.2", "v1.3.0", "3"),
    ] {
        let got = resolve(expr, &repo).unwrap_or_else(|e| panic!("{expr}: {e}"));
        assert_eq!(got.version, tag, "{expr}");
        assert_eq!(got.sha, sha.repeat(40), "{expr}");
        assert_eq!(got.kind, Kind::Version, "{expr}");
    }
}

// version_constraint_test.go: TestResolveConstraint_SemverNoMatch
#[test]
fn test_a_range_no_tag_satisfies_is_refused_with_the_constraint() {
    let repo = Repo::tagged(&["v1.0.0", "v1.5.0"]);
    assert_eq!(
        resolve("^2", &repo),
        Err(ResolveError::NoTag {
            expr: "^2".to_string()
        })
    );
}

// version_constraint_test.go: TestResolveConstraint_SemverIgnoresNonSemverTags
#[test]
fn test_a_range_ignores_tags_that_are_not_semver() {
    let repo = Repo::tagged(&["nightly", "latest", "v1.2.0", "release", "v1.3.0"]);
    assert_eq!(resolve("^1", &repo).expect("^1").version, "v1.3.0");
}

// Owner ruling, 2026-11-04: for 0.y.z a minor bump is the breaking one, so
// ^0.3 does not reach 0.4.
#[test]
fn test_a_range_below_one_point_zero_stays_inside_its_minor() {
    let repo = Repo::tagged(&["v0.3.1", "v0.3.9", "v0.4.0"]);
    for expr in ["^0.3", "^0.3.1", "0.3"] {
        assert_eq!(
            resolve(expr, &repo).expect(expr).version,
            "v0.3.9",
            "{expr}"
        );
    }
}

// An exact version is a pin, even where a newer tag exists: pins move only on
// `deps upgrade --yes` (plan over Cargo's implicit caret on a bare version).
#[test]
fn test_a_full_version_is_exact_even_where_newer_tags_exist() {
    let repo = Repo::tagged(&["v1.0.0", "v1.2.0", "v1.3.0"]);
    for expr in ["1.0.0", "v1.0.0", "=1.0.0"] {
        let got = resolve(expr, &repo).unwrap_or_else(|e| panic!("{expr}: {e}"));
        assert_eq!(got.version, "v1.0.0", "{expr}");
        assert_eq!(got.sha, "1".repeat(40), "{expr}");
    }
}

// Owner ruling, 2026-11-04: nothing is refused for its version not being
// semver. A name that only looks like a version is a tag if the repository
// has it, else a branch.
#[test]
fn test_an_expression_that_looks_like_a_version_but_is_not_one_is_a_name() {
    let mut repo = Repo::new();
    repo.tags
        .insert("2024-05-01", "7777777777777777777777777777777777777777");
    repo.branches
        .insert("2024-06-01", "8888888888888888888888888888888888888888");
    let tag = resolve("2024-05-01", &repo).expect("a tag named so");
    assert_eq!(
        (tag.kind, tag.version.as_str(), tag.sha.as_str()),
        (Kind::Tag, "2024-05-01", "7".repeat(40).as_str())
    );
    let branch = resolve("2024-06-01", &repo).expect("a branch named so");
    assert_eq!(
        (branch.kind, branch.version.as_str(), branch.sha.as_str()),
        (Kind::Branch, "", "8".repeat(40).as_str())
    );
}

// version_constraint_test.go: TestResolveConstraint_ForcedVersionRejectsUnparseableExpr
#[test]
fn test_a_forced_version_that_is_not_a_range_is_refused_with_the_expression() {
    for expr in ["version:nightly", "version:", "version:1.x.y"] {
        let refused = resolve(expr, &Repo::new()).expect_err(expr);
        let ResolveError::Range { expr: shown } = &refused else {
            panic!("{expr}: {refused:?}");
        };
        assert_eq!(shown, expr.strip_prefix("version:").expect("a prefix"));
        assert!(refused.to_string().contains("version"), "{refused}");
    }
}

#[test]
fn test_every_resolve_error_says_what_it_is() {
    for (error, text) in [
        (
            ResolveError::Repository {
                what: "list tags".to_string(),
                cause: "unreachable".to_string(),
            },
            "list tags: unreachable",
        ),
        (
            ResolveError::NoTag {
                expr: "9.9.9".to_string(),
            },
            "no tag satisfies version constraint \"9.9.9\"",
        ),
    ] {
        assert_eq!(error.to_string(), text);
    }
    let range = ResolveError::Range {
        expr: "nightly".to_string(),
    }
    .to_string();
    assert_eq!(range, "\"nightly\" is not a version or a version range");
}

// A version short of its minor or patch is a range, as Cargo reads it (plan
// supersedes Go's Masterminds, where 1.2 is 1.2.x): ^1, ^1.2.
#[test]
fn test_a_short_version_is_a_caret_range() {
    let repo = Repo::tagged(&["v1.0.0", "v1.2.0", "v1.3.0", "v2.0.0"]);
    assert_eq!(resolve("1", &repo).expect("1").version, "v1.3.0");
    assert_eq!(resolve("1.2", &repo).expect("1.2").version, "v1.3.0");
    assert_eq!(resolve("v1.2", &repo).expect("v1.2").version, "v1.3.0");
}
