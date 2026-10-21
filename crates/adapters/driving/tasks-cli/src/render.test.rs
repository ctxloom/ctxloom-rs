//! The shapes answers are rendered in.

use std::path::PathBuf;

use tasks_application::{Hidden, Listing, Project, Reach, Row, Shown, Task, Timestamp};

use presentation::{render, write_json, Format};

use super::{table, Added, Listed, Tagged, TaskJson};

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
fn test_an_empty_table_says_so() {
    assert_eq!(
        rendered(|out| table(out, &[], &Shown::default())),
        "(no tasks)\n"
    );
}

#[test]
fn test_a_table_aligns_ids_and_statuses_and_separates_tasks() {
    let mut deferred = task("calm-otter", "Deferred");
    deferred.trigger = "the release".to_string();
    deferred.tags = vec![tag("a"), tag("b")];
    let (done, deferred) = (task("swift-amber-x", "Done"), deferred);
    let tasks = [&done, &deferred];
    assert_eq!(
        rendered(|out| table(out, &tasks, &Shown::default())),
        "[x] swift-amber-x  Done         Ship it\n\
         \n\
         [ ] calm-otter     Deferred     Ship it  (trigger: the release)  [a, b]\n"
    );
}

#[test]
fn test_a_table_aligns_wide_characters_by_their_display_width() {
    let (wide, narrow) = (
        task("swift-amber", "\u{5b8c}\u{4e86}"),
        task("calm-otter", "To Do"),
    );
    let tasks = [&wide, &narrow];
    assert_eq!(
        rendered(|out| table(out, &tasks, &Shown::default())),
        "[ ] swift-amber  \u{5b8c}\u{4e86}         Ship it\n\
         \n\
         [ ] calm-otter   To Do        Ship it\n"
    );
}

fn listing_none() -> Listing {
    listing(Reach::NoProject, &[], Shown::default())
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
    tagged.tags = vec![tag("a"), tag("b")];
    assert_eq!(
        rendered(|out| render(out, Format::Text, &Added(&tagged))),
        "swift-amber\tTo Do\tShip it\n"
    );
    assert_eq!(
        rendered(|out| render(out, Format::Text, &Tagged(&tagged, &Shown::default()))),
        "swift-amber\tTo Do\ta,b\n"
    );
    let json = rendered(|out| write_json(out, &TaskJson::of(&tagged)));
    for view in [
        rendered(|out| render(out, Format::Json, &Added(&tagged))),
        rendered(|out| render(out, Format::Json, &Tagged(&tagged, &Shown::default()))),
    ] {
        assert_eq!(view, json);
    }
}

fn listing(reach: Reach, tasks: &[Task], shown: Shown) -> Listing {
    Listing {
        reach,
        rows: tasks
            .iter()
            .map(|task| Row {
                project: project(),
                task: task.clone(),
            })
            .collect(),
        shown,
        hidden: Hidden::default(),
        searched: false,
        omitted: 0,
        priorities: None,
        priority_notes: Vec::new(),
    }
}

#[test]
fn test_a_listing_sorted_by_priority_gives_each_task_its_priority_in_json() {
    let tasks = [task("swift-amber", "To Do"), task("calm-otter", "To Do")];
    let mut sorted = listing(Reach::Project(project()), &tasks, Shown::default());
    let unsorted = rendered(|out| {
        render(
            out,
            Format::Json,
            &Listed {
                listing: &sorted,
                compact: false,
            },
        )
    });
    assert_eq!(
        unsorted.matches(r#""derived_priority": null"#).count(),
        2,
        "{unsorted}"
    );
    sorted.priorities = Some(vec![5.0, 2.5]);
    let json = rendered(|out| {
        render(
            out,
            Format::Json,
            &Listed {
                listing: &sorted,
                compact: false,
            },
        )
    });
    assert!(json.contains(r#""derived_priority": 5"#), "{json}");
    assert!(json.contains(r#""derived_priority": 2.5"#), "{json}");
    let compact = rendered(|out| {
        render(
            out,
            Format::Json,
            &Listed {
                listing: &sorted,
                compact: true,
            },
        )
    });
    assert!(
        !compact.contains("derived_priority"),
        "Go's compact rows have none"
    );
}

#[test]
fn test_a_listing_names_its_project_in_text_and_is_an_array_in_json() {
    let tasks = [task("swift-amber", "To Do")];
    let listing = listing(Reach::Project(project()), &tasks, Shown::default());
    let listed = Listed {
        listing: &listing,
        compact: false,
    };
    assert_eq!(
        rendered(|out| render(out, Format::Text, &listed)),
        "Project: /work/tree (jolly-mute-magma)\n\n[ ] swift-amber  To Do        Ship it\n"
    );
    let json: serde_json::Value =
        serde_json::from_str(&rendered(|out| render(out, Format::Json, &listed))).unwrap();
    assert_eq!(json[0]["harp_id"], "swift-amber");
    assert_eq!(json[0].get("project_id"), None, "one project: named once");
    let nowhere = listing_none();
    let listed = Listed {
        listing: &nowhere,
        compact: false,
    };
    assert_eq!(
        rendered(|out| render(out, Format::Text, &listed)),
        "(no tasks)\n"
    );
    assert_eq!(rendered(|out| render(out, Format::Json, &listed)), "[]\n");
}

#[test]
fn test_a_listing_across_projects_has_a_table_for_each() {
    let other = Project {
        id: "brisk-tall-oak".to_string(),
        root: String::new(),
        store: PathBuf::new(),
    };
    let mut listing = listing(
        Reach::Every {
            projects: 2,
            fallback: None,
        },
        &[task("swift-amber", "To Do"), task("calm-otter", "To Do")],
        Shown::default(),
    );
    listing.rows[1].project = other;
    let listed = Listed {
        listing: &listing,
        compact: false,
    };
    assert_eq!(
        rendered(|out| render(out, Format::Text, &listed)),
        "Projects: 2 (--global)\n\n\
         Project: /work/tree (jolly-mute-magma)\n\
         [ ] swift-amber  To Do        Ship it\n\n\
         Project: brisk-tall-oak\n\
         [ ] calm-otter  To Do        Ship it\n\n"
    );
    let json: serde_json::Value =
        serde_json::from_str(&rendered(|out| render(out, Format::Json, &listed))).unwrap();
    assert_eq!(json[1]["project_id"], "brisk-tall-oak");
    assert_eq!(json[1]["project_dir"], "");
    assert_eq!(json[1]["text_hash"], "0123456789ab", "the task whole");
}

fn tag(key: &str) -> tasks_application::Tag {
    tasks_application::Tag {
        namespace: None,
        key: key.to_string(),
        value: None,
    }
}

/// What `hide` facts show.
fn hiding(facts: &[&str]) -> Shown {
    Shown::of(&tasks_application::TagSchema::parse(facts).unwrap())
}

fn spelled_tag(s: &str) -> tasks_application::Tag {
    tasks_application::parse_tag(s).unwrap().unwrap()
}

// Go's TestTagCmd_TextHidesConfiguredTagJSONKeepsIt: a hidden tag is left
// out of what a person reads, and kept in JSON, which a program reads.
#[test]
fn test_a_hidden_tag_is_left_out_of_text_and_kept_in_json() {
    let mut tagged = task("swift-amber", "To Do");
    tagged.tags = ["triage:cwe=79", "triage:kind=defect", "x"]
        .iter()
        .map(|t| spelled_tag(t))
        .collect();
    let shown = hiding(&[r#"tagma.hide:"triage:cwe"=true"#]);
    assert_eq!(
        rendered(|out| render(out, Format::Text, &Tagged(&tagged, &shown))),
        "swift-amber\tTo Do\ttriage:kind=defect,x\n"
    );
    let tasks = [&tagged];
    assert_eq!(
        rendered(|out| table(out, &tasks, &shown)),
        "[ ] swift-amber  To Do        Ship it  [triage:kind=defect, x]\n"
    );
    let json: serde_json::Value = serde_json::from_str(&rendered(|out| {
        render(out, Format::Json, &Tagged(&tagged, &shown))
    }))
    .unwrap();
    assert_eq!(
        json["tags"],
        serde_json::json!(["triage:cwe=79", "triage:kind=defect", "x"])
    );
    let listing = listing(Reach::NoProject, &[tagged.clone()], shown.clone());
    let listed = Listed {
        listing: &listing,
        compact: false,
    };
    let json: serde_json::Value =
        serde_json::from_str(&rendered(|out| render(out, Format::Json, &listed))).unwrap();
    assert_eq!(json[0]["tags"].as_array().unwrap().len(), 3);
    // Every tag hidden: no brackets at all.
    let all = hiding(&[r#"tagma.hide:"*:*"=true"#, "tagma.hide:x=true"]);
    assert_eq!(
        rendered(|out| table(out, &tasks, &all)),
        "[ ] swift-amber  To Do        Ship it\n"
    );
}
