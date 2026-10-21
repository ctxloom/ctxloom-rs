//! A task's shape as a program reads it.

use presentation::write_json;
use tasks_application::{Task, Timestamp};

use super::{rfc3339, TaskJson};

fn task(harp: &str, status: &str) -> Task {
    Task {
        harp_id: harp.to_string(),
        text: "Ship it".to_string(),
        status: status.to_string(),
        checked: status == "Done",
        text_hash: "0123456789ab".to_string(),
        trigger: String::new(),
        origin_session: String::new(),
        tags: Vec::new(),
        created_at: Timestamp {
            seconds: 1_781_058_920,
            nanos: 837_009_562,
        },
    }
}

fn json(task: &Task) -> String {
    let mut out = Vec::new();
    write_json(&mut out, &TaskJson::of(task)).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn test_an_instant_is_rfc3339_in_utc_with_only_the_digits_it_needs() {
    let at = |seconds, nanos| rfc3339(Timestamp { seconds, nanos });
    assert_eq!(
        at(1_781_058_920, 837_009_562),
        "2026-06-10T02:35:20.837009562Z"
    );
    assert_eq!(at(1_781_058_920, 500_000_000), "2026-06-10T02:35:20.5Z");
    assert_eq!(at(1_781_058_920, 0), "2026-06-10T02:35:20Z");
}

#[test]
fn test_a_task_as_json_is_gos_shape_in_gos_order() {
    let mut task = task("swift-amber", "To Do");
    task.tags = vec!["a".to_string()];
    task.origin_session = "calm-otter-fox".to_string();
    task.trigger = "later".to_string();
    assert_eq!(
        json(&task),
        r#"{
  "harp_id": "swift-amber",
  "text": "Ship it",
  "status": "To Do",
  "checked": false,
  "text_hash": "0123456789ab",
  "trigger": "later",
  "origin_session": "calm-otter-fox",
  "tags": [
    "a"
  ],
  "created_at": "2026-06-10T02:35:20.837009562Z"
}
"#
    );
}

#[test]
fn test_no_tags_is_an_empty_array_never_null() {
    let text = json(&task("swift-amber", "Done"));
    assert!(text.contains("\"tags\": []"), "{text}");
    assert!(text.contains("\"checked\": true"), "{text}");
}
