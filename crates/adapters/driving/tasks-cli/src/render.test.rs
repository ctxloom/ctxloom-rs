//! The shapes answers are rendered in.

use std::path::PathBuf;

use tasks_application::{Project, Task, Timestamp};

use super::{headline, json, project_label, rfc3339, table, TaskJson};

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

fn rendered(write: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut out = Vec::new();
    write(&mut out).unwrap();
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
    let text = rendered(|out| json(out, &TaskJson::of(&task)));
    assert_eq!(
        text,
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
    let text = rendered(|out| json(out, &TaskJson::of(&task("swift-amber", "Done"))));
    assert!(text.contains("\"tags\": []"), "{text}");
    assert!(text.contains("\"checked\": true"), "{text}");
}

#[test]
fn test_a_project_is_labelled_by_its_root_then_its_id() {
    let project = Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::new(),
    };
    assert_eq!(project_label(&project), "/work/tree (jolly-mute-magma)");
}

#[test]
fn test_a_headline_is_the_first_line_trimmed_on_the_right() {
    assert_eq!(headline("Ship it   \nthe details"), "Ship it");
    assert_eq!(headline(""), "");
    let exactly = "x".repeat(80);
    assert_eq!(headline(&exactly), exactly);
}

#[test]
fn test_a_long_headline_is_cut_at_80_characters_with_an_ellipsis() {
    let long = format!("{} {}", "a".repeat(79), "b".repeat(10));
    assert_eq!(
        headline(&long),
        format!("{}\u{2026}", "a".repeat(79)),
        "the cut's trailing space trimmed"
    );
    let wide = "\u{e9}".repeat(81);
    assert_eq!(
        headline(&wide),
        format!("{}\u{2026}", "\u{e9}".repeat(80)),
        "characters, not bytes"
    );
}

#[test]
fn test_an_empty_table_says_so() {
    assert_eq!(rendered(|out| table(out, &[])), "(no tasks)\n");
}

#[test]
fn test_a_table_aligns_ids_and_statuses_and_separates_tasks() {
    let mut deferred = task("calm-otter", "Deferred");
    deferred.trigger = "the release".to_string();
    deferred.tags = vec!["a".to_string(), "b".to_string()];
    let tasks = [task("swift-amber-x", "Done"), deferred];
    assert_eq!(
        rendered(|out| table(out, &tasks)),
        "[x] swift-amber-x  Done         Ship it\n\
         \n\
         [ ] calm-otter     Deferred     Ship it  (trigger: the release)  [a, b]\n"
    );
}
