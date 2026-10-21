//! A task's shape as a program reads it.

use std::path::PathBuf;

use presentation::write_json;
use serde_json::json;
use tasks_application::{Project, Reach, Row, Summary, TagCount, Task, Timestamp};

use super::{
    headline, project_label, rfc3339, spelled, CompactJson, RowJson, SummaryJson, TagCountJson,
    TaskJson,
};

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
    task.tags = vec![tag("a")];
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
  "created_at": "2026-06-10T02:35:20.837009562Z",
  "derived_priority": null
}
"#
    );
}

#[test]
fn test_a_derived_priority_is_written_as_go_writes_a_number() {
    let task = task("swift-amber", "To Do");
    let priority = |p: f64| {
        let mut out = Vec::new();
        write_json(&mut out, &TaskJson::of(&task).with_priority(Some(p))).unwrap();
        let text = String::from_utf8(out).unwrap();
        let line = text
            .lines()
            .find(|l| l.contains("derived_priority"))
            .unwrap();
        line.trim().to_string()
    };
    assert_eq!(priority(5.0), r#""derived_priority": 5"#);
    assert_eq!(priority(0.0), r#""derived_priority": 0"#);
    assert_eq!(
        priority(5.0 * 2.0 / 3.0),
        r#""derived_priority": 3.3333333333333335"#
    );
    assert_eq!(priority(1.25), r#""derived_priority": 1.25"#);
}

#[test]
fn test_no_tags_is_an_empty_array_never_null() {
    let text = json(&task("swift-amber", "Done"));
    assert!(text.contains("\"tags\": []"), "{text}");
    assert!(text.contains("\"checked\": true"), "{text}");
}

fn tag(key: &str) -> tasks_application::Tag {
    tasks_application::Tag {
        namespace: None,
        key: key.to_string(),
        value: None,
    }
}

fn spelled_tag(s: &str) -> tasks_application::Tag {
    tasks_application::parse_tag(s).unwrap().unwrap()
}

#[test]
fn test_each_tag_is_spelled_canonically() {
    let tags: Vec<_> = ["triage:cwe=79", "a:b", "x=\"y z\""]
        .iter()
        .map(|t| spelled_tag(t))
        .collect();
    assert_eq!(spelled(&tags), ["triage:cwe=79", "a:b", "x=\"y z\""]);
}

fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::from("/s/state.db"),
    }
}

fn value(shape: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(shape).unwrap()
}

#[test]
fn test_a_project_is_labelled_by_its_root_then_its_id() {
    assert_eq!(project_label(&project()), "/work/tree (jolly-mute-magma)");
}

#[test]
fn test_a_project_with_no_known_directory_is_labelled_by_its_id() {
    let mut project = project();
    project.root.clear();
    assert_eq!(project_label(&project), "jolly-mute-magma");
}

#[test]
fn test_a_headline_is_cut_at_80_characters() {
    let exactly = "x".repeat(80);
    assert_eq!(headline(&format!("{exactly}\nmore")), exactly);
    assert_eq!(headline(&"x".repeat(81)), format!("{exactly}\u{2026}"));
}

#[test]
fn test_a_compact_task_is_its_headline_with_its_project() {
    let mut task = task("swift-amber", "Done");
    task.text = "First line\nthe rest".to_string();
    task.tags = vec![spelled_tag("x=\"y z\"")];
    let project = project();
    assert_eq!(
        value(&RowJson::of(CompactJson::of(&task), &project)),
        json!({
            "harp_id": "swift-amber",
            "status": "Done",
            "checked": true,
            "tags": ["x=\"y z\""],
            "headline": "First line",
            "project_id": "jolly-mute-magma",
            "project_dir": "/work/tree",
        })
    );
}

#[test]
fn test_a_summary_is_its_counts_and_each_task_in_progress_by_its_project() {
    let summary = Summary {
        reach: Reach::Project(project()),
        counts: [("In Progress".to_string(), 1), ("To Do".to_string(), 2)].into(),
        in_progress: vec![Row {
            project: project(),
            task: task("swift-amber", "In Progress"),
        }],
    };
    assert_eq!(
        value(&SummaryJson::of(&summary)),
        json!({
            "counts": {"In Progress": 1, "To Do": 2},
            "in_progress": [{"harp_id": "swift-amber", "project": "/work/tree (jolly-mute-magma)"}],
        })
    );
}

#[test]
fn test_a_tag_in_use_is_spelled_canonically_with_its_counts() {
    let count = TagCount {
        tag: spelled_tag("triage:kind=defect"),
        active: 1,
        total: 3,
    };
    assert_eq!(
        value(&TagCountJson::of(&count)),
        json!({"tag": "triage:kind=defect", "active": 1, "total": 3})
    );
}
