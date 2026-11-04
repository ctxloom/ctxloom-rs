//! A plain test from plan 8.6 (the features run in `gherkin_surfaces.test.rs`).

use super::library::Project;

// Go's fsstatic suite pins the ownership record's promise for a hand-authored
// settings file: what the user wrote comes back as it was once ctxloom's
// entries are released (manage.feature: "Materializing preserves the exact
// numbers" and "Uninstall strips the harness" together).
#[test]
fn test_releasing_a_materialize_over_a_hand_written_settings_file_gives_it_back_byte_for_byte() {
    let project = Project::initialized();
    let theirs = "{\n  \"permissions\": { \"allow\": [\"Read\"] }\n}\n";
    project.write(".claude/settings.json", theirs);
    project.run(&["materialize", "--yes"]).succeeds();
    assert_ne!(
        project.read(".claude/settings.json").as_deref(),
        Some(theirs)
    );
    project
        .run(&["materialize", "--release", "--yes"])
        .succeeds();
    assert_eq!(
        project.read(".claude/settings.json").as_deref(),
        Some(theirs)
    );
}
