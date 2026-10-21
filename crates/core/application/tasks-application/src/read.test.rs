//! The reads over fake ports: what a read reaches, and the listing's
//! filters, hiding and limit.

use std::path::PathBuf;

use annals_diagnostic::Diagnostic;

use crate::fakes::{ready, FakeClock, FakeDelay, FakeIdentity, FakeNames, FakeStores, Flaky};
use crate::{
    AppError, Hidden, ListQuery, NewTask, Project, Reach, Scope, Service, ShowQuery, TaskService,
};

pub(crate) type Fake = Service<FakeIdentity, FakeStores, FakeNames, FakeClock, FakeDelay>;

pub(crate) fn service(names: &[&str]) -> Fake {
    Service::new(
        FakeIdentity::default(),
        FakeStores::default(),
        FakeNames::of(names),
        FakeClock::default(),
        FakeDelay::default(),
    )
}

/// The project `id`, rooted at `/work/<id>`.
pub(crate) fn named(id: &str) -> Project {
    Project {
        id: id.to_string(),
        root: format!("/work/{id}"),
        store: PathBuf::from(format!("/home/.ctxloom/projects/{id}/state.db")),
    }
}

/// Add a task of `text` in `status` (a trigger given for Deferred).
pub(crate) fn add(service: &mut Fake, text: &str, status: &str) {
    let trigger = if status == "Deferred" { "later" } else { "" };
    let task = NewTask {
        text: text.to_string(),
        status: status.to_string(),
        trigger: trigger.to_string(),
        ..NewTask::default()
    };
    ready(service.add(task)).unwrap();
}

/// A service holding a task in each default status and one in
/// `Blocked`, each task's text its status.
pub(crate) fn with_every_status() -> Fake {
    let mut service = service(&["to-do", "doing", "deferred", "done", "archived", "blocked"]);
    for status in [
        "To Do",
        "In Progress",
        "Deferred",
        "Done",
        "Archived",
        "Blocked",
    ] {
        add(&mut service, status, status);
    }
    service
}

fn ids(service: &mut Fake, query: ListQuery) -> Vec<String> {
    ready(service.list(query))
        .unwrap()
        .tasks()
        .map(|t| t.harp_id.clone())
        .collect()
}

fn statuses(statuses: &[&str]) -> ListQuery {
    ListQuery {
        statuses: statuses.iter().map(|s| s.to_string()).collect(),
        ..ListQuery::default()
    }
}

// ------------------------------------------------------------ status classes

#[test]
fn test_open_is_every_status_not_terminal_whatever_its_name() {
    let mut service = with_every_status();
    assert_eq!(
        ids(&mut service, statuses(&["@open"])),
        ["to-do", "doing", "deferred", "blocked"]
    );
}

#[test]
fn test_terminal_is_done_and_archived_and_mixes_with_a_status() {
    let mut service = with_every_status();
    assert_eq!(
        ids(&mut service, statuses(&["@terminal"])),
        ["done", "archived"]
    );
    assert_eq!(
        ids(&mut service, statuses(&[" In Progress ", "@terminal", ""])),
        ["doing", "done", "archived"]
    );
}

#[test]
fn test_a_class_no_one_knows_is_refused_before_anything_is_read() {
    let mut service = with_every_status();
    service.identity.pins.clear();
    let error = ready(service.list(statuses(&["To Do", "@done"]))).unwrap_err();
    assert_eq!(
        error,
        AppError::UnknownStatusClass {
            status: "@done".to_string()
        }
    );
    assert!(error.to_string().contains("@open, @terminal"), "{error}");
    assert!(service.identity.pins.is_empty());
}

// ------------------------------------------------------------ term

#[test]
fn test_a_term_keeps_the_tasks_whose_text_mentions_it_in_any_case() {
    let mut service = service(&["first", "second", "third"]);
    for text in ["Ship the DOCS", "fix docs", "other"] {
        add(&mut service, text, "");
    }
    let query = ListQuery {
        term: " Docs ".to_string(),
        ..ListQuery::default()
    };
    assert_eq!(ids(&mut service, query), ["first", "second"]);
}

// ------------------------------------------------------------ hidden

#[test]
fn test_a_search_says_what_the_default_view_hid_of_its_matches() {
    let mut service = with_every_status();
    let query = ListQuery {
        term: "e".to_string(),
        ..ListQuery::default()
    };
    let listing = ready(service.list(query)).unwrap();
    let shown: Vec<&str> = listing.tasks().map(|t| t.harp_id.as_str()).collect();
    assert_eq!(shown, ["doing", "blocked"], "In Progress, Blocked");
    assert_eq!(
        listing.hidden,
        Hidden {
            completed: 2,
            deferred: 1
        },
        "Done, Archived; Deferred"
    );
    assert_eq!(listing.hidden.total(), 3);
    assert!(listing.searched);
    let plain = ready(service.list(ListQuery::default())).unwrap();
    assert!(!plain.searched);
    let tagged = ListQuery {
        tag_query: "x/not".to_string(),
        ..ListQuery::default()
    };
    assert!(ready(service.list(tagged)).unwrap().searched);
}

#[test]
fn test_all_or_a_status_hides_nothing() {
    let mut service = with_every_status();
    for query in [
        ListQuery {
            all: true,
            ..ListQuery::default()
        },
        statuses(&["Done"]),
    ] {
        let listing = ready(service.list(query)).unwrap();
        assert_eq!(listing.hidden, Hidden::default());
    }
}

// ------------------------------------------------------------ limit

#[test]
fn test_a_limit_keeps_the_first_tasks_and_counts_the_rest() {
    let mut service = with_every_status();
    let query = ListQuery {
        all: true,
        limit: 4,
        ..ListQuery::default()
    };
    let listing = ready(service.list(query)).unwrap();
    let shown: Vec<&str> = listing.tasks().map(|t| t.harp_id.as_str()).collect();
    assert_eq!(shown, ["to-do", "doing", "deferred", "done"]);
    assert_eq!(listing.omitted, 2);
    let roomy = ListQuery {
        limit: 9,
        ..ListQuery::default()
    };
    assert_eq!(ready(service.list(roomy)).unwrap().omitted, 0);
    let none = ready(service.list(ListQuery::default())).unwrap();
    assert_eq!((none.rows.len(), none.omitted), (3, 0), "0 is no limit");
}

// ------------------------------------------------------------ sort

#[test]
fn test_a_sort_is_refused_before_anything_is_read() {
    let mut service = service(&[]);
    for (sort, error) in [
        ("priority", AppError::PriorityUnavailable),
        (
            "age",
            AppError::UnknownSort {
                sort: "age".to_string(),
            },
        ),
    ] {
        let query = ListQuery {
            sort: sort.to_string(),
            ..ListQuery::default()
        };
        assert_eq!(ready(service.list(query)).unwrap_err(), error);
    }
    assert!(service.identity.pins.is_empty());
    let blank = ListQuery {
        sort: " ".to_string(),
        ..ListQuery::default()
    };
    assert!(ready(service.list(blank)).is_ok());
}

// ------------------------------------------------------------ reach

/// Two projects with tasks, `b` added before `a`, and a third with no
/// store.
fn with_two_projects() -> Fake {
    let mut service = service(&["zesty-one", "alpha-two", "brisk-three"]);
    for id in ["alpha-proj", "zulu-proj"] {
        service
            .stores
            .others
            .insert(id.to_string(), Flaky::default());
    }
    service.identity.bound = Some(named("zulu-proj"));
    add(&mut service, "in zulu", "");
    service.identity.bound = Some(named("alpha-proj"));
    add(&mut service, "in alpha", "");
    add(&mut service, "in alpha too", "");
    service.identity.bound = None;
    service.stores.absent = true;
    service.identity.every = vec![named("alpha-proj"), named("empty-proj"), named("zulu-proj")];
    service
}

#[test]
fn test_global_reads_every_project_by_project_then_harp() {
    let mut service = with_two_projects();
    service.identity.bound = Some(named("zulu-proj"));
    let query = ListQuery {
        global: true,
        ..ListQuery::default()
    };
    let listing = ready(service.list(query)).unwrap();
    let rows: Vec<(&str, &str)> = listing
        .rows
        .iter()
        .map(|r| (r.project.id.as_str(), r.task.harp_id.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("alpha-proj", "alpha-two"),
            ("alpha-proj", "brisk-three"),
            ("zulu-proj", "zesty-one")
        ]
    );
    assert_eq!(
        listing.reach,
        Reach::Every {
            projects: 2,
            fallback: None
        },
        "the project with no store is not counted"
    );
    assert_eq!(listing.project(), None);
    assert!(listing.reach.is_every());
}

#[test]
fn test_no_project_outside_a_work_root_reads_every_project_saying_where() {
    let mut service = with_two_projects();
    service.identity.rootless = Some("/tmp/somewhere".to_string());
    let listing = ready(service.list(ListQuery::default())).unwrap();
    assert_eq!(
        listing.reach,
        Reach::Every {
            projects: 2,
            fallback: Some("/tmp/somewhere".to_string())
        }
    );
    assert_eq!(listing.rows.len(), 3);
}

#[test]
fn test_a_project_found_is_read_alone_rootless_or_not() {
    let mut service = with_two_projects();
    service.identity.bound = Some(named("zulu-proj"));
    service.identity.rootless = Some("/tmp/somewhere".to_string());
    let listing = ready(service.list(ListQuery::default())).unwrap();
    assert_eq!(listing.reach, Reach::Project(named("zulu-proj")));
    assert_eq!(listing.rows.len(), 1);
    assert!(!listing.reach.is_every());
}

#[test]
fn test_a_global_read_whose_projects_cannot_be_named_fails_with_why() {
    let mut service = service(&[]);
    service.identity.fails = Some(annals_diagnostic::Report::new("x-registry", "no home"));
    let query = ListQuery {
        global: true,
        ..ListQuery::default()
    };
    assert_eq!(ready(service.list(query)).unwrap_err().code(), "x-registry");
}

#[test]
fn test_a_tag_query_over_every_project_tells_their_tasks_apart() {
    let mut service = with_two_projects();
    let query = ListQuery {
        global: true,
        tag_query: "x/not".to_string(),
        ..ListQuery::default()
    };
    assert_eq!(ready(service.list(query)).unwrap().rows.len(), 3);
}

// ------------------------------------------------------------ show

fn asked(ids: &[&str]) -> ShowQuery {
    ShowQuery {
        harp_ids: ids.iter().map(|s| s.to_string()).collect(),
        ..ShowQuery::default()
    }
}

#[test]
fn test_show_answers_with_each_task_asked_for_in_the_order_asked_whatever_its_status() {
    let mut service = with_every_status();
    let details = ready(service.show(asked(&["done", "to-do", "done"]))).unwrap();
    let ids: Vec<&str> = details
        .rows
        .iter()
        .map(|r| r.task.harp_id.as_str())
        .collect();
    assert_eq!(ids, ["done", "to-do", "done"], "a repeated id twice");
    assert_eq!(details.rows[0].task.text, "Done");
    assert_eq!(details.reach, Reach::Project(crate::fakes::project()));
}

#[test]
fn test_show_fails_naming_every_id_no_task_holds() {
    let mut service = with_every_status();
    let error = ready(service.show(asked(&["nope-a", "done", "nope-b"]))).unwrap_err();
    assert_eq!(
        error,
        AppError::NotFound {
            harp_ids: vec!["nope-a".to_string(), "nope-b".to_string()]
        }
    );
    assert!(
        error.to_string().contains("\"nope-a\", \"nope-b\""),
        "{error}"
    );
}

#[test]
fn test_show_across_projects_fails_on_an_id_two_hold_naming_them() {
    let mut service = service(&["swift-amber", "swift-amber", "calm-otter", "swift-amber"]);
    for id in ["alpha-proj", "zulu-proj"] {
        service
            .stores
            .others
            .insert(id.to_string(), Flaky::default());
    }
    service.identity.bound = Some(named("alpha-proj"));
    add(&mut service, "in alpha", "");
    add(&mut service, "in alpha too", "");
    service.identity.bound = Some(named("zulu-proj"));
    add(&mut service, "in zulu", "");
    service.identity.every = vec![named("alpha-proj"), named("zulu-proj")];
    let global = ShowQuery {
        global: true,
        ..asked(&["swift-amber", "calm-otter"])
    };
    let error = ready(service.show(global.clone())).unwrap_err();
    assert_eq!(
        error,
        AppError::Ambiguous {
            held: vec![(
                "swift-amber".to_string(),
                vec!["alpha-proj".to_string(), "zulu-proj".to_string()]
            )]
        }
    );
    assert!(
        error
            .to_string()
            .contains("\"swift-amber\" in alpha-proj, zulu-proj"),
        "{error}"
    );
    let one = ShowQuery {
        harp_ids: vec!["calm-otter".to_string()],
        ..global
    };
    let details = ready(service.show(one)).unwrap();
    assert_eq!(details.rows[0].project.id, "alpha-proj");
    let here = ready(service.show(asked(&["swift-amber"]))).unwrap();
    assert_eq!(here.rows[0].task.text, "in zulu", "this project's alone");
}

#[test]
fn test_show_where_there_is_no_project_finds_nothing() {
    let mut service = service(&[]);
    let error = ready(service.show(asked(&["swift-amber"]))).unwrap_err();
    assert_eq!(error.code(), "tasks-application-not-found");
}

// ------------------------------------------------------------ summary

#[test]
fn test_a_summary_counts_every_task_by_status_and_names_those_in_progress() {
    let mut service = with_every_status();
    service.names = crate::fakes::FakeNames::of(&["also-doing"]);
    add(&mut service, "Second", "In Progress");
    let summary = ready(service.summary(Scope::default())).unwrap();
    let counts: Vec<(&str, usize)> = summary
        .counts
        .iter()
        .map(|(s, n)| (s.as_str(), *n))
        .collect();
    assert_eq!(
        counts,
        [
            ("Archived", 1),
            ("Blocked", 1),
            ("Deferred", 1),
            ("Done", 1),
            ("In Progress", 2),
            ("To Do", 1)
        ]
    );
    let doing: Vec<&str> = summary
        .in_progress
        .iter()
        .map(|r| r.task.harp_id.as_str())
        .collect();
    assert_eq!(doing, ["doing", "also-doing"], "in the order added");
    assert_eq!(summary.reach, Reach::Project(crate::fakes::project()));
}

#[test]
fn test_a_summary_across_projects_sums_them() {
    let mut service = with_two_projects();
    let global = Scope {
        global: true,
        ..Scope::default()
    };
    let summary = ready(service.summary(global)).unwrap();
    assert_eq!(summary.counts.get("To Do"), Some(&3));
    assert!(summary.reach.is_every());
}
