//! Acceptance: `taskloom prune` through the real binary over SQLite, in a
//! temporary home: a report changes nothing, `--yes` removes and purges,
//! and each answer is one JSON document as jq reads it.

use super::{one_document, Env};

/// Add `args` as a task; its harp.
fn add(env: &Env, args: &[&str]) -> String {
    let mut args = [&["add"][..], args, &["--json"]].concat();
    args.retain(|a| !a.is_empty());
    let added = env.run(&args);
    assert!(added.ok, "{}", added.stderr);
    one_document(&added.stdout)["harp_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn every_harp(env: &Env) -> Vec<String> {
    let listed = env.run(&["list", "--all", "--json"]);
    assert!(listed.ok, "{}", listed.stderr);
    one_document(&listed.stdout)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["harp_id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn test_a_prune_reports_then_with_yes_removes_and_purges_and_names_what_dangles() {
    let env = Env::new("prune-harp");
    let done = add(&env, &["Finished", "--status", "Done"]);
    let keeper = add(
        &env,
        &["Follows", &done, "--tag", &format!("relates:{done}")],
    );
    let waiter = add(
        &env,
        &[
            "Waiting",
            "--status",
            "Deferred",
            "--trigger",
            &format!("when {done} ships"),
        ],
    );

    // A cut of a nanosecond back takes every Done task already recorded.
    let report = env.run(&["prune", "--older-than", "1ns", "--json"]);
    assert!(report.ok, "{}", report.stderr);
    let document = one_document(&report.stdout);
    assert_eq!(document["applied"], false);
    assert_eq!(document["statuses"], serde_json::json!(["Done"]));
    assert_eq!(document["pruned"][0]["harp_id"], done.as_str());
    assert_eq!(document["tasks_pruned"], 1);
    assert_eq!(document["events_removed"], 2, "its add, and the removal");
    assert_eq!(document["tasks_remaining"], 2);
    let dangling: Vec<(String, String, String)> = document["dangling_references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let field = |name: &str| d[name].as_str().unwrap().to_string();
            (field("harp_id"), field("pruned"), field("via"))
        })
        .collect();
    assert_eq!(
        dangling,
        [
            (keeper.clone(), done.clone(), format!("tag relates:{done}")),
            (keeper.clone(), done.clone(), "text".to_string()),
            (waiter.clone(), done.clone(), "trigger".to_string()),
        ]
    );
    assert!(
        report
            .stderr
            .contains("removed nothing: this was a report, not a removal"),
        "{}",
        report.stderr
    );
    assert!(
        report
            .stderr
            .contains("taskloom prune --older-than 1ns --status Done --yes"),
        "{}",
        report.stderr
    );
    assert!(every_harp(&env).contains(&done), "a report changes nothing");

    let applied = env.run(&["prune", "--older-than", "1ns", "--yes", "--json"]);
    assert!(applied.ok, "{}", applied.stderr);
    let document = one_document(&applied.stdout);
    assert_eq!(document["applied"], true);
    assert_eq!(document["pruned"][0]["harp_id"], done.as_str());
    assert_eq!(
        document["events_removed"], 2,
        "as the store reports the purge"
    );
    assert_eq!(applied.stderr, "", "nothing to say after taking it");
    let left = every_harp(&env);
    assert!(!left.contains(&done), "{left:?}");
    assert_eq!(left.len(), 2);

    let again = env.run(&["prune", "--older-than", "1ns", "--format", "text"]);
    assert!(again.ok, "{}", again.stderr);
    assert!(
        again.stdout.contains("Nothing to prune (status Done, "),
        "{}",
        again.stdout
    );
}

#[test]
fn test_a_prune_where_there_is_no_project_creates_nothing() {
    let env = Env::new("prune-nowhere-harp");
    let ran = env.run(&["prune", "--yes", "--json"]);
    assert!(ran.ok, "{}", ran.stderr);
    assert_eq!(one_document(&ran.stdout)["pruned"], serde_json::json!([]));
    assert_eq!(
        ran.stderr,
        "taskloom: no project here yet: nothing to prune\n"
    );
    assert!(!env.home.path().join(".ctxloom").exists());
    assert!(!env.project.path().join(".project-id").exists());
}

#[test]
fn test_a_refused_prune_is_an_envelope_and_leaves_stdout_empty() {
    let env = Env::new("prune-refused-harp");
    let ran = env.run(&["prune", "--older-than", "soon", "--json"]);
    assert!(!ran.ok);
    assert_eq!(ran.stdout, "");
    let envelope = one_document(&ran.stderr);
    assert_eq!(envelope["code"], "tasks-application-prune-age-unreadable");
    assert_eq!(envelope["args"]["age"], "soon");
}
