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
        tags: vec!["a".to_string(), "b".to_string()],
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
            tags: vec!["a".to_string(), "b".to_string()],
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
