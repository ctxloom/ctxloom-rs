//! What the shipped formulas rank, read from the default tag schema itself
//! (a copy here would go on proving things of formulas no longer shipped),
//! and the warnings a ranking that means little earns.

use annals::next::Timestamp;
use annals_diagnostic::{Arg, Diagnostic};
use task_priority::{Diagnostics, Ranking, TargetCoverage, MAX};

use super::{candidate, formulas, rank_rows, warnings, PriorityNote, PriorityWarning};
use crate::view::{text_hash, Row, Task};
use crate::{Project, TagSchema};

/// 2026-07-20T12:00:00Z: every age here is measured from it.
const NOW: Timestamp = Timestamp {
    seconds: 1_784_548_800,
    nanos: 0,
};

fn project(id: &str) -> Project {
    Project {
        id: id.to_string(),
        root: format!("/work/{id}"),
        store: format!("/home/.ctxloom/projects/{id}/state.db").into(),
    }
}

/// A task of the project `p`, `days` old.
fn row(p: &str, harp_id: &str, status: &str, tags: &[&str], days: i64) -> Row {
    Row {
        project: project(p),
        task: Task {
            harp_id: harp_id.to_string(),
            text: harp_id.to_string(),
            status: status.to_string(),
            checked: crate::is_terminal(status),
            text_hash: text_hash(harp_id),
            trigger: String::new(),
            origin_session: String::new(),
            tags: tags
                .iter()
                .map(|s| tagma_core::Tag::parse(s).expect("a valid tag").into())
                .collect(),
            created_at: Timestamp {
                seconds: NOW.seconds - days * 86_400,
                nanos: 0,
            },
        },
    }
}

/// `rows` ranked by the default schema's formulas.
fn shipped(rows: &[Row]) -> Ranking {
    let schema = TagSchema::resolved(&[]).unwrap();
    let formulas = formulas(&schema).unwrap();
    let candidates: Vec<_> = rows.iter().map(candidate).collect();
    task_priority::rank(&candidates, &formulas, NOW).unwrap()
}

fn raws(ranking: &Ranking) -> Vec<f64> {
    ranking.scores.iter().map(|s| s.raw).collect()
}

fn todo(harp_id: &str, tags: &[&str]) -> Row {
    row("p", harp_id, "To Do", tags, 0)
}

// ------------------------------------------------------------ shipped formulas

#[test]
fn test_each_level_is_worth_twice_the_one_below() {
    let rows: Vec<Row> = (1..=5)
        .map(|level| todo(&format!("l{level}"), &[&format!("triage:level={level}")]))
        .collect();
    let ranking = shipped(&rows);
    assert_eq!(raws(&ranking), [4.0, 2.0, 1.0, 0.5, 0.25]);
    assert_eq!(ranking.scores[0].priority, MAX);
    assert_eq!(ranking.scores[4].priority, MAX / 5.0);
}

#[test]
fn test_an_untriaged_task_floors_below_the_mildest_level() {
    let rows = [
        todo("critical", &["triage:level=1"]),
        todo("wishlist", &["triage:level=5"]),
        todo("untriaged", &[]),
    ];
    let ranking = shipped(&rows);
    assert_eq!(
        raws(&ranking),
        [4.0, 0.25, 0.1],
        "not the unguarded 2**3 == 8"
    );
}

#[test]
fn test_effort_divides_and_a_release_blocker_doubles() {
    let rows = [
        todo("plain", &["triage:level=2"]),
        todo("costly", &["triage:level=2", "triage:effort=2"]),
        todo(
            "blocker",
            &["triage:level=2", "triage:blocks-release=0.7.0"],
        ),
    ];
    assert_eq!(raws(&shipped(&rows)), [2.0, 1.0, 4.0]);
}

#[test]
fn test_an_exposed_task_escalates_with_age_but_a_deferred_one_does_not() {
    let rows = [
        row(
            "p",
            "active",
            "To Do",
            &["triage:level=2", "triage:exposed"],
            100,
        ),
        row(
            "p",
            "parked",
            "Deferred",
            &["triage:level=2", "triage:exposed"],
            100,
        ),
    ];
    let ranking = shipped(&rows);
    assert!((ranking.scores[0].raw - 2.0 * (1.0 + 100.0 / 190.0)).abs() < 1e-9);
    assert_eq!(ranking.scores[1].raw, 2.0);
}

#[test]
fn test_age_promotes_a_task_toward_the_next_level_and_never_past_it() {
    let rows = [
        row(
            "p",
            "old-minor",
            "To Do",
            &["triage:level=4", "triage:exposed"],
            100_000,
        ),
        row(
            "p",
            "old-normal",
            "To Do",
            &["triage:level=3", "triage:exposed"],
            100_000,
        ),
        todo("fresh-normal", &["triage:level=3"]),
        todo("fresh-serious", &["triage:level=2"]),
    ];
    let raw = raws(&shipped(&rows));
    assert!((raw[0] - 1.0).abs() < 0.01 && raw[0] < raw[2]);
    assert!((raw[1] - 2.0).abs() < 0.01 && raw[1] > raw[2] && raw[1] < raw[3]);
}

#[test]
fn test_a_capability_decays_over_its_half_life() {
    let rows = [
        row(
            "p",
            "capability",
            "To Do",
            &["triage:level=2", "triage:kind=capability"],
            120,
        ),
        row(
            "p",
            "defect",
            "To Do",
            &["triage:level=2", "triage:kind=defect"],
            120,
        ),
    ];
    let raw = raws(&shipped(&rows));
    assert!((raw[0] - 1.4).abs() < 1e-9, "2 * (0.4 + 0.6 * 0.5)");
    assert_eq!(raw[1], 2.0);
}

#[test]
fn test_every_term_of_the_shipped_formulas_is_covered() {
    let ranking = shipped(&[todo("a", &["triage:level=1"])]);
    let covered: Vec<(&str, usize)> = ranking
        .diagnostics
        .target_coverage
        .iter()
        .map(|c| (c.target.as_str(), c.tasks))
        .collect();
    assert_eq!(
        covered,
        [
            ("triage:blind-gate", 0),
            ("triage:blocks-release", 0),
            ("triage:effort", 0),
            ("triage:exposed", 0),
            ("triage:kind", 0),
            ("triage:level", 1),
        ]
    );
}

// ------------------------------------------------------------ ranking rows

#[test]
fn test_rows_are_ranked_within_their_own_project() {
    let rows = [
        row("a", "a-high", "To Do", &["triage:level=1"], 0),
        row("a", "a-low", "To Do", &["triage:level=5"], 0),
        row("b", "b-only", "To Do", &["triage:level=5"], 0),
    ];
    let schema = TagSchema::resolved(&[]).unwrap();
    let ranked = rank_rows(&rows, &formulas(&schema).unwrap(), NOW, false).unwrap();
    assert_eq!(ranked.priority(&rows[0]), MAX);
    assert_eq!(ranked.priority(&rows[1]), MAX / 2.0);
    assert_eq!(ranked.priority(&rows[2]), MAX, "b's one task tops b");
}

#[test]
fn test_warnings_are_attributed_to_their_project_when_asked() {
    let rows = [
        row("a", "a-1", "To Do", &[], 0),
        row("a", "a-2", "To Do", &[], 0),
    ];
    let schema = TagSchema::resolved(&[]).unwrap();
    let formulas = formulas(&schema).unwrap();
    let plain = rank_rows(&rows, &formulas, NOW, false).unwrap();
    assert!(!plain.notes.is_empty());
    assert!(plain.notes.iter().all(|n| n.project.is_none()));
    let attributed = rank_rows(&rows, &formulas, NOW, true).unwrap();
    assert!(attributed
        .notes
        .iter()
        .all(|n| n.project.as_deref() == Some("a")));
    let first = &attributed.notes[0];
    assert_eq!(first.code(), "tasks-application-priority-in-project");
    assert!(first.to_string().starts_with("project a: "), "{first}");
}

// ------------------------------------------------------------ warnings

fn coverage(target: &str, tasks: usize) -> TargetCoverage {
    TargetCoverage {
        target: target.to_string(),
        tasks,
    }
}

#[test]
fn test_no_priority_fn_is_the_one_warning() {
    let diagnostics = Diagnostics {
        no_priority_fn: true,
        all_tied: true,
        non_terminal_tasks: 4,
        target_coverage: vec![coverage("a:b", 0)],
        ..Diagnostics::default()
    };
    assert_eq!(warnings(&diagnostics), [PriorityWarning::NoPriorityFn]);
}

#[test]
fn test_a_tie_and_each_thinly_covered_target_are_warned_of_worst_first() {
    let diagnostics = Diagnostics {
        all_tied: true,
        scored_tasks: 1,
        non_terminal_tasks: 4,
        target_coverage: vec![
            coverage("a:dead", 0),
            coverage("a:thin", 1),
            coverage("a:half", 2),
            coverage("a:full", 4),
        ],
        ..Diagnostics::default()
    };
    assert_eq!(
        warnings(&diagnostics),
        [
            PriorityWarning::AllTied { scored: 1, live: 4 },
            PriorityWarning::Inert {
                target: "a:dead".to_string(),
                live: 4
            },
            PriorityWarning::Thin {
                uncovered: 3,
                live: 4,
                target: "a:thin".to_string()
            },
        ],
        "half the live tasks is covered enough"
    );
}

#[test]
fn test_a_healthy_ranking_or_no_live_task_warns_of_nothing() {
    let healthy = Diagnostics {
        non_terminal_tasks: 2,
        scored_tasks: 2,
        target_coverage: vec![coverage("a:b", 2)],
        ..Diagnostics::default()
    };
    assert!(warnings(&healthy).is_empty());
    let empty = Diagnostics {
        target_coverage: vec![coverage("a:b", 0)],
        ..Diagnostics::default()
    };
    assert!(warnings(&empty).is_empty());
}

#[test]
fn test_each_warning_renders_go_s_english() {
    let cases = [
        (
            PriorityWarning::NoPriorityFn,
            "--sort priority is meaningless here: this project's tag_schema declares no priority_fn, so every task's raw score is 0 and the ranking reflects nothing",
        ),
        (
            PriorityWarning::AllTied { scored: 1, live: 4 },
            "--sort priority is meaningless here: every active task ties at the same raw priority score (only 1 of 4 active tasks carry a tag any priority_fn/decay_fn formula actually reads) \u{2014} the ranking reflects nothing",
        ),
        (
            PriorityWarning::Inert {
                target: "a:dead".to_string(),
                live: 4,
            },
            "priority_fn/decay_fn reads a:dead, which none of the 4 active tasks carries \u{2014} that term is inert and moves no task's rank",
        ),
        (
            PriorityWarning::Thin {
                uncovered: 3,
                live: 4,
                target: "a:thin".to_string(),
            },
            "3 of 4 active tasks carry no a:thin \u{2014} they rank as if it were absent, on a formula that reads it",
        ),
    ];
    for (warning, english) in cases {
        assert_eq!(warning.to_string(), english);
    }
    let note = PriorityNote {
        project: Some("p".to_string()),
        warning: PriorityWarning::NoPriorityFn,
    };
    assert!(
        matches!(&note.args()[..], [("project", Arg::Str(p)), ("warning", Arg::Nested(_))] if p == "p")
    );
    let bare = PriorityNote {
        project: None,
        warning: PriorityWarning::AllTied { scored: 1, live: 4 },
    };
    assert_eq!(bare.code(), "tasks-application-priority-all-tied");
    assert_eq!(bare.to_string(), bare.warning.to_string());
    assert_eq!(bare.args().len(), 2);
}
