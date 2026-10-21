//! The shapes answers are rendered in.

use std::path::PathBuf;

use tasks_application::{Project, Task, Timestamp};

use presentation::{render, write_json, Format};

use super::{headline, project_label, rfc3339, table, Added, Listed, Tagged, TaskJson};

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
    let text = rendered(|out| write_json(out, &TaskJson::of(&task)));
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
    let text = rendered(|out| write_json(out, &TaskJson::of(&task("swift-amber", "Done"))));
    assert!(text.contains("\"tags\": []"), "{text}");
    assert!(text.contains("\"checked\": true"), "{text}");
}

#[test]
fn test_a_project_is_labelled_by_its_root_then_its_id() {
    assert_eq!(project_label(&project()), "/work/tree (jolly-mute-magma)");
}

#[test]
fn test_a_headline_is_cut_at_80_characters() {
    let exactly = "x".repeat(80);
    assert_eq!(headline(&format!("{exactly}\nmore")), exactly);
    assert_eq!(headline(&"x".repeat(81)), format!("{exactly}\u{2026}"));
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

#[test]
fn test_a_table_aligns_wide_characters_by_their_display_width() {
    let tasks = [
        task("swift-amber", "\u{5b8c}\u{4e86}"),
        task("calm-otter", "To Do"),
    ];
    assert_eq!(
        rendered(|out| table(out, &tasks)),
        "[ ] swift-amber  \u{5b8c}\u{4e86}         Ship it\n\
         \n\
         [ ] calm-otter   To Do        Ship it\n"
    );
}

fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::new(),
    }
}

#[test]
fn test_a_written_task_is_one_line_of_text_or_the_task_in_json() {
    let mut tagged = task("swift-amber", "To Do");
    tagged.tags = vec!["a".to_string(), "b".to_string()];
    assert_eq!(
        rendered(|out| render(out, Format::Text, &Added(&tagged))),
        "swift-amber\tTo Do\tShip it\n"
    );
    assert_eq!(
        rendered(|out| render(out, Format::Text, &Tagged(&tagged))),
        "swift-amber\tTo Do\ta,b\n"
    );
    let json = rendered(|out| write_json(out, &TaskJson::of(&tagged)));
    for view in [
        rendered(|out| render(out, Format::Json, &Added(&tagged))),
        rendered(|out| render(out, Format::Json, &Tagged(&tagged))),
    ] {
        assert_eq!(view, json);
    }
}

#[test]
fn test_a_listing_names_its_project_in_text_and_is_an_array_in_json() {
    let project = project();
    let tasks = [task("swift-amber", "To Do")];
    let listed = Listed {
        project: Some(&project),
        tasks: &tasks,
    };
    assert_eq!(
        rendered(|out| render(out, Format::Text, &listed)),
        "Project: /work/tree (jolly-mute-magma)\n\n[ ] swift-amber  To Do        Ship it\n"
    );
    let json: serde_json::Value =
        serde_json::from_str(&rendered(|out| render(out, Format::Json, &listed))).unwrap();
    assert_eq!(json[0]["harp_id"], "swift-amber");
    let nowhere = Listed {
        project: None,
        tasks: &[],
    };
    assert_eq!(
        rendered(|out| render(out, Format::Text, &nowhere)),
        "(no tasks)\n"
    );
    assert_eq!(rendered(|out| render(out, Format::Json, &nowhere)), "[]\n");
}
