//! The names' shape: so many words, from the right lists.

use harp_core::{ADJECTIVES, NOUNS};
use ports::Names;

use super::HarpNames;

/// `name`'s words, each checked against the list its place draws from.
fn assert_words(name: &str, adjectives: usize) {
    let words: Vec<&str> = name.split('-').collect();
    assert_eq!(words.len(), adjectives + 1, "{name}");
    for word in &words[..adjectives] {
        assert!(
            ADJECTIVES.contains(word),
            "{word} in {name} is an adjective"
        );
    }
    assert!(NOUNS.contains(&words[adjectives]), "{name} ends in a noun");
}

#[test]
fn test_a_task_id_is_an_adjective_and_a_noun() {
    for _ in 0..50 {
        assert_words(&HarpNames.task_id(), 1);
    }
}

#[test]
fn test_a_project_id_is_two_adjectives_and_a_noun() {
    for _ in 0..50 {
        assert_words(&HarpNames.project_id(), 2);
    }
}

#[test]
fn test_names_are_drawn_at_random() {
    let drawn: std::collections::BTreeSet<String> = (0..20).map(|_| HarpNames.task_id()).collect();
    assert!(drawn.len() > 1, "{drawn:?}");
}
