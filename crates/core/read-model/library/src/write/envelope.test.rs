use super::*;

// captured from a Go run: bundle create
#[test]
fn test_a_new_envelope_is_the_bytes_go_writes() {
    assert_eq!(
        new_envelope("1.0.0", "acceptance fixture bundle"),
        "schema_version: 4\nversion: 1.0.0\ndescription: acceptance fixture bundle\n"
    );
    assert_eq!(
        new_envelope("1.0.0", ""),
        "schema_version: 4\nversion: 1.0.0\n"
    );
    assert_eq!(
        new_envelope("2.0.0", "Go: the tool"),
        "schema_version: 4\nversion: 2.0.0\ndescription: \"Go: the tool\"\n"
    );
    // The envelope reads back as the bundle it describes.
    let (bundle, generation) =
        crate::document::parse_with_generation(&new_envelope("1.2.3", "d")).unwrap();
    assert_eq!(
        (
            bundle.version.as_str(),
            bundle.description.as_str(),
            generation
        ),
        ("1.2.3", "d", 4)
    );
}

fn edit(
    description: Option<&str>,
    version: Option<&str>,
    add: &[&str],
    remove: &[&str],
) -> EnvelopeEdit {
    EnvelopeEdit {
        description: description.map(ToString::to_string),
        version: version.map(ToString::to_string),
        add_tags: add.iter().map(ToString::to_string).collect(),
        remove_tags: remove.iter().map(ToString::to_string).collect(),
    }
}

const BASE: &str = "schema_version: 4\nversion: 1.0.0\ndescription: acceptance fixture bundle\n";

#[test]
fn test_the_scalar_edits_land_in_place_and_are_named() {
    let (text, changes) =
        edit_envelope(BASE, &edit(Some("updated-desc"), Some("2.0.0"), &[], &[])).unwrap();
    assert_eq!(
        text,
        "schema_version: 4\nversion: 2.0.0\ndescription: updated-desc\n"
    );
    assert_eq!(changes, ["updated description", "updated version"]);
}

// operations.applyScalarEdits: restating the held value is not a change
#[test]
fn test_restating_a_held_value_is_no_change() {
    let (text, changes) = edit_envelope(
        BASE,
        &edit(Some("acceptance fixture bundle"), Some("1.0.0"), &[], &[]),
    )
    .unwrap();
    assert_eq!(text, BASE);
    assert!(changes.is_empty());
    let (text, changes) = edit_envelope(BASE, &edit(None, None, &[], &[])).unwrap();
    assert_eq!((text.as_str(), changes.len()), (BASE, 0));
}

#[test]
fn test_tags_are_added_in_order_removed_by_name_and_never_doubled() {
    let (text, changes) =
        edit_envelope(BASE, &edit(None, None, &["golang", "b tag", "golang"], &[])).unwrap();
    assert_eq!(
        text,
        "schema_version: 4\nversion: 1.0.0\ndescription: acceptance fixture bundle\ntags:\n  - golang\n  - b tag\n"
    );
    assert_eq!(changes, ["added tag: golang", "added tag: b tag"]);
    let (text2, changes) = edit_envelope(&text, &edit(None, None, &["golang"], &[])).unwrap();
    assert_eq!((text2.as_str(), changes.len()), (text.as_str(), 0));
    let (text3, changes) =
        edit_envelope(&text, &edit(None, None, &["c"], &["golang", "nosuch"])).unwrap();
    assert!(text3.contains("  - b tag\n  - c\n"), "{text3}");
    assert!(!text3.contains("golang"), "{text3}");
    assert_eq!(changes, ["added tag: c", "removed tag: golang"]);
}

// operations.UpdateBundle (documented condition): a tag both added and
// removed is added and then removed, so it nets out
#[test]
fn test_a_tag_both_added_and_removed_nets_out() {
    let (text, changes) = edit_envelope(BASE, &edit(None, None, &["x"], &["x"])).unwrap();
    assert_eq!((text.as_str(), changes.len()), (BASE, 0));
    let (held, _) = edit_envelope(BASE, &edit(None, None, &["x"], &[])).unwrap();
    let (text, changes) = edit_envelope(&held, &edit(None, None, &["x"], &["x"])).unwrap();
    assert_eq!(text, BASE);
    assert_eq!(changes, ["removed tag: x"]);
}

#[test]
fn test_removing_the_last_tag_takes_the_key() {
    let (text, _) = edit_envelope(BASE, &edit(None, None, &["a"], &[])).unwrap();
    let (text, changes) = edit_envelope(&text, &edit(None, None, &[], &["a"])).unwrap();
    assert_eq!(text, BASE);
    assert_eq!(changes, ["removed tag: a"]);
}

#[test]
fn test_the_rest_of_the_envelope_is_as_the_author_left_it() {
    let text = "# my bundle\nschema_version: 4\nversion: 1.0.0 # first\nauthor: me\nrequires:\n  commands:\n    - git\n";
    let (out, changes) =
        edit_envelope(text, &edit(Some("now described"), None, &["t"], &[])).unwrap();
    assert_eq!(
        out,
        "# my bundle\nschema_version: 4\nversion: 1.0.0 # first\nauthor: me\nrequires:\n  commands:\n    - git\ndescription: now described\ntags:\n  - t\n"
    );
    assert_eq!(changes, ["updated description", "added tag: t"]);
}

#[test]
fn test_a_description_with_lines_is_written_as_a_block() {
    let (text, _) = edit_envelope(BASE, &edit(Some("multi\nline"), None, &[], &[])).unwrap();
    assert!(
        text.contains("description: |-\n  multi\n  line\n"),
        "{text}"
    );
    let (bundle, _) = crate::document::parse_with_generation(&text).unwrap();
    assert_eq!(bundle.description, "multi\nline");
}

#[test]
fn test_an_envelope_that_does_not_parse_is_refused() {
    let err = edit_envelope("version: [unclosed\n", &edit(Some("x"), None, &[], &[])).unwrap_err();
    assert!(matches!(err, BundleError::Syntax { .. }), "{err:?}");
}
