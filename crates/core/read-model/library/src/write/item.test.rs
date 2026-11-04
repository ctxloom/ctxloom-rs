use super::*;

// Go's paths.go itemPath
#[test]
fn test_paths_are_the_kind_directory_the_name_and_the_form() {
    assert_eq!(item_path(FRAGMENTS, "testing"), "fragments/testing.md");
    assert_eq!(item_path(PROMPTS, "review"), "prompts/review.md");
    assert_eq!(
        distilled_path(FRAGMENTS, "testing"),
        "fragments/testing.distilled.md"
    );
    assert_eq!(distilled_path(PROMPTS, "a.b"), "prompts/a.b.distilled.md");
}

// surface_markdown.go encodeMarkdownItem: a single path segment
#[test]
fn test_a_valid_name_is_one_visible_segment() {
    for good in ["testing", "a-b", "a_b", "a.b", "x1", "\u{dc}ber"] {
        assert!(valid_name(good), "{good}");
    }
    for bad in [
        "", ".", "..", ".hidden", "a/b", "a\\b", "/abs", "a\0b", "a/../b",
    ] {
        assert!(!valid_name(bad), "{bad:?}");
    }
}

// captured from a Go run: bundle edit --add-fragment / fragment create
#[test]
fn test_the_placeholders_are_the_bytes_go_writes() {
    assert_eq!(
        placeholder_fragment("testing"),
        "# testing\n\nAdd content here."
    );
    assert_eq!(
        placeholder_prompt("review"),
        "---\ndescription: review\n---\nAdd prompt content here."
    );
    // A name the YAML writer must quote is quoted in the description.
    assert_eq!(
        placeholder_prompt("a: b"),
        "---\ndescription: \"a: b\"\n---\nAdd prompt content here."
    );
}

// captured from a Go run: bundle create
#[test]
fn test_the_seeds_are_the_bytes_go_writes() {
    assert_eq!(
        seed_fragment(),
        "---\ntags:\n  - example\nno_distill: true\n---\n# Example Fragment\n\nAdd your content here."
    );
    assert_eq!(
        seed_prompt(),
        "---\ndescription: Example prompt\ntags:\n  - example\nno_distill: true\n---\nExample prompt content. Describe what this prompt does."
    );
}

#[test]
fn test_the_body_is_what_follows_the_front_matter() {
    assert_eq!(body_of("plain"), "plain");
    assert_eq!(body_of("---\ntags:\n  - t\n---\nBODY\nmore"), "BODY\nmore");
    assert_eq!(body_of("---\ntags: x\n---\n"), "");
    // Not front matter: unterminated, or not the first line.
    assert_eq!(body_of("---\nno end"), "---\nno end");
    assert_eq!(
        body_of("text\n---\na: b\n---\nx"),
        "text\n---\na: b\n---\nx"
    );
    // A rule in the body is the body's.
    assert_eq!(body_of("---\na: b\n---\none\n---\ntwo"), "one\n---\ntwo");
    assert_eq!(body_of(""), "");
}

#[test]
fn test_a_marked_item_is_no_distill() {
    assert!(marked_no_distill("---\nno_distill: true\n---\nx"));
    assert!(marked_no_distill(
        "---\ntags:\n  - e\nno_distill: true\n---\nx"
    ));
    assert!(!marked_no_distill("---\nno_distill: false\n---\nx"));
    assert!(!marked_no_distill("---\ntags: x\n---\nx"));
    assert!(!marked_no_distill("no_distill: true\nx"));
    assert!(!marked_no_distill("plain"));
    assert!(!marked_no_distill("---\n: broken [\n---\nx"));
}

// item_crud.go editItem + invalidateDerived
#[test]
fn test_replacing_the_body_keeps_the_front_matter_minus_the_hash() {
    assert_eq!(replace_body("old", "new\n"), "new\n");
    assert_eq!(
        replace_body(
            "---\ntags:\n  - t\ncontent_hash: sha256:abc\nno_distill: true\n---\nBODY",
            "BODY\nEDITED-BY-TEST\n"
        ),
        "---\ntags:\n  - t\nno_distill: true\n---\nBODY\nEDITED-BY-TEST\n"
    );
    // A comment in the front matter is the author's.
    assert_eq!(
        replace_body("---\n# mine\ntags: [a]\n---\nold", "new"),
        "---\n# mine\ntags: [a]\n---\nnew"
    );
    // The hash was all there was: no front matter is left.
    assert_eq!(
        replace_body("---\ncontent_hash: sha256:abc\n---\nold", "new"),
        "new"
    );
    // The body may be empty only as the caller says.
    assert_eq!(
        replace_body("---\ntags: [a]\n---\nold", ""),
        "---\ntags: [a]\n---\n"
    );
}

#[test]
fn test_replacing_the_body_of_a_file_that_is_not_front_matter_replaces_it_all() {
    assert_eq!(replace_body("---\nunterminated", "new"), "new");
    assert_eq!(replace_body("", "new"), "new");
}
