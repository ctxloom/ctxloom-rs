//! A task's read-time values.

use annals::next::Timestamp;
use tasks::TaskState;

use super::{is_terminal, text_hash, Task};

#[test]
fn test_done_and_archived_are_terminal_and_nothing_else_is() {
    assert!(is_terminal("Done"));
    assert!(is_terminal("Archived"));
    for open in ["To Do", "In Progress", "Deferred", "Blocked", ""] {
        assert!(!is_terminal(open), "{open}");
    }
}

#[test]
fn test_the_text_hash_is_the_head_of_the_trimmed_texts_sha256() {
    // sha256("hello") = 2cf24dba5fb0a30e...
    assert_eq!(text_hash("hello"), "2cf24dba5fb0");
    assert_eq!(text_hash("  hello\n"), "2cf24dba5fb0", "trimmed first");
}

#[test]
fn test_a_task_carries_its_state_and_the_values_derived_from_it() {
    let at = Timestamp {
        seconds: 1_700_000_000,
        nanos: 5,
    };
    let state = TaskState {
        harp_id: "swift-amber".to_string(),
        text: "hello".to_string(),
        status: "Done".to_string(),
        trigger: "the release".to_string(),
        origin_session: "calm-otter-fox".to_string(),
        tags: vec![tag("a"), tag("b")],
        created_at: at,
        ..TaskState::default()
    };
    assert_eq!(
        Task::of(&state),
        Task {
            harp_id: "swift-amber".to_string(),
            text: "hello".to_string(),
            status: "Done".to_string(),
            checked: true,
            text_hash: "2cf24dba5fb0".to_string(),
            trigger: "the release".to_string(),
            origin_session: "calm-otter-fox".to_string(),
            tags: vec![tag("a"), tag("b")],
            created_at: at,
        }
    );
}

#[test]
fn test_an_open_task_is_not_checked() {
    let state = TaskState {
        status: "To Do".to_string(),
        ..TaskState::default()
    };
    assert!(!Task::of(&state).checked);
}

fn tag(key: &str) -> tasks::Tag {
    tasks::Tag {
        namespace: None,
        key: key.to_string(),
        value: None,
    }
}

// ------------------------------------------------------------ shown

fn spelled(s: &str) -> tasks::Tag {
    crate::tags::parse(s).unwrap().unwrap()
}

#[test]
fn test_tagma_tags_are_hidden_by_default_and_the_rest_shown() {
    let shown = super::Shown::default();
    assert!(!shown.shows(&spelled("tagma:x")));
    assert!(!shown.shows(&spelled("tagma.hide:x=true")));
    assert!(shown.shows(&spelled("urgent")));
    assert!(shown.shows(&spelled("tagmatic:x")));
}

#[test]
fn test_the_schemas_hide_facts_decide_what_is_shown() {
    let schema = crate::TagSchema::parse([
        r#"tagma.hide:"triage:cwe"=true"#,
        r#"tagma.hide:"tagma:*"=false"#,
    ])
    .unwrap();
    let shown = super::Shown::of(&schema);
    let tags = [
        spelled("triage:cwe=79"),
        spelled("triage:kind=chore"),
        spelled("tagma:x"),
    ];
    assert_eq!(shown.shown(&tags), [&tags[1], &tags[2]]);
}
