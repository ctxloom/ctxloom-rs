//! The import report, as a person and as a program reads it.

use annals_diagnostic::Report;
use serde_json::{json, Value};
use tasks_application::{GoOp, ImportReport, Lost, ProjectImport, Why};

use super::{text, ImportJson};

fn catalog() -> presentation::Catalog {
    crate::catalog(&[]).expect("the catalog loads")
}

fn every_count() -> ProjectImport {
    ProjectImport {
        project_id: "jolly-mute-magma".to_string(),
        failed: None,
        records: 9,
        repeated: 1,
        tasks: 2,
        applied: 3,
        already: 4,
        unchanged: 5,
        ignored: 6,
        repaired: 7,
        lost: vec![
            Lost {
                line: 12,
                op: GoOp::Add,
                harp_id: "swift-amber".to_string(),
                text: "Ship it\nwith notes".to_string(),
                why: Why::Displaced,
            },
            Lost {
                line: 13,
                op: GoOp::Status,
                harp_id: "calm-otter".to_string(),
                text: String::new(),
                why: Why::Refused(Report::new("tasks-trigger-required", "needs a trigger")),
            },
        ],
    }
}

fn texted(report: &ImportReport) -> String {
    let mut out = Vec::new();
    text(&mut out, report, &catalog()).unwrap();
    String::from_utf8(out).unwrap()
}

/// The record a rejection left behind gives the catalog's message, not the
/// English it carried; a code no catalog has keeps its English.
#[test]
fn test_text_names_every_count_and_every_record_left_behind() {
    let mut stopped = every_count();
    stopped.failed = Some(Report::new("x-bad", "the store failed"));
    let report = ImportReport {
        projects: vec![stopped],
    };
    assert_eq!(
        texted(&report),
        "jolly-mute-magma: 9 records, 2 tasks added, 3 records imported, \
         4 already imported, 1 repeated lines skipped, 5 retags changing nothing, \
         6 to no live task, as Go ignores them, 7 displaced adds Go's repair re-added, \
         2 not imported\n  \
         not imported: line 12, add swift-amber \"Ship it\": another task holds this harp in Go's log: run Go's `taskloom repair`, then import again\n  \
         not imported: line 13, status calm-otter: a Deferred task needs a trigger: the condition that should revive it\n  \
         stopped: the store failed\n"
    );
}

#[test]
fn test_text_leaves_out_the_counts_that_are_zero() {
    let report = ImportReport {
        projects: vec![
            ProjectImport {
                project_id: "able-calm-fox".to_string(),
                ..ProjectImport::default()
            },
            ProjectImport {
                project_id: "jolly-mute-magma".to_string(),
                records: 1,
                tasks: 1,
                applied: 1,
                ..ProjectImport::default()
            },
        ],
    };
    assert_eq!(
        texted(&report),
        "able-calm-fox: 0 records, 0 tasks added, 0 records imported\n\
         jolly-mute-magma: 1 records, 1 tasks added, 1 records imported\n"
    );
}

#[test]
fn test_json_carries_the_whole_report() {
    let mut stopped = every_count();
    stopped.failed = Some(Report::new("x-bad", "the store failed"));
    let report = ImportReport {
        projects: vec![stopped],
    };
    let json: Value = serde_json::to_value(ImportJson::of(&report, &catalog())).unwrap();
    assert_eq!(
        json,
        json!({
            "complete": false,
            "projects": [{
                "project_id": "jolly-mute-magma",
                "failed": {"code": "x-bad", "args": {}, "message": "the store failed"},
                "records": 9,
                "repeated": 1,
                "tasks": 2,
                "applied": 3,
                "already": 4,
                "unchanged": 5,
                "ignored": 6,
                "repaired": 7,
                "lost": [
                    {
                        "line": 12,
                        "op": "add",
                        "harp_id": "swift-amber",
                        "text": "Ship it\nwith notes",
                        "why": {
                            "code": "tasks-application-import-displaced",
                            "args": {},
                            "message": "another task holds this harp in Go's log: run Go's `taskloom repair`, then import again"
                        }
                    },
                    {
                        "line": 13,
                        "op": "status",
                        "harp_id": "calm-otter",
                        "text": "",
                        "why": {"code": "tasks-trigger-required", "args": {}, "message": "a Deferred task needs a trigger: the condition that should revive it"}
                    }
                ]
            }]
        })
    );
}

#[test]
fn test_json_of_a_complete_import_says_so() {
    let report = ImportReport {
        projects: vec![ProjectImport::default()],
    };
    let json: Value = serde_json::to_value(ImportJson::of(&report, &catalog())).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["projects"][0]["failed"], Value::Null);
}
