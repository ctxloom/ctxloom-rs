use super::*;

fn tags(names: &[&str]) -> Vec<String> {
    names.iter().map(|t| t.to_string()).collect()
}

fn req(text: &str) -> VersionReq {
    VersionReq::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

// Owner ruling, 2026-11-04: a version that parses is compared by the semver
// spec.
#[test]
fn test_a_tag_parses_as_semver_after_a_v() {
    for (tag, want) in [
        ("1.2.3", "1.2.3"),
        ("v1.2.3", "1.2.3"),
        ("V1.2.3", "1.2.3"),
        ("1.2.3-rc.1", "1.2.3-rc.1"),
        ("1.2.3+build.5", "1.2.3+build.5"),
    ] {
        let parsed = parse(tag).unwrap_or_else(|| panic!("{tag} is semver"));
        assert_eq!(parsed.to_string(), want, "{tag}");
    }
}

#[test]
fn test_a_name_that_is_not_semver_does_not_parse() {
    for tag in [
        "main",
        "nightly",
        "",
        "1.2",
        "v1",
        "2024",
        "2024-05-01",
        "1.2.3.4",
        "1.a",
        "1..2",
        "+1",
        "v",
        "01.2.3",
    ] {
        assert_eq!(parse(tag), None, "{tag:?}");
    }
}

#[test]
fn test_a_compat_is_shown_as_its_word() {
    assert_eq!(Compat::Breaking.name(), "breaking");
    assert_eq!(Compat::Compatible.name(), "compatible");
    assert_eq!(Compat::Unknown.name(), "unknown");
}

// Owner ruling, 2026-11-04: breaking changes bump MAJOR from 1.0.0.
#[test]
fn test_from_one_point_zero_a_major_bump_is_breaking_and_the_rest_compatible() {
    assert_eq!(classify("1.2.3", "2.0.0"), Compat::Breaking);
    assert_eq!(classify("v1.2.3", "v2.0.0"), Compat::Breaking);
    assert_eq!(classify("1.2.3", "3.1.0"), Compat::Breaking);
    assert_eq!(classify("1.2.3", "1.3.0"), Compat::Compatible);
    assert_eq!(classify("1.2.3", "1.2.4"), Compat::Compatible);
    assert_eq!(classify("1.2.3", "1.9.0"), Compat::Compatible);
    assert_eq!(classify("1.0.0", "1.0.1"), Compat::Compatible);
}

// Owner ruling, 2026-11-04: for 0.y.z a MINOR bump is the breaking one (the
// Cargo caret convention); no "minor for breaks" divergence.
#[test]
fn test_below_one_point_zero_a_minor_bump_is_the_breaking_one() {
    assert_eq!(classify("0.3.1", "0.4.0"), Compat::Breaking);
    assert_eq!(classify("0.3.1", "0.3.2"), Compat::Compatible);
    assert_eq!(classify("0.3.1", "0.3.9"), Compat::Compatible);
    assert_eq!(classify("0.3.1", "1.0.0"), Compat::Breaking);
    assert_eq!(classify("0.9.0", "1.0.0"), Compat::Breaking);
    assert_eq!(classify("0.1.0", "0.2.0"), Compat::Breaking);
}

#[test]
fn test_below_zero_point_one_even_a_patch_bump_is_the_breaking_one() {
    assert_eq!(classify("0.0.3", "0.0.4"), Compat::Breaking);
    assert_eq!(classify("0.0.3", "0.1.0"), Compat::Breaking);
}

// A downgrade across a line is still a break for whoever used the newer API;
// within a line the newer API's features may be gone: nothing says.
#[test]
fn test_a_move_down_is_breaking_across_a_line_and_unknown_within_one() {
    assert_eq!(classify("2.0.0", "1.9.0"), Compat::Breaking);
    assert_eq!(classify("0.4.0", "0.3.0"), Compat::Breaking);
    assert_eq!(classify("1.3.0", "1.2.0"), Compat::Unknown);
    assert_eq!(classify("0.3.2", "0.3.1"), Compat::Unknown);
}

// The semver spec: a pre-release is unstable and may not satisfy the
// compatibility its version implies; build metadata is not part of precedence.
#[test]
fn test_a_pre_release_is_unknown_and_build_metadata_is_ignored() {
    assert_eq!(classify("1.0.0", "2.0.0-rc.1"), Compat::Unknown);
    assert_eq!(classify("1.0.0-rc.1", "1.0.0"), Compat::Unknown);
    assert_eq!(classify("1.0.0-rc.1", "1.0.0-rc.2"), Compat::Unknown);
    assert_eq!(classify("1.0.0+a", "1.0.0+b"), Compat::Unknown);
    assert_eq!(classify("1.0.0+a", "1.1.0+b"), Compat::Compatible);
}

// The content moved and the version did not: nothing says it is safe.
#[test]
fn test_no_change_of_version_is_unknown() {
    assert_eq!(classify("1.2.3", "1.2.3"), Compat::Unknown);
    assert_eq!(classify("v1.2.3", "1.2.3"), Compat::Unknown);
    assert_eq!(classify("", ""), Compat::Unknown);
    assert_eq!(classify("main", "main"), Compat::Unknown);
}

// Owner ruling, 2026-11-04: where a version is not semver, any difference is
// evidence of a possible break, and nothing is refused.
#[test]
fn test_a_version_that_is_not_semver_is_unknown_whatever_the_other_is() {
    for (from, to) in [
        ("main", "dev"),
        ("", "1.0.0"),
        ("1.0.0", ""),
        ("1.0.0", "nightly"),
        ("nightly", "2.0.0"),
        ("2024-05-01", "2024-06-01"),
        ("2024", "2025"),
        ("1.2.3.4", "1.2.3.5"),
    ] {
        assert_eq!(classify(from, to), Compat::Unknown, "{from:?} -> {to:?}");
    }
}

// version_constraint_test.go: TestResolveConstraint_SemverRangePicksHighest
#[test]
fn test_the_newest_tag_a_range_allows_is_the_one_chosen() {
    let all = tags(&["v1.0.0", "v1.2.0", "v1.3.0", "v2.0.0"]);
    assert_eq!(
        newest_matching(&req("^1.2"), &all),
        Some("v1.3.0".to_string())
    );
    assert_eq!(
        newest_matching(&req(">=1.0.0"), &all),
        Some("v2.0.0".to_string())
    );
    assert_eq!(
        newest_matching(&req("~1.2"), &all),
        Some("v1.2.0".to_string())
    );
}

// version_constraint_test.go: TestResolveConstraint_SemverIgnoresNonSemverTags
#[test]
fn test_a_tag_that_is_not_semver_is_not_a_candidate() {
    let all = tags(&["nightly", "latest", "v1.2.0", "release", "v1.3.0"]);
    assert_eq!(
        newest_matching(&req("^1"), &all),
        Some("v1.3.0".to_string())
    );
    assert_eq!(newest_matching(&req("*"), &tags(&["nightly"])), None);
}

// version_constraint_test.go: TestResolveConstraint_SemverPrefersPrefixedOriginal
#[test]
fn test_the_tag_is_returned_as_it_was_written() {
    let all = tags(&["1.2.0", "v1.3.0"]);
    assert_eq!(
        newest_matching(&req(">=1.0.0"), &all),
        Some("v1.3.0".to_string())
    );
    assert_eq!(
        newest_matching(&req("<1.3.0"), &all),
        Some("1.2.0".to_string())
    );
}

// version_constraint_test.go: TestResolveConstraint_SemverNoMatch
#[test]
fn test_no_tag_in_range_is_none() {
    assert_eq!(
        newest_matching(&req("^2"), &tags(&["v1.0.0", "v1.5.0"])),
        None
    );
    assert_eq!(newest_matching(&req("^1"), &[]), None);
}

// Cargo's caret: ^0.3 stays inside 0.3.x.
#[test]
fn test_a_caret_range_below_one_stays_inside_its_minor() {
    let all = tags(&["v0.3.1", "v0.3.9", "v0.4.0", "v1.0.0"]);
    assert_eq!(
        newest_matching(&req("^0.3"), &all),
        Some("v0.3.9".to_string())
    );
    assert_eq!(
        newest_matching(&req("^0.0.3"), &tags(&["v0.0.3", "v0.0.4"])),
        Some("v0.0.3".to_string())
    );
}

// The semver crate's own rule, the Cargo one: a pre-release is a candidate
// only where the range names one.
#[test]
fn test_a_pre_release_is_a_candidate_only_where_the_range_names_one() {
    let all = tags(&["v1.3.0", "v1.4.0-rc.1"]);
    assert_eq!(
        newest_matching(&req("^1"), &all),
        Some("v1.3.0".to_string())
    );
    assert_eq!(
        newest_matching(&req(">=1.4.0-rc.0"), &all),
        Some("v1.4.0-rc.1".to_string())
    );
}

// Precedence, not text: 1.10.0 is above 1.9.0, and a tie goes to the same tag
// every run.
#[test]
fn test_the_newest_is_by_precedence_and_a_tie_is_settled_the_same_way_every_time() {
    assert_eq!(
        newest_matching(&req("^1"), &tags(&["v1.9.0", "v1.10.0", "v1.2.0"])),
        Some("v1.10.0".to_string())
    );
    let tie = newest_matching(&req("^1"), &tags(&["v1.2.0", "1.2.0"]));
    let flipped = newest_matching(&req("^1"), &tags(&["1.2.0", "v1.2.0"]));
    assert_eq!(tie, flipped);
}

// A full version is a pin; anything else is a range as Cargo reads it.
#[test]
fn test_a_full_version_is_exact_and_anything_else_is_a_range() {
    let allows = |expr: &str, version: &str| {
        range(expr)
            .unwrap_or_else(|| panic!("{expr} is a range"))
            .matches(&Version::parse(version).expect("a version"))
    };
    assert!(allows("1.2.3", "1.2.3") && !allows("1.2.3", "1.2.4"));
    assert!(allows("v1.2.3", "1.2.3") && !allows("v1.2.3", "1.3.0"));
    assert!(allows("=1.2.3", "1.2.3") && !allows("=1.2.3", "1.2.4"));
    assert!(allows("1.2.3-rc.1", "1.2.3-rc.1") && !allows("1.2.3-rc.1", "1.2.3"));
    assert!(allows("1.2", "1.9.0") && !allows("1.2", "2.0.0"));
    assert!(allows("v1.2", "1.9.0"));
    assert!(allows("^1", "1.9.0") && allows("~1.2", "1.2.9") && !allows("~1.2", "1.3.0"));
    assert!(allows(">=1, <2", "1.5.0") && !allows(">=1, <2", "2.0.0"));
    assert!(allows("1.x", "1.5.0") && allows("*", "9.9.9"));
    assert!(allows(" ^1 ", "1.0.0"));
}

#[test]
fn test_what_is_not_a_range_is_none() {
    for expr in [
        "",
        "nightly",
        "main",
        "2024-05-01",
        "1.x.y",
        "1.2.3.4",
        ">=",
        "^",
    ] {
        assert!(range(expr).is_none(), "{expr:?}");
    }
}
