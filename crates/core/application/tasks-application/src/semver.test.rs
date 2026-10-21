//! SemVer 2.0.0, strictly: Go's `type_comparator_test.go` cases, and the
//! strictness Go's `StrictNewVersion` holds a written value to.

use std::cmp::Ordering;

use tagma_core::TypeComparator;

use super::{Semver, Version};

// SemVer 2.0.0 §11's own chain, each strictly below the next, both ways.
#[test]
fn test_precedence_follows_the_specs_chain() {
    let chain = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
        "1.0.1",
        "1.1.0",
        "2.0.0",
    ];
    for pair in chain.windows(2) {
        assert_eq!(
            Semver.compare(pair[0], pair[1]),
            Some(Ordering::Less),
            "{pair:?}"
        );
        assert_eq!(
            Semver.compare(pair[1], pair[0]),
            Some(Ordering::Greater),
            "{pair:?}"
        );
    }
}

#[test]
fn test_equal_versions_compare_equal_and_build_metadata_takes_no_part() {
    assert_eq!(Semver.compare("1.2.3", "1.2.3"), Some(Ordering::Equal));
    assert_eq!(Semver.compare("1.0.0+a", "1.0.0+b"), Some(Ordering::Equal));
    assert_eq!(
        Semver.compare("1.0.0-rc.1+build.5", "1.0.0-rc.1"),
        Some(Ordering::Equal)
    );
}

#[test]
fn test_a_side_that_is_no_version_is_not_comparable() {
    for (a, b) in [
        ("not-a-version", "1.0.0"),
        ("1.0.0", "not-a-version"),
        ("nope", "also-nope"),
        ("1.2", "1.0.0"),
        ("v1.2.3", "1.0.0"),
        ("", "1.0.0"),
    ] {
        assert_eq!(Semver.compare(a, b), None, "{a:?} {b:?}");
    }
}

#[test]
fn test_a_version_is_parsed_strictly() {
    for valid in [
        "0.0.0",
        "0.7.0-pre001",
        "1.0.0-0.3.7",
        "1.0.0-x.7.z.92",
        "1.0.0-x-y-z.--",
        "1.0.0+20130313144700",
        "1.0.0-beta+exp.sha.5114f85",
    ] {
        assert!(Version::parse(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "1",
        "1.2",
        "1.2.3.4",
        "01.2.3",
        "1.02.3",
        "1.2.03",
        "1.2.3-01",
        "1.2.3-",
        "1.2.3-a..b",
        "1.2.3+",
        "1.2.3+a_b",
        "1.2.3-a b",
        "a.b.c",
        "-1.2.3",
        "1.2.-3",
        "99999999999999999999.0.0",
    ] {
        assert!(Version::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn test_a_numeric_identifier_is_below_an_alphanumeric_one() {
    let v = |s| Version::parse(s).unwrap();
    assert!(v("1.0.0-1") < v("1.0.0-a"));
    assert!(v("1.0.0-2") < v("1.0.0-10"), "numerically");
    assert!(v("1.0.0-b") > v("1.0.0-a"));
    assert!(
        v("1.0.0-a") < v("1.0.0-a.1"),
        "more identifiers rank higher"
    );
}

#[test]
fn test_identifiers_order_the_same_partially() {
    use super::Identifier;
    let (one, a) = (
        Identifier::Numeric(1),
        Identifier::Alphanumeric("a".to_string()),
    );
    assert_eq!(one.partial_cmp(&a), Some(Ordering::Less));
    assert_eq!(a.partial_cmp(&one), Some(Ordering::Greater));
}

#[test]
fn test_a_part_that_is_no_number_says_so() {
    for (bad, why) in [
        ("a.0.0", "is not a number"),
        ("1..0", "is not a number"),
        ("01.0.0", "leading zero"),
        ("1.0.0-01", "leading zero"),
    ] {
        let error = Version::parse(bad).unwrap_err();
        assert!(error.contains(why), "{bad}: {error}");
    }
}
