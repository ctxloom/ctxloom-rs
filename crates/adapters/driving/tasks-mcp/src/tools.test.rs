//! Each tool's arguments as the request they make, and each answer as
//! the result it gives.

use std::path::PathBuf;

use serde_json::json;
use tasks_application::{
    Hidden, ListQuery, Listing, Project, Reach, Row, Shown, Task, Timestamp, Written,
};

use super::{added, list_query, listed, new_task, offered};
use crate::{McpError, Settings};

fn settings() -> Settings {
    Settings {
        session: "calm-otter-fox".to_string(),
        ..Settings::default()
    }
}

fn project() -> Project {
    Project {
        id: "jolly-mute-magma".to_string(),
        root: "/work/tree".to_string(),
        store: PathBuf::from("/s/state.db"),
    }
}

fn task() -> Task {
    Task {
        harp_id: "swift-amber".to_string(),
        text: "Ship it".to_string(),
        status: "To Do".to_string(),
        checked: false,
        text_hash: "0123456789ab".to_string(),
        trigger: String::new(),
        origin_session: String::new(),
        tags: vec![tag("a")],
        created_at: Timestamp {
            seconds: 1_781_058_920,
            nanos: 0,
        },
    }
}

fn tag(spelled: &str) -> tasks_application::Tag {
    tasks_application::parse_tag(spelled).unwrap().unwrap()
}

/// What `hide` facts show.
fn hiding(facts: &[&str]) -> Shown {
    Shown::of(&tasks_application::TagSchema::parse(facts).unwrap())
}

/// A listing that read `reach`, of `tasks` in `project()`.
fn listing(reach: Reach, tasks: Vec<Task>, shown: Shown) -> Listing {
    let rows = tasks
        .into_iter()
        .map(|task| Row {
            project: project(),
            task,
        })
        .collect();
    Listing {
        reach,
        rows,
        shown,
        hidden: Hidden::default(),
        searched: false,
        omitted: 0,
    }
}

fn catalog() -> presentation::Catalog {
    presentation::Catalog::en_us(&[crate::EN_US]).unwrap()
}

#[test]
fn test_the_tools_offered_are_task_add_and_task_list() {
    let names: Vec<String> = offered().into_iter().map(|t| t.name.into_owned()).collect();
    assert_eq!(names, ["task_add", "task_list"]);
}

#[test]
fn test_a_listing_asks_for_its_statuses_and_whether_to_include_the_hidden() {
    let query = list_query(
        json!({"statuses": ["To Do", "Done"], "include_completed": true}),
        &settings(),
    )
    .unwrap();
    assert_eq!(
        query,
        ListQuery {
            project: None,
            statuses: vec!["To Do".to_string(), "Done".to_string()],
            all: true,
            ..ListQuery::default()
        }
    );
    assert_eq!(
        list_query(json!({}), &settings()).unwrap(),
        ListQuery::default()
    );
}

#[test]
fn test_a_parameter_not_served_is_refused_unless_it_asks_for_nothing() {
    let asking_nothing = json!({
        "term": "", "tag_query": "", "include_summary": false, "global": false,
        "sort": "", "compact": false, "limit": 0,
    });
    assert!(list_query(asking_nothing, &settings()).is_ok());
    for (parameter, value) in [
        ("term", json!("x")),
        ("tag_query", json!("a/b/and")),
        ("include_summary", json!(true)),
        ("global", json!(true)),
        ("sort", json!("priority")),
        ("compact", json!(true)),
        ("limit", json!(5)),
    ] {
        let mut arguments = serde_json::Map::new();
        arguments.insert(parameter.to_string(), value);
        assert_eq!(
            list_query(arguments.into(), &settings()),
            Err(McpError::Unserved {
                tool: "task_list".to_string(),
                parameter: parameter.to_string(),
            })
        );
    }
}

#[test]
fn test_an_add_asks_for_its_task_in_the_servers_session() {
    let task = new_task(
        json!({"text": "Ship it", "status": "Deferred", "trigger": "later", "tags": ["a"]}),
        &settings(),
    )
    .unwrap();
    assert_eq!(task.text, "Ship it");
    assert_eq!(task.status, "Deferred");
    assert_eq!(task.trigger, "later");
    assert_eq!(task.tags, ["a"]);
    assert_eq!(task.origin_session, "calm-otter-fox");
    let bare = new_task(json!({"text": "x", "tags": null}), &settings()).unwrap();
    assert!(bare.tags.is_empty());
    assert!(matches!(
        new_task(json!({"text": 3}), &settings()),
        Err(McpError::Arguments { tool, .. }) if tool == "task_add"
    ));
}

#[test]
fn test_a_listed_task_carries_its_project_and_no_priority_yet() {
    let listing = listing(Reach::Project(project()), vec![task()], Shown::default());
    let value = listed(&listing, &catalog());
    assert_eq!(value["path"], "/s/state.db");
    assert_eq!(value.get("warning"), None);
    let row = &value["tasks"][0];
    assert_eq!(row["harp_id"], "swift-amber");
    assert_eq!(row["project_id"], "jolly-mute-magma");
    assert_eq!(row["project_dir"], "/work/tree");
    assert_eq!(row["derived_priority"], json!(null));
    assert_eq!(row["created_at"], "2026-06-10T02:35:20Z");
}

#[test]
fn test_an_added_task_is_answered_with_its_store_and_project() {
    let written = Written {
        project: project(),
        task: task(),
        shown: Shown::default(),
    };
    let value = added(&written);
    assert_eq!(value["path"], "/s/state.db");
    assert_eq!(value["project_id"], "jolly-mute-magma");
    assert_eq!(value["project_dir"], "/work/tree");
    assert_eq!(value["task"]["tags"], json!(["a"]));
    assert_eq!(value["task"]["derived_priority"], json!(null));
}

// A tool's result is read by a program, as the CLI's JSON is: each tag in
// its canonical spelling, and the tags a person is not shown kept.
#[test]
fn test_a_result_spells_every_tag_canonically_hidden_ones_too() {
    let mut task = task();
    task.tags = vec![tag("triage:cwe=79"), tag("x=\"y z\""), tag("a")];
    let shown = hiding(&[r#"tagma.hide:"triage:cwe"=true"#]);
    let spelled = json!(["triage:cwe=79", "x=\"y z\"", "a"]);
    let written = Written {
        project: project(),
        task: task.clone(),
        shown: shown.clone(),
    };
    assert_eq!(added(&written)["task"]["tags"], spelled);
    let listing = listing(Reach::Project(project()), vec![task], shown);
    assert_eq!(listed(&listing, &catalog())["tasks"][0]["tags"], spelled);
}

// Go falls back to every project where none is found outside a work
// root, and says so in `global` and `notice`; until that is served, such
// a listing is answered as no project, as it was before the fallback.
#[test]
fn test_a_listing_of_no_project_or_every_project_is_answered_as_no_project() {
    let every = Reach::Every {
        projects: 1,
        fallback: Some("/tmp/somewhere".to_string()),
    };
    for reach in [Reach::NoProject, every] {
        let value = listed(&listing(reach, vec![task()], Shown::default()), &catalog());
        assert_eq!(value["path"], "");
        assert_eq!(value.get("project_id"), None);
        assert_eq!(value["tasks"], json!([]));
        assert!(value["warning"].as_str().is_some_and(|w| !w.is_empty()));
    }
}
