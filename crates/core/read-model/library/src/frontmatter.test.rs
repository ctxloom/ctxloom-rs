//! Ported from Go's `internal/adapters/content/frontmatter_test.go`
//! (860336c4e).

use super::*;

// frontmatter_test.go: TestSplitFrontMatter_DoesNotCorruptBodies
#[test]
fn test_a_body_with_its_own_rule_and_mustaches_is_not_corrupted() {
    let body = "Intro paragraph.\n\n---\n\nAfter the rule: {{ not_a_template }} and {{{ triple }}}.\n\n---\n";
    let raw = format!("---\ntags:\n  - edge-case\n---\n{body}");
    let split = split_front_matter(&raw).expect("front matter is found");
    assert_eq!(split.meta.trim(), "tags:\n  - edge-case");
    assert_eq!(split.body, body);
}

// frontmatter_test.go: TestSplitFrontMatter_AbsentAndUnterminated
#[test]
fn test_absent_and_unterminated_front_matter_is_none() {
    for (what, raw) in [
        ("no fence at all", "# Just a heading\n\nSome prose.\n"),
        // An unterminated block must NOT be read as front matter: that would
        // swallow the whole document into metadata and leave an empty body.
        (
            "unterminated",
            "---\ntags:\n  - a\n\nstill prose, no closing fence\n",
        ),
        (
            "fence not on the first line",
            "\n---\ntags: [a]\n---\nbody\n",
        ),
    ] {
        assert_eq!(split_front_matter(raw), None, "{what}");
    }
}

// content/surface_markdown.go + tree fixtures: a fence line may carry
// trailing blanks, and a CRLF file's fences are fences.
#[test]
fn test_a_fence_line_may_end_in_blanks_or_a_carriage_return() {
    let split = split_front_matter("---  \r\ntags: [a]\r\n---\t\r\nbody\r\n").expect("fenced");
    assert_eq!(split.meta, "tags: [a]\r\n");
    assert_eq!(split.body, "body\r\n");
}

// frontmatter_test.go: TestPathHelpers
#[test]
fn test_path_helpers() {
    let cases = [
        ("fragments/solid.md", false, "solid"),
        ("fragments/solid.distilled.md", false, "solid"),
        ("fragments/.solid.meta.yaml", true, "solid"),
        ("fragments/.solid.distilled.meta.yaml", true, "solid"),
        ("mcp/postgres.yaml", false, "postgres"),
        ("mcp/.postgres.meta.yaml", true, "postgres"),
        ("hooks/pre_tool/guard.yaml", false, "guard"),
        ("skills/.code-reviewer.meta.yaml", true, "code-reviewer"),
        ("skills/code-reviewer/SKILL.md", false, "SKILL"),
    ];
    for (path, is_meta, stem) in cases {
        assert_eq!(is_meta_path(path), is_meta, "is_meta_path({path})");
        assert_eq!(stem_of(path), stem, "stem_of({path})");
    }
    assert_eq!(meta_path("mcp/postgres.yaml"), "mcp/.postgres.meta.yaml");
    assert_eq!(
        meta_path_for_name("skills", "code-reviewer"),
        "skills/.code-reviewer.meta.yaml"
    );
    // paths.go MetaPathForName: the dot goes on the LAST segment of a name
    // that holds a separator, or the sidecar lands in a hidden directory.
    assert_eq!(
        meta_path_for_name("hooks", "pre_tool/guard"),
        "hooks/pre_tool/.guard.meta.yaml"
    );
    // paths.go IsMetaPath: the bare `.meta.yaml` is no sidecar.
    assert!(!is_meta_path("fragments/..meta.yaml"));
}
