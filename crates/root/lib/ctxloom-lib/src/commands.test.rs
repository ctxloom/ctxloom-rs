//! The program's nouns: the library's, with the publishing verbs added to
//! `bundle`, and the remote and dependency ones.

use super::*;

fn verbs(noun: &Command) -> Vec<String> {
    noun.get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect()
}

#[test]
fn test_the_nouns_are_the_librarys_and_the_dependency_ones() {
    let nouns = Commands::new().nouns();
    let names: Vec<&str> = nouns.iter().map(Command::get_name).collect();
    for noun in ["bundle", "fragment", "profile", "search", "remote", "deps"] {
        assert!(names.contains(&noun), "{noun} in {names:?}");
    }
    assert_eq!(names.iter().filter(|n| **n == "bundle").count(), 1);
}

#[test]
fn test_only_the_bundle_noun_gains_the_publishing_verbs() {
    let nouns = Commands::new().nouns();
    let find = |name: &str| nouns.iter().find(|n| n.get_name() == name).expect(name);
    let bundle = verbs(find("bundle"));
    assert!(bundle.contains(&"push".to_string()) && bundle.contains(&"move".to_string()));
    assert!(
        bundle.contains(&"list".to_string()),
        "the library's verbs stay: {bundle:?}"
    );
    for other in ["fragment", "profile"] {
        let verbs = verbs(find(other));
        assert!(
            !verbs.contains(&"push".to_string()) && !verbs.contains(&"move".to_string()),
            "{other}: {verbs:?}"
        );
    }
}
