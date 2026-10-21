//! Go's records read from a real directory, decoded as Go decodes them.

use std::fs;

use annals::next::Timestamp;
use annals_diagnostic::Diagnostic;
use ports::{GoOp, GoRecord, GoTaskLogs};
use tempfile::TempDir;

use crate::{parse, GoTaskLogDir, LogError, GO_ZERO_TIME};

const ADD: &str = r#"{"op":"add","task":"swift-amber","text":"Write the tour","status":"To Do","trigger":"t","session":"calm-otter","repair_of":"abc","tags":["b","a"],"ts":"2026-04-01T12:30:45.123456789Z"}"#;

fn logs(files: &[(&str, &str)]) -> (TempDir, GoTaskLogDir) {
    let home = TempDir::new().unwrap();
    let dir = home.path().join(".ctxloom").join("tasks");
    fs::create_dir_all(&dir).unwrap();
    for (name, text) in files {
        fs::write(dir.join(name), text).unwrap();
    }
    let logs = GoTaskLogDir::in_home(Some(home.path()));
    (home, logs)
}

fn only(text: &str) -> GoRecord {
    let mut records = parse("p", text).unwrap();
    assert_eq!(records.len(), 1);
    records.remove(0)
}

#[test]
fn test_a_record_carries_every_field_go_writes() {
    assert_eq!(
        only(ADD),
        GoRecord {
            line: 1,
            raw: ADD.to_string(),
            op: GoOp::Add,
            task: "swift-amber".to_string(),
            text: "Write the tour".to_string(),
            status: "To Do".to_string(),
            trigger: "t".to_string(),
            session: "calm-otter".to_string(),
            repair_of: "abc".to_string(),
            tags: vec!["b".to_string(), "a".to_string()],
            ts: Timestamp {
                seconds: 1_775_046_645,
                nanos: 123_456_789,
            },
        }
    );
}

#[test]
fn test_absent_and_null_fields_are_empty_and_unknown_ones_ignored() {
    let record = only(r#"{"op":"status","task":"swift-amber","text":null,"tags":null,"extra":7}"#);
    assert_eq!(record.text, "");
    assert_eq!(record.status, "");
    assert!(record.tags.is_empty());
    assert_eq!(
        record.ts,
        Timestamp {
            seconds: GO_ZERO_TIME,
            nanos: 0
        },
        "no ts is Go's zero time"
    );
}

#[test]
fn test_go_zero_time_is_year_one() {
    let at = time::OffsetDateTime::from_unix_timestamp(GO_ZERO_TIME).unwrap();
    assert_eq!((at.year(), u8::from(at.month()), at.day()), (1, 1, 1));
}

#[test]
fn test_an_offset_instant_is_the_same_instant_in_utc() {
    let record = only(r#"{"op":"remove","task":"x","ts":"2026-04-01T14:30:45+02:00"}"#);
    assert_eq!(
        record.ts,
        Timestamp {
            seconds: 1_775_046_645,
            nanos: 0
        }
    );
}

#[test]
fn test_every_op_go_knows_is_read() {
    let ops: Vec<GoOp> = ["add", "status", "text", "remove", "tag", "untag"]
        .iter()
        .map(|op| only(&format!(r#"{{"op":"{op}","task":"x"}}"#)).op)
        .collect();
    assert_eq!(
        ops,
        [
            GoOp::Add,
            GoOp::Status,
            GoOp::Text,
            GoOp::Remove,
            GoOp::Tag,
            GoOp::Untag
        ]
    );
}

#[test]
fn test_lines_are_trimmed_numbered_from_one_and_blank_ones_skipped() {
    let text = format!(
        "\n  {ADD}  \n\n{}\n",
        r#"{"op":"remove","task":"swift-amber"}"#
    );
    let records = parse("p", &text).unwrap();
    let lines: Vec<(u32, &str)> = records.iter().map(|r| (r.line, r.raw.as_str())).collect();
    assert_eq!(
        lines,
        [(2, ADD), (4, r#"{"op":"remove","task":"swift-amber"}"#)]
    );
}

#[test]
fn test_a_malformed_line_refuses_the_log_naming_where() {
    let text = format!("{ADD}\n{{\"op\":\"add\"\n");
    let Err(LogError::Malformed { path, line, .. }) = parse("/l.jsonl", &text) else {
        panic!("refused as malformed");
    };
    assert_eq!((path.as_str(), line), ("/l.jsonl", 2));
}

#[test]
fn test_an_unparseable_time_refuses_the_log() {
    assert!(matches!(
        parse("p", r#"{"op":"add","task":"x","ts":"yesterday"}"#),
        Err(LogError::Malformed { line: 1, .. })
    ));
}

#[test]
fn test_a_record_without_an_op_refuses_the_log() {
    assert_eq!(
        parse("p", r#"{"task":"x"}"#),
        Err(LogError::NoOp {
            path: "p".to_string(),
            line: 1
        })
    );
    assert_eq!(
        parse("p", r#"{"op":"","task":"x"}"#),
        Err(LogError::NoOp {
            path: "p".to_string(),
            line: 1
        })
    );
}

#[test]
fn test_an_op_go_does_not_know_refuses_the_log() {
    assert_eq!(
        parse("p", r#"{"op":"archive","task":"x"}"#),
        Err(LogError::UnknownOp {
            path: "p".to_string(),
            line: 1,
            op: "archive".to_string()
        })
    );
}

#[test]
fn test_projects_are_the_logs_in_the_directory_sorted() {
    let (home, mut logs) = logs(&[
        ("zany-bold-crab.jsonl", ""),
        ("able-calm-fox.jsonl", ADD),
        ("able-calm-fox.jsonl.lock", ""),
        ("able-calm-fox.jsonl.premove.bak", ""),
    ]);
    fs::create_dir(home.path().join(".ctxloom/tasks/dir.jsonl")).unwrap();
    assert_eq!(
        logs.projects().unwrap(),
        ["able-calm-fox", "zany-bold-crab"]
    );
}

#[test]
fn test_no_directory_is_no_projects() {
    let home = TempDir::new().unwrap();
    assert_eq!(
        GoTaskLogDir::in_home(Some(home.path())).projects().unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn test_an_unreadable_directory_is_an_error() {
    let home = TempDir::new().unwrap();
    fs::create_dir(home.path().join(".ctxloom")).unwrap();
    fs::write(home.path().join(".ctxloom/tasks"), "not a directory").unwrap();
    let error = GoTaskLogDir::in_home(Some(home.path()))
        .projects()
        .unwrap_err();
    assert_eq!(error.code(), "go-task-log-unreadable");
}

#[test]
fn test_records_reads_the_projects_log() {
    let (_home, mut logs) = logs(&[("able-calm-fox.jsonl", ADD)]);
    let records = logs.records("able-calm-fox").unwrap().expect("a log");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].task, "swift-amber");
}

#[test]
fn test_a_project_without_a_log_has_no_records() {
    let (_home, mut logs) = logs(&[]);
    assert_eq!(logs.records("able-calm-fox").unwrap(), None);
}

#[test]
fn test_a_log_that_cannot_be_read_is_an_error() {
    let (home, mut logs) = logs(&[]);
    fs::create_dir(home.path().join(".ctxloom/tasks/able-calm-fox.jsonl")).unwrap();
    let error = logs.records("able-calm-fox").unwrap_err();
    assert_eq!(error.code(), "go-task-log-unreadable");
}

#[test]
fn test_a_bad_log_reports_its_path() {
    let (home, mut logs) = logs(&[("able-calm-fox.jsonl", "{")]);
    let error = logs.records("able-calm-fox").unwrap_err();
    assert_eq!(error.code(), "go-task-log-malformed");
    let path = home.path().join(".ctxloom/tasks/able-calm-fox.jsonl");
    assert!(
        error
            .to_string()
            .starts_with(&format!("{}:1:", path.display())),
        "{error}"
    );
}

#[test]
fn test_bytes_that_are_not_utf8_are_replaced_as_go_replaces_them() {
    let (home, mut logs) = logs(&[]);
    let mut line = br#"{"op":"add","task":"x","text":"caf"#.to_vec();
    line.extend_from_slice(&[0xff]);
    line.extend_from_slice(br#""}"#);
    fs::write(home.path().join(".ctxloom/tasks/able-calm-fox.jsonl"), line).unwrap();
    let records = logs.records("able-calm-fox").unwrap().unwrap();
    assert_eq!(records[0].text, "caf\u{fffd}");
}

#[test]
fn test_without_a_home_every_read_says_so() {
    for home in [None, Some(std::path::Path::new(""))] {
        let mut logs = GoTaskLogDir::in_home(home);
        assert_eq!(logs.projects().unwrap_err().code(), "go-task-log-no-home");
        assert_eq!(
            logs.records("able-calm-fox").unwrap_err().code(),
            "go-task-log-no-home"
        );
    }
}

#[test]
fn test_a_directory_named_outright_is_read() {
    let (home, _) = logs(&[("able-calm-fox.jsonl", ADD)]);
    let mut logs = GoTaskLogDir::new(home.path().join(".ctxloom/tasks"));
    assert_eq!(logs.projects().unwrap(), ["able-calm-fox"]);
}
