//! Plain tests for what manage.feature does not cover (the features run in
//! `gherkin_surfaces.test.rs`).

use super::library::Project;

// manage.go's `--print`: the steps install would take, taking none.
#[test]
fn test_install_print_says_what_it_would_do_and_does_nothing() {
    let project = Project::bare();
    let ran = project.run(&["manage", "install", "--engine", "mock", "--print"]);
    ran.succeeds();
    let doc = ran.json();
    assert_eq!(doc["ctxloom_dir_exists"], false);
    assert_eq!(doc["engine"], "mock");
    project.file_is_absent(".ctxloom/config.yaml");
    project.file_is_absent(".ctxloom/.gitignore");
}
