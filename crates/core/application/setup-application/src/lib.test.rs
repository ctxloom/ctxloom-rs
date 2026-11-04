use super::fakes::*;

#[test]
fn test_a_relative_path_is_made_absolute_against_the_working_directory() {
    let w = world(CONFIG);
    assert_eq!(w.setup.absolute("out"), "/p/out");
    assert_eq!(w.setup.absolute("./a/../b/"), "/p/b");
    assert_eq!(w.setup.absolute("/x/y"), "/x/y");
    assert_eq!(w.setup.absolute("../../.."), "/");
}

#[test]
fn test_the_engines_composed_in_are_listed() {
    assert_eq!(
        world(CONFIG).setup.engine_names(),
        vec!["claude-code", "mock"]
    );
}
