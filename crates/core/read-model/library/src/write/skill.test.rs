use super::*;

#[test]
fn test_the_package_and_its_sidecar_are_in_the_skills_directory() {
    assert_eq!(package_dir("reviewer"), "skills/reviewer");
    assert_eq!(sidecar_path("reviewer"), "skills/.reviewer.meta.yaml");
}

// captured from a Go run: skill create
#[test]
fn test_a_new_skill_md_is_the_bytes_go_writes() {
    assert_eq!(
        skill_md("reviewer", "SKILL-MARKER-reviewer-9f3c21"),
        "---\nname: reviewer\ndescription: SKILL-MARKER-reviewer-9f3c21\n---\n\n# reviewer\n\nTODO: describe what this skill does and how to use it.\n"
    );
    // The default description has a colon in it, so it is quoted.
    let md = skill_md("nodesc", DEFAULT_DESCRIPTION);
    assert!(md.starts_with("---\nname: nodesc\ndescription: \"TODO: describe when an agent should use this skill.\"\n---\n\n# nodesc\n"), "{md}");
    // It reads: the frontmatter holds the name and the description.
    let split = crate::frontmatter::split_front_matter(&md).expect("front matter");
    let meta = angzarr_yaml::parse(split.meta).unwrap();
    assert_eq!(
        meta.get("name").and_then(angzarr_yaml::Value::as_str),
        Some("nodesc")
    );
    assert_eq!(
        meta.get("description")
            .and_then(angzarr_yaml::Value::as_str),
        Some(DEFAULT_DESCRIPTION)
    );
}

#[test]
fn test_the_executable_files_are_what_the_sidecar_lists() {
    assert_eq!(executable_files(None), Vec::<String>::new());
    assert_eq!(executable_files(Some("")), Vec::<String>::new());
    assert_eq!(executable_files(Some("notes: x\n")), Vec::<String>::new());
    assert_eq!(
        executable_files(Some("executable:\n  - scripts/run.sh\n  - bin/x\n")),
        ["scripts/run.sh", "bin/x"]
    );
    // A sidecar that is no YAML declares nothing here (reading refuses it).
    assert_eq!(
        executable_files(Some("executable: [unclosed\n")),
        Vec::<String>::new()
    );
}

#[test]
fn test_setting_the_executables_keeps_every_other_key() {
    let files = vec!["scripts/run.sh".to_string(), "bin/a".to_string()];
    assert_eq!(
        with_executable(None, &files).unwrap().as_deref(),
        Some("executable:\n  - bin/a\n  - scripts/run.sh\n")
    );
    assert_eq!(
        with_executable(Some("tags:\n  - t\nnotes: n # mine\n"), &files)
            .unwrap()
            .as_deref(),
        Some("tags:\n  - t\nnotes: n # mine\nexecutable:\n  - bin/a\n  - scripts/run.sh\n")
    );
    assert_eq!(
        with_executable(Some("executable:\n  - old\nnotes: n\n"), &files)
            .unwrap()
            .as_deref(),
        Some("executable:\n  - bin/a\n  - scripts/run.sh\nnotes: n\n")
    );
}

#[test]
fn test_an_empty_list_takes_the_key_and_the_sidecar_with_it() {
    assert_eq!(with_executable(None, &[]).unwrap(), None);
    assert_eq!(
        with_executable(Some("executable:\n  - a\n"), &[]).unwrap(),
        None
    );
    assert_eq!(
        with_executable(Some("notes: n\nexecutable:\n  - a\n"), &[])
            .unwrap()
            .as_deref(),
        Some("notes: n\n")
    );
    assert!(with_executable(Some("executable: [unclosed\n"), &["a".to_string()]).is_err());
}
