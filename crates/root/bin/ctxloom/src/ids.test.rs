//! Ids and the store: `Names` produce harp ids of the right shapes (a
//! session `adj-adj-noun`; a run id UUIDv5(session, incarnation), D5 Q4), and
//! `Stores::open` opens `~/.ctxloom/projects/<id>/ctxloom.db` under a temp
//! home, never the real one (D26).
//!
//! These are the application ports' tests in the plan. They are reached
//! through the binary's test-only commands because the ports crate
//! (`ports`) cannot be made beside the taskloom fork's own `ports` until S0
//! deletes it, and `Rooted` is in the file-system crate S0 has not pinned.

use std::collections::BTreeSet;

use harp_core::{ADJECTIVES, NOUNS};

use super::{run, Env};
use crate::tests::files;

const SESSION: &str = "0197a5c0-1b6e-7a41-8c3d-5e2f4a9b7d10";
const OTHER_SESSION: &str = "0197a5c0-1b6e-7a41-8c3d-5e2f4a9b7d11";

fn minted_session() -> String {
    let ran = run(&["test", "session-id"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.stderr, "");
    ran.stdout.trim_end().to_string()
}

#[test]
fn test_a_session_id_is_two_adjectives_and_a_noun() {
    for _ in 0..5 {
        let name = minted_session();
        let words: Vec<&str> = name.split('-').collect();
        assert_eq!(words.len(), 3, "{name}");
        for word in &words[..2] {
            assert!(ADJECTIVES.contains(word), "{word} in {name}");
        }
        assert!(NOUNS.contains(&words[2]), "{name} ends in a noun");
    }
}

#[test]
fn test_session_ids_are_drawn_at_random() {
    let drawn: BTreeSet<String> = (0..8).map(|_| minted_session()).collect();
    assert!(drawn.len() > 1, "{drawn:?}");
}

fn run_id(session: &str, incarnation: &str) -> String {
    let ran = run(&["test", "run-id", session, incarnation]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    assert_eq!(ran.stderr, "");
    ran.stdout.trim_end().to_string()
}

#[test]
fn test_a_run_id_is_a_uuid_of_version_5() {
    let id = run_id(SESSION, "1");
    let groups: Vec<usize> = id.split('-').map(str::len).collect();
    assert_eq!(groups, [8, 4, 4, 4, 12], "{id}");
    assert!(
        id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()),
        "{id}"
    );
    assert_eq!(id, id.to_lowercase());
    assert_eq!(id.as_bytes()[14], b'5', "version 5: {id}");
    assert!(
        matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
        "RFC 4122 variant: {id}"
    );
}

#[test]
fn test_a_run_id_is_a_function_of_its_session_and_incarnation() {
    let first = run_id(SESSION, "1");
    assert_eq!(run_id(SESSION, "1"), first, "the same inputs, the same id");
    assert_ne!(run_id(SESSION, "2"), first, "another incarnation");
    assert_ne!(run_id(OTHER_SESSION, "1"), first, "another session");
}

#[test]
fn test_the_store_opens_under_the_home_s_projects_directory() {
    let env = Env::new();
    let project = format!("s0-store-{}", std::process::id());
    let ran = env.run(&["test", "open-store", &project]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    let db = env
        .home()
        .join(".ctxloom/projects")
        .join(&project)
        .join("ctxloom.db");
    assert_eq!(
        std::path::Path::new(ran.stdout.trim_end()),
        db,
        "it says where: {}",
        ran.said()
    );
    let bytes = files::read_bytes(&db).unwrap_or_else(|e| panic!("{}: {e}", db.display()));
    assert!(bytes.starts_with(b"SQLite format 3\0"), "a SQLite database");
    let beside = env.home().join(".ctxloom/projects").join(&project);
    assert!(
        !files::exists(beside.join("state.db")),
        "taskloom's store is not ctxloom's (D26)"
    );
}

#[test]
fn test_the_store_never_opens_under_the_real_home() {
    let project = format!("s0-real-home-{}", std::process::id());
    let env = Env::new();
    let ran = env.run(&["test", "open-store", &project]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    let made = env.home().join(".ctxloom/projects").join(&project);
    assert!(
        files::exists(made.join("ctxloom.db")),
        "made under the temp home"
    );
    if let Some(real) = crate::tests::host_var("HOME") {
        let there = std::path::Path::new(&real)
            .join(".ctxloom/projects")
            .join(&project);
        assert!(!files::exists(&there), "{} was made", there.display());
    }
}
