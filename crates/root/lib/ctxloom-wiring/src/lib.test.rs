use harp_core::{ADJECTIVES, NOUNS};

use super::{run_id, session_name};

const SESSION: &str = "0197a5c0-1b6e-7a41-8c3d-5e2f4a9b7d10";

#[test]
fn test_a_session_name_is_two_adjectives_and_a_noun() {
    for _ in 0..20 {
        let name = session_name();
        let words: Vec<&str> = name.split('-').collect();
        assert_eq!(words.len(), 3, "{name}");
        assert!(ADJECTIVES.contains(&words[0]) && ADJECTIVES.contains(&words[1]));
        assert!(NOUNS.contains(&words[2]));
    }
}

#[test]
fn test_a_run_id_is_uuid_v5_of_the_session_and_the_incarnation() {
    let id = run_id(SESSION, 1).unwrap();
    assert_eq!(id.get_version_num(), 5);
    assert_eq!(run_id(SESSION, 1).unwrap(), id);
    assert_ne!(run_id(SESSION, 2).unwrap(), id);
}

#[test]
fn test_a_session_that_is_no_uuid_has_no_run() {
    assert!(run_id("jolly-mute-magma", 1).is_err());
}
