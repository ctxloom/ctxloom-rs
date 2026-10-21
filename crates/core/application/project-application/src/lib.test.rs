//! The use cases over fake ports: what each finds in a survey and in the
//! check of a database, what it would change, and that it changes it only
//! when asked.

use std::path::{Path, PathBuf};

use annals_diagnostic::Report;

use super::{
    Change, Checked, IdSource, Plan, Problem, ProjectAdmin, ProjectError, Projects, Registered,
    StoreCheck, Survey,
};

/// A survey answered, and every apply recorded.
struct Fake {
    survey: Survey,
    surveyed: Vec<Option<String>>,
    applied: Vec<(Survey, Vec<Change>)>,
    refuse: Option<Report>,
}

impl ProjectAdmin for Fake {
    fn survey(&mut self, pinned: Option<&str>) -> Result<Survey, Report> {
        self.surveyed.push(pinned.map(str::to_string));
        match &self.refuse {
            Some(report) => Err(report.clone()),
            None => Ok(self.survey.clone()),
        }
    }

    fn apply(
        &mut self,
        pinned: Option<&str>,
        seen: &Survey,
        changes: &[Change],
    ) -> Result<(), Report> {
        assert_eq!(pinned, None, "changes are made for the directory, unpinned");
        self.applied.push((seen.clone(), changes.to_vec()));
        Ok(())
    }
}

/// A database check answered, and every database checked recorded.
struct FakeCheck {
    found: Result<Checked, Report>,
    checked: Vec<PathBuf>,
}

impl StoreCheck for FakeCheck {
    fn check(&mut self, store: &Path) -> Result<Checked, Report> {
        self.checked.push(store.to_path_buf());
        self.found.clone()
    }
}

fn entry(id: &str, path: &str, exists: bool) -> Registered {
    Registered {
        id: id.to_string(),
        path: format!("{path}/"),
        canonical: path.to_string(),
        exists,
    }
}

/// The work root /w, nothing bound.
fn survey() -> Survey {
    Survey {
        work_root: "/w".to_string(),
        root: "/w".to_string(),
        resolved: None,
        marker: None,
        legacy_marker: None,
        registry: Vec::new(),
        databases: Vec::new(),
        projects: PathBuf::from("/h/.ctxloom/projects"),
    }
}

fn projects(survey: Survey) -> Projects<Fake, FakeCheck> {
    checked(survey, Ok(Checked::Absent))
}

/// The use cases over `survey`, its database's check finding `found`.
fn checked(survey: Survey, found: Result<Checked, Report>) -> Projects<Fake, FakeCheck> {
    Projects::new(
        Fake {
            survey,
            surveyed: Vec::new(),
            applied: Vec::new(),
            refuse: None,
        },
        FakeCheck {
            found,
            checked: Vec::new(),
        },
    )
}

fn damage() -> Report {
    Report::new(
        "annals-store-sql-damaged",
        "the database file is damaged: row 2 missing from index i",
    )
    .arg("first", "row 2 missing from index i")
    .arg("more", 0u64)
}

fn marked(id: &str) -> Survey {
    Survey {
        marker: Some(id.to_string()),
        resolved: Some((id.to_string(), IdSource::Marker)),
        ..survey()
    }
}

fn write_marker(root: &str, id: &str) -> Change {
    Change::WriteMarker {
        root: root.to_string(),
        id: id.to_string(),
    }
}

fn register(id: &str, path: &str) -> Change {
    Change::Register {
        id: id.to_string(),
        path: path.to_string(),
    }
}

fn unregister(id: &str, path: &str) -> Change {
    Change::Unregister {
        id: id.to_string(),
        path: path.to_string(),
    }
}

fn s(text: &str) -> String {
    text.to_string()
}

// ------------------------------------------------------------ show

#[test]
fn test_show_gives_the_root_its_entries_its_store_and_passes_the_pin() {
    let mut survey = marked("a");
    survey.registry = vec![entry("a", "/w", true), entry("b", "/x", true)];
    let mut projects = projects(survey.clone());
    let shown = projects.show(Some("pinned")).unwrap();
    assert_eq!(projects.admin().surveyed, [Some(s("pinned"))]);
    assert_eq!(shown.survey, survey);
    assert_eq!(shown.entries, [entry("a", "/w", true)]);
    assert_eq!(
        shown.store,
        Some(PathBuf::from("/h/.ctxloom/projects/a/state.db"))
    );
    assert_eq!(shown.problems, []);
}

#[test]
fn test_show_with_no_project_has_no_store() {
    let shown = projects(survey()).show(None).unwrap();
    assert_eq!(shown.store, None);
    assert_eq!(shown.problems, []);
}

#[test]
fn test_show_finds_a_root_registered_under_two_ids() {
    let mut survey = survey();
    survey.registry = vec![
        entry("a", "/w", true),
        entry("b", "/w", true),
        entry("a", "/w", true),
    ];
    let problems = projects(survey).show(None).unwrap().problems;
    assert_eq!(
        problems,
        [Problem::TwoIds {
            root: s("/w"),
            ids: vec![s("a"), s("b")],
        }]
    );
}

#[test]
fn test_show_finds_a_marker_naming_another_id_than_the_registry() {
    let mut survey = marked("m");
    survey.registry = vec![entry("a", "/w", true)];
    let problems = projects(survey).show(None).unwrap().problems;
    assert_eq!(
        problems,
        [Problem::TwoIds {
            root: s("/w"),
            ids: vec![s("a"), s("m")],
        }]
    );
}

#[test]
fn test_show_finds_a_registered_root_with_no_marker() {
    let mut survey = survey();
    survey.registry = vec![entry("a", "/w", true)];
    survey.resolved = Some((s("a"), IdSource::Registry));
    let problems = projects(survey).show(None).unwrap().problems;
    assert_eq!(
        problems,
        [Problem::MissingMarker {
            root: s("/w"),
            id: s("a"),
        }]
    );
}

#[test]
fn test_show_finds_a_marker_the_registry_does_not_hold() {
    let problems = projects(marked("m")).show(None).unwrap().problems;
    assert_eq!(
        problems,
        [Problem::Unregistered {
            root: s("/w"),
            id: s("m"),
        }]
    );
}

#[test]
fn test_show_reports_another_live_tree_holding_the_id_as_sharing_not_a_problem() {
    // A second clone with the same committed marker is the same project,
    // by design: only that the root is not registered yet is for heal.
    let mut survey = marked("m");
    survey.registry = vec![entry("m", "/gone", false), entry("m", "/clone", true)];
    let shown = projects(survey).show(None).unwrap();
    assert_eq!(shown.shared_with, [s("/clone/")]);
    assert_eq!(
        shown.problems,
        [
            Problem::Unregistered {
                root: s("/w"),
                id: s("m"),
            },
            Problem::Gone {
                id: s("m"),
                path: s("/gone/"),
            },
        ]
    );
}

#[test]
fn test_show_shares_with_no_tree_but_the_live_others_of_the_resolved_id() {
    let mut alone = marked("m");
    alone.registry = vec![
        entry("m", "/w", true),
        entry("m", "/gone", false),
        entry("other", "/x", true),
    ];
    assert_eq!(projects(alone).show(None).unwrap().shared_with, [""; 0]);
    let mut pinned = survey();
    pinned.resolved = Some((s("p"), IdSource::Flag));
    pinned.registry = vec![entry("p", "/x", true)];
    assert_eq!(
        projects(pinned).show(Some("p")).unwrap().shared_with,
        [s("/x/")],
        "the project shown, wherever its id came from"
    );
    assert_eq!(
        projects(survey()).show(None).unwrap().shared_with,
        [""; 0],
        "no project, no sharing"
    );
}

#[test]
fn test_show_finds_gos_older_marker_alone() {
    let mut survey = survey();
    survey.legacy_marker = Some(s("g"));
    survey.registry = vec![entry("g", "/w", true)];
    let problems = projects(survey).show(None).unwrap().problems;
    assert_eq!(
        problems,
        [Problem::LegacyMarkerOnly {
            root: s("/w"),
            id: s("g"),
        }]
    );
}

#[test]
fn test_show_finds_orphaned_databases_but_not_its_own() {
    let mut survey = marked("own");
    survey.registry = vec![
        entry("own", "/w", true),
        entry("live", "/x", true),
        entry("dead", "/y", false),
    ];
    survey.databases = vec![s("dead"), s("live"), s("own"), s("stray")];
    let problems = projects(survey).show(None).unwrap().problems;
    assert_eq!(
        problems,
        [
            Problem::Gone {
                id: s("dead"),
                path: s("/y/"),
            },
            Problem::Orphan {
                id: s("dead"),
                database: PathBuf::from("/h/.ctxloom/projects/dead"),
            },
            Problem::Orphan {
                id: s("stray"),
                database: PathBuf::from("/h/.ctxloom/projects/stray"),
            },
        ]
    );
}

#[test]
fn test_a_failing_survey_fails_each_use_case_as_the_ports_error() {
    let report = Report::new("project-registry-no-home", "no home");
    let mut projects = projects(survey());
    projects.admin.refuse = Some(report.clone());
    let refused = ProjectError::Port(report);
    assert_eq!(projects.show(None).unwrap_err(), refused);
    assert_eq!(projects.heal(true).unwrap_err(), refused);
    assert_eq!(projects.repoint("a", true, true).unwrap_err(), refused);
    assert_eq!(projects.orphans().unwrap_err(), refused);
    assert_eq!(projects.remove_orphans(&[], true).unwrap_err(), refused);
}

// ------------------------------------------------------------ heal

#[test]
fn test_heal_registers_the_marker_and_unregisters_what_else_the_root_is() {
    let mut survey = marked("m");
    survey.registry = vec![
        entry("a", "/w", true),
        entry("m", "/moved-from", false),
        entry("m", "/moved-from", false),
    ];
    let mut projects = projects(survey.clone());
    let plan = projects.heal(false).unwrap();
    assert_eq!(
        plan,
        Plan {
            changes: vec![
                register("m", "/w"),
                unregister("a", "/w"),
                unregister("m", "/moved-from"),
            ],
            applied: false,
            unresolved: vec![],
            previous: None,
        }
    );
    assert!(projects.admin.applied.is_empty(), "only reported");
    assert_eq!(projects.admin.surveyed, [None]);
    let applied = projects.heal(true).unwrap();
    assert!(applied.applied);
    assert_eq!(projects.admin.applied, [(survey, plan.changes)]);
}

#[test]
fn test_heal_writes_a_missing_marker_from_the_roots_one_entry() {
    let mut survey = survey();
    survey.registry = vec![entry("a", "/w", true), entry("a", "/w", true)];
    let plan = projects(survey).heal(false).unwrap();
    assert_eq!(plan.changes, [write_marker("/w", "a")]);
}

#[test]
fn test_heal_adopts_gos_older_marker() {
    let mut survey = survey();
    survey.legacy_marker = Some(s("g"));
    survey.registry = vec![entry("g", "/w", true)];
    let plan = projects(survey).heal(false).unwrap();
    assert_eq!(plan.changes, [write_marker("/w", "g")]);
}

#[test]
fn test_heal_leaves_two_ids_and_no_marker_to_a_repoint() {
    let mut survey = survey();
    survey.registry = vec![entry("a", "/w", true), entry("b", "/w", true)];
    let mut projects = projects(survey);
    let plan = projects.heal(true).unwrap();
    assert_eq!(plan.changes, []);
    assert!(!plan.applied, "nothing to make");
    assert!(projects.admin.applied.is_empty());
    assert_eq!(
        plan.unresolved,
        [Problem::TwoIds {
            root: s("/w"),
            ids: vec![s("a"), s("b")],
        }]
    );
}

#[test]
fn test_heal_registers_a_second_live_tree_holding_the_same_id() {
    // Clones share by default: heal registers this tree beside the other,
    // which it leaves registered.
    let mut survey = marked("m");
    survey.registry = vec![entry("m", "/clone", true)];
    let plan = projects(survey).heal(true).unwrap();
    assert_eq!(plan.changes, [register("m", "/w")]);
    assert_eq!(plan.unresolved, []);
    assert!(plan.applied);
}

#[test]
fn test_heal_of_a_consistent_or_unbound_root_changes_nothing() {
    let mut consistent = marked("m");
    consistent.registry = vec![entry("m", "/w", true)];
    assert_eq!(projects(consistent).heal(true).unwrap(), Plan::default());
    assert_eq!(projects(survey()).heal(true).unwrap(), Plan::default());
}

#[test]
fn test_heal_checks_the_resolved_projects_database_and_reports_damage_unrepaired() {
    let mut survey = marked("m");
    survey.registry = vec![entry("a", "/w", true)];
    let mut projects = checked(survey.clone(), Ok(Checked::Damaged(damage())));
    let found = Problem::Damaged {
        id: s("m"),
        database: PathBuf::from("/h/.ctxloom/projects/m/state.db"),
        damage: damage(),
    };
    let plan = projects.heal(false).unwrap();
    assert_eq!(plan.unresolved, std::slice::from_ref(&found));
    assert_eq!(
        plan.changes,
        [register("m", "/w"), unregister("a", "/w")],
        "the skew is healed as ever, the damage left alone"
    );
    let applied = projects.heal(true).unwrap();
    assert!(applied.applied);
    assert_eq!(applied.unresolved, [found], "reported when applying too");
    assert_eq!(projects.admin.applied, [(survey, plan.changes)]);
    assert_eq!(
        projects.checks().checked,
        [
            PathBuf::from("/h/.ctxloom/projects/m/state.db"),
            PathBuf::from("/h/.ctxloom/projects/m/state.db"),
        ]
    );
}

#[test]
fn test_heal_finds_nothing_in_an_intact_or_absent_database() {
    for found in [Checked::Intact, Checked::Absent] {
        let mut consistent = marked("m");
        consistent.registry = vec![entry("m", "/w", true)];
        let mut projects = checked(consistent, Ok(found));
        assert_eq!(projects.heal(true).unwrap(), Plan::default());
        assert_eq!(projects.checks().checked.len(), 1);
    }
}

#[test]
fn test_heal_with_no_project_checks_no_database() {
    let mut projects = checked(survey(), Ok(Checked::Damaged(damage())));
    assert_eq!(projects.heal(true).unwrap(), Plan::default());
    assert!(projects.checks().checked.is_empty());
}

#[test]
fn test_a_check_that_cannot_run_fails_the_heal_changing_nothing() {
    let failed = Report::new("annals-store-sql-unavailable", "locked");
    let mut survey = marked("m");
    survey.registry = vec![entry("a", "/w", true)];
    let mut projects = checked(survey, Err(failed.clone()));
    assert_eq!(projects.heal(true).unwrap_err(), ProjectError::Port(failed));
    assert!(projects.admin.applied.is_empty());
}

// ------------------------------------------------------------ repoint

#[test]
fn test_repoint_binds_the_work_root_and_names_what_it_was() {
    let mut survey = marked("old");
    survey.registry = vec![entry("old", "/w", true), entry("t", "/gone", false)];
    let mut projects = projects(survey.clone());
    let plan = projects.repoint("t", false, false).unwrap();
    assert_eq!(
        plan,
        Plan {
            changes: vec![
                write_marker("/w", "t"),
                register("t", "/w"),
                unregister("old", "/w"),
                unregister("t", "/gone"),
            ],
            applied: false,
            unresolved: vec![],
            previous: Some(s("old")),
        }
    );
    assert!(projects.admin.applied.is_empty());
    assert!(projects.repoint("t", false, true).unwrap().applied);
    assert_eq!(projects.admin.applied, [(survey, plan.changes)]);
}

#[test]
fn test_repoint_to_an_id_known_only_by_its_database() {
    let mut survey = survey();
    survey.databases = vec![s("restored")];
    let plan = projects(survey).repoint("restored", false, false).unwrap();
    assert_eq!(
        plan.changes,
        [write_marker("/w", "restored"), register("restored", "/w")]
    );
    assert_eq!(plan.previous, None);
}

#[test]
fn test_repoint_to_an_unknown_id_is_refused() {
    let mut survey = survey();
    survey.databases = vec![s("other")];
    survey.registry = vec![entry("another", "/x", true)];
    assert_eq!(
        projects(survey).repoint("tpyo", true, true).unwrap_err(),
        ProjectError::UnknownProject { id: s("tpyo") }
    );
}

#[test]
fn test_repoint_refuses_while_the_ids_tree_exists_unless_shared() {
    let mut survey = survey();
    survey.registry = vec![entry("t", "/original", true)];
    let mut projects = projects(survey);
    assert_eq!(
        projects.repoint("t", false, true).unwrap_err(),
        ProjectError::InUse {
            id: s("t"),
            path: s("/original/"),
        }
    );
    assert!(projects.admin.applied.is_empty());
    let plan = projects.repoint("t", true, true).unwrap();
    assert_eq!(plan.changes, [write_marker("/w", "t"), register("t", "/w")]);
    assert!(plan.applied);
}

#[test]
fn test_repoint_to_the_id_the_marker_already_names_is_not_refused_for_sharing_it() {
    // A clone already shares its marker's project: a repoint to that id
    // moves it nowhere, so there is no one else's id to guard.
    let mut survey = marked("t");
    survey.registry = vec![entry("t", "/original", true)];
    let plan = projects(survey).repoint("t", false, true).unwrap();
    assert_eq!(plan.changes, [register("t", "/w")]);
    assert!(plan.applied);
}

#[test]
fn test_repoint_from_another_id_onto_one_a_live_tree_holds_is_still_refused() {
    // A tree with an id of its own, moved onto someone else's: not what a
    // shared marker gives, so --share says it is meant.
    let mut survey = marked("own");
    survey.registry = vec![entry("own", "/w", true), entry("t", "/original", true)];
    assert_eq!(
        projects(survey.clone()).repoint("t", false, false),
        Err(ProjectError::InUse {
            id: s("t"),
            path: s("/original/"),
        })
    );
    // Pinned to it, the tree is still not it.
    survey.resolved = Some((s("t"), IdSource::Flag));
    assert!(projects(survey).repoint("t", false, false).is_err());
}

#[test]
fn test_repoint_to_what_the_root_already_is_changes_nothing() {
    let mut survey = marked("t");
    survey.registry = vec![entry("t", "/w", true)];
    let plan = projects(survey).repoint("t", false, true).unwrap();
    assert_eq!(plan.changes, []);
    assert!(!plan.applied);
    assert_eq!(plan.previous, Some(s("t")));
}

#[test]
fn test_repoint_in_a_worktree_marks_the_worktree_not_its_primary() {
    let mut survey = marked("primary-id");
    survey.root = s("/primary");
    survey.registry = vec![entry("primary-id", "/primary", true)];
    survey.databases = vec![s("primary-id"), s("t")];
    let plan = projects(survey).repoint("t", false, false).unwrap();
    assert_eq!(plan.changes, [write_marker("/w", "t"), register("t", "/w")]);
}

#[test]
fn test_repoint_names_no_previous_id_given_by_a_pin() {
    for source in [IdSource::Flag, IdSource::Environment] {
        let mut survey = survey();
        survey.resolved = Some((s("pinned"), source));
        survey.databases = vec![s("t")];
        assert_eq!(
            projects(survey)
                .repoint("t", false, false)
                .unwrap()
                .previous,
            None
        );
    }
}

// ------------------------------------------------------------ orphans

fn with_orphans() -> Survey {
    let mut survey = marked("own");
    survey.registry = vec![entry("own", "/w", true), entry("live", "/x", true)];
    survey.databases = vec![s("live"), s("own"), s("stray"), s("x2")];
    survey
}

#[test]
fn test_orphans_are_listed() {
    let orphans = projects(with_orphans()).orphans().unwrap();
    assert_eq!(
        orphans,
        [
            Problem::Orphan {
                id: s("stray"),
                database: PathBuf::from("/h/.ctxloom/projects/stray"),
            },
            Problem::Orphan {
                id: s("x2"),
                database: PathBuf::from("/h/.ctxloom/projects/x2"),
            },
        ]
    );
}

#[test]
fn test_orphans_are_removed_by_name_and_only_when_asked() {
    let survey = with_orphans();
    let mut projects = projects(survey.clone());
    let ids = [s("stray"), s("stray")];
    let plan = projects.remove_orphans(&ids, false).unwrap();
    let removal = vec![Change::RemoveDatabase { id: s("stray") }];
    assert_eq!(plan.changes, removal);
    assert!(projects.admin.applied.is_empty());
    assert!(projects.remove_orphans(&ids, true).unwrap().applied);
    assert_eq!(projects.admin.applied, [(survey, removal)]);
}

#[test]
fn test_a_removal_naming_anything_not_orphaned_removes_nothing() {
    for named in ["live", "own", "nonexistent"] {
        let mut projects = projects(with_orphans());
        let ids = [s("stray"), s(named)];
        assert_eq!(
            projects.remove_orphans(&ids, true).unwrap_err(),
            ProjectError::NotOrphan { id: s(named) }
        );
        assert!(projects.admin.applied.is_empty());
    }
}

#[test]
fn test_heal_leaves_a_tree_already_sharing_its_id_alone() {
    let mut survey = marked("m");
    survey.registry = vec![entry("m", "/w", true), entry("m", "/clone", true)];
    let shown = projects(survey.clone()).show(None).unwrap();
    assert_eq!(
        shown.problems,
        [],
        "shared, as clones with one marker share"
    );
    assert_eq!(shown.shared_with, [s("/clone/")]);
    assert_eq!(projects(survey).heal(true).unwrap(), Plan::default());
}
