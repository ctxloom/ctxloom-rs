use super::*;

fn command(name: &str, description: &str) -> Command {
    Command {
        name: name.into(),
        description: description.into(),
        body: "BODY".into(),
    }
}

#[test]
fn test_a_command_with_a_description_gets_frontmatter() {
    assert_eq!(
        render(&command("a/b", "Does a thing")),
        "---\ndescription: Does a thing\n---\n\nBODY"
    );
    assert_eq!(render(&command("a/b", "")), "BODY");
}

#[test]
fn test_a_description_that_would_not_read_back_is_quoted() {
    for (text, quoted) in [
        ("a: b", "\"a: b\""),
        ("# x", "\"# x\""),
        (" lead", "\" lead\""),
        ("say \"hi\"", "\"say \\\"hi\\\"\""),
        ("two\nlines", "\"two\\nlines\""),
        ("- dash", "\"- dash\""),
    ] {
        assert_eq!(scalar(text), quoted);
    }
    assert_eq!(scalar("plain words"), "plain words");
}

#[test]
fn test_a_nested_name_becomes_one_file_with_dashes() {
    let mut planned = Planned::default();
    plan(&[command("team/onboarding", "d")], &mut planned);
    assert_eq!(planned.claims.len(), 1);
    assert_eq!(
        planned.claims[0].path(),
        ".claude/commands/team-onboarding.md"
    );
}

#[test]
fn test_a_name_that_climbs_is_skipped_and_said() {
    for bad in ["", "../x", "/abs", "a/../b", "a//b", "a\\b"] {
        let mut planned = Planned::default();
        plan(&[command(bad, "d")], &mut planned);
        assert!(planned.claims.is_empty(), "{bad:?}");
        assert_eq!(planned.skipped.len(), 1, "{bad:?}");
    }
}
