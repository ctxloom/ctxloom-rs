use super::*;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

#[test]
fn test_a_profile_lives_in_its_bundles_profiles_directory() {
    assert_eq!(profile_path("dev"), "profiles/dev.yaml");
    assert_eq!(PROJECT_BUNDLE, "project");
}

// captured from a Go run: profile create
#[test]
fn test_a_new_profile_is_the_bytes_go_writes() {
    let new = NewProfile {
        description: "day-to-day-work".to_string(),
        bundles: strings(&["demo"]),
        ..NewProfile::default()
    };
    assert_eq!(
        new_profile(&new),
        "description: day-to-day-work\nbundles:\n  - demo\n"
    );
    let full = NewProfile {
        description: "d".to_string(),
        parents: strings(&["base", "other"]),
        llm: "claude-fast".to_string(),
        bundles: strings(&["demo", "x"]),
    };
    assert_eq!(
        new_profile(&full),
        "description: d\nparents:\n  - base\n  - other\nllm: claude-fast\nbundles:\n  - demo\n  - x\n"
    );
    let parent_only = NewProfile {
        parents: strings(&["base"]),
        ..NewProfile::default()
    };
    assert_eq!(new_profile(&parent_only), "parents:\n  - base\n");
    // The document reads back as the profile.
    let read = crate::profile::Profile::decode(&new_profile(&full)).unwrap();
    assert_eq!(
        (read.description.as_str(), read.llm.as_str()),
        ("d", "claude-fast")
    );
    assert_eq!(read.parents, ["base", "other"]);
    assert_eq!(read.bundles, ["demo", "x"]);
}

const BASE: &str = "description: acceptance fixture profile\nbundles:\n  - demo\n";

fn edit() -> ProfileEdit {
    ProfileEdit::default()
}

#[test]
fn test_an_edit_that_names_nothing_changes_nothing() {
    let (text, changes) = edit_profile(BASE, &edit()).unwrap();
    assert_eq!(text, BASE);
    assert!(changes.is_empty());
    // Restatements and removals of what is not there are no change either.
    let restate = ProfileEdit {
        description: Some("acceptance fixture profile".to_string()),
        add_bundles: strings(&["demo"]),
        remove_bundles: strings(&["nope"]),
        remove_parents: strings(&["nope"]),
        include_fragments: strings(&["nope"]),
        include_mcp: strings(&["nope"]),
        ..edit()
    };
    let (text, changes) = edit_profile(BASE, &restate).unwrap();
    assert_eq!((text.as_str(), changes.len()), (BASE, 0));
}

// captured from a Go run: profile modify, every flag at once
#[test]
fn test_every_change_is_named_in_go_s_order() {
    let all = ProfileEdit {
        description: Some("new d".to_string()),
        llm: Some("claude-fast".to_string()),
        add_parents: strings(&["dev3"]),
        add_bundles: strings(&["demo2", "demo3"]),
        exclude_fragments: strings(&["f1", "f2"]),
        exclude_mcp: strings(&["m1"]),
        ..edit()
    };
    let (text, changes) = edit_profile(BASE, &all).unwrap();
    assert_eq!(
        changes,
        [
            "updated description",
            "set llm to \"claude-fast\"",
            "added parent: dev3",
            "added bundle: demo2",
            "added bundle: demo3",
            "added exclude fragment: f1",
            "added exclude fragment: f2",
            "added exclude mcp: m1",
        ]
    );
    let read = crate::profile::Profile::decode(&text).unwrap();
    assert_eq!(read.description, "new d");
    assert_eq!(read.llm, "claude-fast");
    assert_eq!(read.parents, ["dev3"]);
    assert_eq!(read.bundles, ["demo", "demo2", "demo3"]);
    assert_eq!(read.exclude_fragments, ["f1", "f2"]);
    assert_eq!(read.exclude_mcp, ["m1"]);
    let undo = ProfileEdit {
        llm: Some(String::new()),
        remove_parents: strings(&["dev3"]),
        remove_bundles: strings(&["demo2"]),
        include_fragments: strings(&["f1"]),
        include_mcp: strings(&["m1"]),
        ..edit()
    };
    let (text, changes) = edit_profile(&text, &undo).unwrap();
    assert_eq!(
        changes,
        [
            "cleared llm",
            "removed parent: dev3",
            "removed bundle: demo2",
            "removed exclude fragment: f1",
            "removed exclude mcp: m1",
        ]
    );
    let read = crate::profile::Profile::decode(&text).unwrap();
    assert_eq!(read.llm, "");
    assert!(read.parents.is_empty() && read.exclude_mcp.is_empty());
    assert_eq!(read.bundles, ["demo", "demo3"]);
    assert_eq!(read.exclude_fragments, ["f2"]);
    assert!(
        !text.contains("parents") && !text.contains("exclude_mcp"),
        "{text}"
    );
}

#[test]
fn test_adds_come_before_removes_and_an_item_is_not_added_twice() {
    let e = ProfileEdit {
        add_bundles: strings(&["x", "x", "demo"]),
        remove_bundles: strings(&["demo"]),
        ..edit()
    };
    let (text, changes) = edit_profile(BASE, &e).unwrap();
    assert_eq!(changes, ["added bundle: x", "removed bundle: demo"]);
    assert_eq!(
        crate::profile::Profile::decode(&text).unwrap().bundles,
        ["x"]
    );
}

#[test]
fn test_the_description_is_cleared_to_empty_and_is_then_a_change() {
    let (text, changes) = edit_profile(
        BASE,
        &ProfileEdit {
            description: Some(String::new()),
            ..edit()
        },
    )
    .unwrap();
    assert_eq!(changes, ["updated description"]);
    assert_eq!(
        crate::profile::Profile::decode(&text).unwrap().description,
        ""
    );
}

#[test]
fn test_the_authors_comments_and_order_survive() {
    let text = "# my profile\ndescription: mine # why\nbundles:\n  - demo # the base\n";
    let (out, _) = edit_profile(
        text,
        &ProfileEdit {
            add_bundles: strings(&["b2"]),
            description: Some("new".to_string()),
            ..edit()
        },
    )
    .unwrap();
    assert!(out.starts_with("# my profile\ndescription: new"), "{out}");
    assert!(out.contains("  - demo # the base\n  - b2\n"), "{out}");
}

#[test]
fn test_a_profile_that_does_not_parse_is_refused() {
    assert!(edit_profile(
        "bundles: [unclosed\n",
        &ProfileEdit {
            add_bundles: strings(&["x"]),
            ..edit()
        }
    )
    .is_err());
}
