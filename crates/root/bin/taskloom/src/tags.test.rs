//! Acceptance: tags through the real binary, its configuration read from
//! a temporary home and project, never a real one.

use std::fs;
use std::path::Path;

use super::Env;

fn configure(dir: &Path, yaml: &str) {
    fs::create_dir_all(dir.join(".taskloom")).unwrap();
    fs::write(dir.join(".taskloom/config.yaml"), yaml).unwrap();
}

/// The `code` of the JSON error envelope on stderr.
fn code(stderr: &str) -> String {
    let envelope: serde_json::Value = serde_json::from_str(stderr)
        .unwrap_or_else(|e| panic!("not an envelope ({e}): {stderr:?}"));
    envelope["code"].as_str().unwrap().to_string()
}

#[test]
fn test_the_schema_comes_from_every_layer_the_last_winning() {
    let env = Env::new("calm-otter-fox");
    let add = |tag: &str, vars: &[(&str, &str)], set: &[&str]| {
        let mut args = vec!["add", "Ship it", "--json", "--tag", tag];
        for s in set {
            args.extend(["--config-set", s]);
        }
        env.run_with(&args, vars)
    };

    // The default schema: triage:level is 1-5.
    let ran = add("triage:level=7", &[], &[]);
    assert!(!ran.ok);
    assert_eq!(code(&ran.stderr), "tasks-application-tag-out-of-range");

    // The home widens it; the project replaces the home's list.
    configure(
        env.home.path(),
        "tag_schema:\n  - 'tagma.range:\"triage:level\"=\"1,9\"'\n",
    );
    assert!(add("triage:level=7", &[], &[]).ok, "the home's range");
    configure(
        env.project.path(),
        "tag_schema:\n  - 'tagma.enum:\"area:x\"=\"a,b\"'\n",
    );
    let ran = add("triage:level=7", &[], &[]);
    assert_eq!(
        code(&ran.stderr),
        "tasks-application-tag-out-of-range",
        "home replaced"
    );
    assert_eq!(
        code(&add("area:x=c", &[], &[]).stderr),
        "tasks-application-tag-not-in-enum"
    );
    assert!(add("area:x=b", &[], &[]).ok);

    // The environment beats the files, and --config-set the environment.
    let wide = [(
        "TASKLOOM_CONFIG_TAG_SCHEMA",
        r#"tagma.range:"triage:level"="1,9""#,
    )];
    assert!(add("triage:level=8", &wide, &[]).ok);
    assert!(
        add("area:x=c", &wide, &[]).ok,
        "the project's list replaced"
    );
    let narrow = r#"tag_schema=tagma.range:"triage:level"="1,2""#;
    let ran = add("triage:level=8", &wide, &[narrow]);
    assert_eq!(code(&ran.stderr), "tasks-application-tag-out-of-range");
}

#[test]
fn test_a_malformed_configuration_refuses_the_command_and_says_where() {
    let env = Env::new("calm-otter-fox");
    configure(
        env.project.path(),
        "tag_schema:\n  - 'tagma.arty:\"x:y\"=1'\n",
    );
    let ran = env.run(&["add", "Ship it", "--json"]);
    assert!(!ran.ok);
    assert_eq!(code(&ran.stderr), "tasks-application-schema-unknown-facet");
    configure(env.project.path(), "homeing: home\n");
    let ran = env.run(&["add", "Ship it"]);
    assert!(!ran.ok);
    assert!(
        ran.stderr.contains(".taskloom/config.yaml"),
        "{}",
        ran.stderr
    );
    configure(env.project.path(), "homing: home\n");
    let ran = env.run(&["add", "Ship it", "--json", "--config-set", "nonsense"]);
    assert_eq!(code(&ran.stderr), "tasks-config-not-an-override");
}

#[test]
fn test_a_scalar_tag_displaces_its_old_value() {
    let env = Env::new("calm-otter-fox");
    let task = env.seed("Ship it");
    let harp = task["harp_id"].as_str().unwrap();
    for level in ["triage:level=3", "triage:level=1"] {
        let ran = env.run(&["tag", harp, "--add", level, "--add", "x", "--json"]);
        assert!(ran.ok, "{}", ran.stderr);
    }
    let listed = env.list();
    assert_eq!(
        listed[0]["tags"],
        serde_json::json!(["triage:level=1", "x"])
    );
}

#[test]
fn test_list_selects_by_a_tag_query_postfix_or_infix() {
    let env = Env::new("calm-otter-fox");
    for (text, tag) in [
        ("one", "urgent"),
        ("two", "release"),
        ("three", "area:cli=x"),
    ] {
        let ran = env.run(&["add", text, "--tag", tag, "--json"]);
        assert!(ran.ok, "{}", ran.stderr);
    }
    let texts = |query: &str| -> Vec<String> {
        let ran = env.run(&["list", "--json", "--tag-query", query]);
        assert!(ran.ok, "{query}: {}", ran.stderr);
        ran.json()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["text"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(texts("urgent/release/or"), ["one", "two"]);
    assert_eq!(texts("urgent or release"), ["one", "two"]);
    assert_eq!(texts("urgent/not"), ["two", "three"]);
    assert_eq!(texts("*:cli=+"), ["three"]);
    let ran = env.run(&["list", "--json", "--tag-query", "urgent/and"]);
    assert!(!ran.ok);
    assert_eq!(code(&ran.stderr), "tasks-application-tag-query");
}

// Go's TestTagCmd_TextHidesConfiguredTagJSONKeepsIt, through the binary.
#[test]
fn test_a_tag_the_schema_hides_is_left_out_of_text_and_kept_in_json() {
    let env = Env::new("calm-otter-fox");
    configure(
        env.project.path(),
        "tag_schema:\n  - 'tagma.hide:\"triage:cwe\"=true'\n",
    );
    let ran = env.run(&[
        "add",
        "Ship it",
        "--tag",
        "triage:cwe=79",
        "--tag",
        "x",
        "--json",
    ]);
    assert!(ran.ok, "{}", ran.stderr);
    let harp = ran.json()["harp_id"].as_str().unwrap().to_string();
    let text = env.run(&["list", "--format", "text"]);
    assert!(text.stdout.contains("Ship it  [x]"), "{}", text.stdout);
    let tagged = env.run(&["tag", &harp, "--add", "y", "--format", "text"]);
    assert_eq!(tagged.stdout, format!("{harp}\tTo Do\tx,y\n"));
    assert_eq!(
        env.list()[0]["tags"],
        serde_json::json!(["triage:cwe=79", "x", "y"])
    );
}
