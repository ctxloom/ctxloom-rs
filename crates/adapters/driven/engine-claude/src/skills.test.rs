use surface_ports::SkillFile;

use super::*;

fn skill(name: &str, description: &str) -> Skill {
    Skill {
        name: name.into(),
        description: description.into(),
        files: vec![
            SkillFile {
                path: "SKILL.md".into(),
                bytes: b"x".to_vec(),
                executable: false,
            },
            SkillFile {
                path: "scripts/run.sh".into(),
                bytes: b"y".to_vec(),
                executable: true,
            },
            SkillFile {
                path: "../escape".into(),
                bytes: b"z".to_vec(),
                executable: false,
            },
        ],
    }
}

#[test]
fn test_a_skill_package_is_written_under_its_name_with_its_exec_bits() {
    let mut planned = Planned::default();
    plan(&[skill("ctxloom-doctor", "d")], &mut planned);
    let paths: Vec<&str> = planned.claims.iter().map(Claim::path).collect();
    assert_eq!(
        paths,
        vec![
            ".claude/skills/ctxloom-doctor/SKILL.md",
            ".claude/skills/ctxloom-doctor/scripts/run.sh"
        ]
    );
    assert!(matches!(
        &planned.claims[1],
        Claim::Whole {
            executable: true,
            ..
        }
    ));
}

#[test]
fn test_a_skill_that_breaks_the_standard_is_refused_by_name_and_constraint() {
    let long_name = "a".repeat(65);
    let long_description = "d".repeat(1025);
    for (name, description, fragment) in [
        ("", "d", "missing the required `name`"),
        (long_name.as_str(), "d", "exceeds the 64 char limit"),
        ("Upper", "d", "lowercase alphanumerics and hyphens"),
        ("a--b", "d", "lowercase alphanumerics and hyphens"),
        ("-a", "d", "lowercase alphanumerics and hyphens"),
        ("my-claude-skill", "d", "reserved word \"claude\""),
        ("anthropic-x", "d", "reserved word \"anthropic\""),
        ("ok", "", "missing the required `description`"),
        (
            "ok",
            long_description.as_str(),
            "exceeds the 1024 char limit",
        ),
    ] {
        let mut planned = Planned::default();
        plan(&[skill(name, description)], &mut planned);
        assert!(planned.claims.is_empty(), "{name:?}");
        assert_eq!(planned.skipped.len(), 1, "{name:?}");
        assert!(
            planned.skipped[0].contains(fragment),
            "{:?}",
            planned.skipped
        );
    }
}

#[test]
fn test_a_name_of_digits_and_hyphens_is_a_name() {
    let mut planned = Planned::default();
    plan(&[skill("a1-b2", "d")], &mut planned);
    assert!(planned.skipped.is_empty());
}

#[test]
fn test_a_name_or_description_at_its_limit_is_accepted_and_one_over_is_refused() {
    let ok = "a".repeat(64);
    let desc = "d".repeat(1024);
    assert_eq!(violation(&skill(&ok, &desc)), None);
    let long_name = "a".repeat(65);
    assert!(violation(&skill(&long_name, &desc))
        .unwrap()
        .contains("65 chars"));
    let long_desc = "d".repeat(1025);
    assert!(violation(&skill(&ok, &long_desc))
        .unwrap()
        .contains("1025 chars"));
}

#[test]
fn test_a_package_file_is_inside_only_when_its_path_stays_in_the_package() {
    for good in ["SKILL.md", "a/b.sh", "..a", "a.b"] {
        assert!(inside(good), "{good}");
    }
    for bad in [
        "", "/a", "a\\b", "a//b", "a/", "./a", "a/./b", "../a", "a/../b", "a/..",
    ] {
        assert!(!inside(bad), "{bad}");
    }
}
