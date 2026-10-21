//! A prune's report, as text and as JSON, and the command that applies it.

use std::path::PathBuf;

use presentation::{render, Format};
use tasks_application::{Dangling, Project, Pruned, Pruning, Shown, Task, Timestamp, Via};

use super::{apply_command, shell_word, PruneReport};

fn task(harp: &str, status: &str, text: &str) -> Task {
    Task {
        harp_id: harp.to_string(),
        text: text.to_string(),
        status: status.to_string(),
        checked: true,
        text_hash: "0123456789ab".to_string(),
        trigger: String::new(),
        origin_session: String::new(),
        tags: vec![tasks_application::parse_tag("tagma:hidden")
            .unwrap()
            .unwrap()],
        created_at: Timestamp::default(),
    }
}

fn pruning(applied: bool) -> Pruning {
    Pruning {
        project: Some(Project {
            id: "jolly-mute-magma".to_string(),
            root: "/work/tree".to_string(),
            store: PathBuf::new(),
        }),
        statuses: vec!["Done".to_string(), "Archived".to_string()],
        before: Timestamp {
            seconds: 1_767_268_800,
            nanos: 0,
        },
        tag_query: "release".to_string(),
        applied,
        pruned: vec![
            Pruned {
                task: task("swift-amber", "Done", "Ship it\nand more"),
                status_set_at: Timestamp {
                    seconds: 1_766_000_000,
                    nanos: 5,
                },
            },
            Pruned {
                task: task("tall-river-two", "Archived", "Put away"),
                status_set_at: Timestamp {
                    seconds: 1_700_000_000,
                    nanos: 0,
                },
            },
        ],
        events_removed: 5,
        tasks_remaining: 3,
        dangling: vec![
            Dangling {
                harp_id: "keeper".to_string(),
                pruned: "swift-amber".to_string(),
                via: Via::Tag("relates:swift-amber".to_string()),
            },
            Dangling {
                harp_id: "waiter".to_string(),
                pruned: "tall-river-two".to_string(),
                via: Via::Trigger,
            },
        ],
        shown: Shown::default(),
    }
}

fn rendered(format: Format, pruning: &Pruning) -> String {
    let mut out = Vec::new();
    render(&mut out, format, &PruneReport(pruning)).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn test_a_report_as_text_is_gos() {
    assert_eq!(
        rendered(Format::Text, &pruning(false)),
        "Project: /work/tree (jolly-mute-magma)\n\
         \n\
         Would prune 2 task(s), 5 event(s) \u{2014} status Done or Archived, \
         last status change before 2026-01-01 12:00 UTC, tag query release:\n\
         \n\
         \x20 swift-amber     Done      2025-12-17  Ship it\n\
         \x20 tall-river-two  Archived  2023-11-14  Put away\n\
         \n\
         3 task(s) remain.\n\
         \n\
         Still named by a surviving task (left as written):\n\
         \x20 keeper -> swift-amber  (tag relates:swift-amber)\n\
         \x20 waiter -> tall-river-two  (trigger)\n"
    );
}

#[test]
fn test_an_applied_prune_says_it_pruned() {
    let text = rendered(Format::Text, &pruning(true));
    assert!(
        text.contains("\nPruned 2 task(s), 5 event(s) \u{2014} "),
        "{text}"
    );
}

#[test]
fn test_a_prune_that_takes_nothing_says_so_with_what_it_selected_by() {
    let mut nothing = pruning(false);
    nothing.pruned.clear();
    nothing.tag_query.clear();
    nothing.project = None;
    assert_eq!(
        rendered(Format::Text, &nothing),
        "Nothing to prune (status Done or Archived, last status change before 2026-01-01 12:00 UTC).\n"
    );
}

#[test]
fn test_a_report_with_no_dangling_reference_ends_with_what_remains() {
    let mut clean = pruning(false);
    clean.dangling.clear();
    assert!(rendered(Format::Text, &clean).ends_with("\n3 task(s) remain.\n"));
}

#[test]
fn test_a_report_as_json_is_one_document_in_gos_shape() {
    let json = rendered(Format::Json, &pruning(false));
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let order = [
        "project_id",
        "project_dir",
        "statuses",
        "before",
        "tag_query",
        "applied",
        "pruned",
        "tasks_pruned",
        "events_removed",
        "tasks_remaining",
        "dangling_references",
    ];
    assert_eq!(
        keys.len(),
        order.len(),
        "every field, none absent: {keys:?}"
    );
    let at: Vec<usize> = order
        .iter()
        .map(|key| json.find(&format!("\"{key}\":")).expect(key))
        .collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "in Go's order: {json}");
    assert_eq!(value["project_id"], "jolly-mute-magma");
    assert_eq!(value["project_dir"], "/work/tree");
    assert_eq!(value["statuses"], serde_json::json!(["Done", "Archived"]));
    assert_eq!(value["before"], "2026-01-01T12:00:00Z");
    assert_eq!(value["tag_query"], "release");
    assert_eq!(value["applied"], false);
    assert_eq!(value["tasks_pruned"], 2);
    assert_eq!(value["events_removed"], 5);
    assert_eq!(value["tasks_remaining"], 3);
    assert_eq!(
        value["pruned"][0],
        serde_json::json!({
            "harp_id": "swift-amber",
            "status": "Done",
            "tags": ["tagma:hidden"],
            "headline": "Ship it",
            "last_status_change": "2025-12-17T19:33:20.000000005Z",
        }),
        "every tag, the hidden too"
    );
    assert_eq!(
        value["dangling_references"][1],
        serde_json::json!({"harp_id": "waiter", "pruned": "tall-river-two", "via": "trigger"})
    );
    assert!(json.starts_with('{'));
    assert!(json.trim_end().ends_with('}'));
}

#[test]
fn test_a_report_with_no_project_has_empty_project_fields() {
    let mut nowhere = pruning(false);
    nowhere.project = None;
    let value: serde_json::Value = serde_json::from_str(&rendered(Format::Json, &nowhere)).unwrap();
    assert_eq!(value["project_id"], "");
    assert_eq!(value["project_dir"], "");
}

#[test]
fn test_the_apply_command_repeats_the_selection_then_yes() {
    assert_eq!(
        apply_command("7d", &["Done".to_string()], ""),
        "taskloom prune --older-than 7d --status Done --yes"
    );
    assert_eq!(
        apply_command(
            "2026-01-01",
            &["Done".to_string(), "To Do".to_string()],
            "urgent/release/and"
        ),
        "taskloom prune --older-than 2026-01-01 --status Done --status 'To Do' \
         --tag-query urgent/release/and --yes"
    );
}

#[test]
fn test_a_word_is_quoted_only_where_a_shell_needs_it() {
    assert_eq!(
        shell_word("@terminal,a%b+c-d.e/f:g=h_i09"),
        "@terminal,a%b+c-d.e/f:g=h_i09"
    );
    assert_eq!(shell_word("To Do"), "'To Do'");
    assert_eq!(shell_word("level<=2"), "'level<=2'");
    assert_eq!(shell_word("it's"), r"'it'\''s'");
    assert_eq!(shell_word(""), "''");
    assert_eq!(
        shell_word("caf\u{e9}"),
        "'caf\u{e9}'",
        "ASCII alone is bare"
    );
}
