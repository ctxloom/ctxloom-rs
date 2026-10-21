//! Each problem, change and plan as text, and the shapes text alone shows.

use std::path::PathBuf;

use annals_diagnostic::Report;
use presentation::Catalog;
use project_application::{Change, IdSource, Plan, Problem, Shown, Survey};

use super::{change_line, orphans_json, orphans_text, plan_text, problem_line, show_text};

/// A store's damage, its catalog message differing from its English.
const STORE: (&str, &str) = (
    "fake-store",
    "fake-store-damaged = from the catalog: { $first } (+{ $more })\n",
);

fn catalog() -> Catalog {
    Catalog::en_us(&[STORE]).expect("the catalog loads")
}

fn damage() -> Report {
    Report::new("fake-store-damaged", "the English")
        .arg("first", "row 2 missing from index i")
        .arg("more", 1u64)
}

fn s(text: &str) -> String {
    text.to_string()
}

fn text(write: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut out = Vec::new();
    write(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn test_each_problem_says_what_fixes_it() {
    let lines: Vec<String> = [
        Problem::TwoIds {
            root: s("/w"),
            ids: vec![s("a"), s("b")],
        },
        Problem::MissingMarker {
            root: s("/w"),
            id: s("a"),
        },
        Problem::LegacyMarkerOnly {
            root: s("/w"),
            id: s("g"),
        },
        Problem::Unregistered {
            root: s("/w"),
            id: s("m"),
        },
        Problem::Gone {
            id: s("a"),
            path: s("/p"),
        },
        Problem::Orphan {
            id: s("x"),
            database: PathBuf::from("/h/x"),
        },
        Problem::Damaged {
            id: s("m"),
            database: PathBuf::from("/h/m/state.db"),
            damage: damage(),
        },
    ]
    .iter()
    .map(|p| problem_line(p, &catalog()))
    .collect();
    assert_eq!(
        lines,
        [
            "/w is bound to more than one project: a, b; run `project repoint <project-id>` to choose one",
            "/w is registered as a but has no .project-id; run `project heal`",
            "/w names g only in Go's .ctxloom/project-id; run `project heal` to adopt it",
            "/w/.project-id names m, which the registry does not hold for it; run `project heal`",
            "a is registered for /p, which no longer exists",
            "/h/x is orphaned: no live registered path points at x; `project orphans remove x --yes` deletes it",
            "/h/m/state.db, the database of m: from the catalog: row 2 missing from index i (+1); heal reports damage and does not repair it",
        ]
    );
}

#[test]
fn test_each_change_in_a_line() {
    let lines: Vec<String> = [
        Change::WriteMarker {
            root: s("/w"),
            id: s("a"),
        },
        Change::Register {
            id: s("a"),
            path: s("/w"),
        },
        Change::Unregister {
            id: s("b"),
            path: s("/w"),
        },
        Change::RemoveDatabase { id: s("x") },
    ]
    .iter()
    .map(change_line)
    .collect();
    assert_eq!(
        lines,
        [
            "write a into /w/.project-id",
            "register a for /w",
            "unregister b for /w",
            "delete the database directory of x",
        ]
    );
}

#[test]
fn test_a_plan_says_whether_it_changed_anything() {
    assert_eq!(
        text(|out| plan_text(out, &Plan::default(), &catalog())),
        "Nothing to change.\n"
    );
    let mut plan = Plan {
        changes: vec![Change::RemoveDatabase { id: s("x") }],
        previous: Some(s("old")),
        unresolved: vec![Problem::Gone {
            id: s("a"),
            path: s("/p"),
        }],
        applied: false,
    };
    assert_eq!(
        text(|out| plan_text(out, &plan, &catalog())),
        "Would change (run again with --yes to make the change):\n\
         \x20 - delete the database directory of x\n\
         It was old, whose database stays; `project show` reports it once nothing points at it.\n\
         Not resolved:\n\
         \x20 - a is registered for /p, which no longer exists\n"
    );
    plan.applied = true;
    assert!(text(|out| plan_text(out, &plan, &catalog())).starts_with("Changed:\n"));
}

#[test]
fn test_show_as_text_names_a_worktree_gos_marker_and_what_is_missing() {
    let shown = Shown {
        survey: Survey {
            work_root: s("/linked"),
            root: s("/primary"),
            resolved: None,
            marker: None,
            legacy_marker: Some(s("g")),
            registry: Vec::new(),
            databases: Vec::new(),
            projects: PathBuf::from("/h"),
        },
        entries: Vec::new(),
        store: None,
        shared_with: Vec::new(),
        problems: Vec::new(),
    };
    assert_eq!(
        text(|out| show_text(out, &shown, &catalog())),
        "Project:  none yet (a write would mint one)\n\
         Root:     /primary\n\
         Worktree: /linked (a linked worktree of the root, with no marker of its own)\n\
         Marker:   none\n\
         Go's marker: g\n\
         Registry: no entry for the root\n\
         Database: none\n\
         No problems found.\n"
    );
    let mut sourced = shown.clone();
    sourced.survey.resolved = Some((s("e"), IdSource::Environment));
    assert!(text(|out| show_text(out, &sourced, &catalog()))
        .starts_with("Project:  e (from the environment)\n"));
}

#[test]
fn test_show_as_text_names_each_other_tree_sharing_the_project() {
    let shown = Shown {
        survey: Survey {
            work_root: s("/w"),
            root: s("/w"),
            resolved: Some((s("m"), IdSource::Marker)),
            marker: Some(s("m")),
            legacy_marker: None,
            registry: Vec::new(),
            databases: Vec::new(),
            projects: PathBuf::from("/h"),
        },
        entries: Vec::new(),
        store: Some(PathBuf::from("/h/m/state.db")),
        shared_with: vec![s("/clone"), s("/other")],
        problems: Vec::new(),
    };
    assert_eq!(
        text(|out| show_text(out, &shown, &catalog())),
        "Project:  m (from the marker)\n\
         Root:     /w\n\
         Marker:   m\n\
         Registry: no entry for the root\n\
         Database: /h/m/state.db\n\
         Shared:   /clone (another tree holding the same project id: one project, one database)\n\
         Shared:   /other (another tree holding the same project id: one project, one database)\n\
         No problems found.\n"
    );
}

#[test]
fn test_orphans_leave_out_other_problems_and_say_when_there_are_none() {
    let gone = [Problem::Gone {
        id: s("a"),
        path: s("/p"),
    }];
    assert!(orphans_json(&gone).is_empty());
    assert_eq!(
        text(|out| orphans_text(out, &gone)),
        "No orphaned databases.\n"
    );
}
