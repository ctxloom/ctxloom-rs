//! `version.feature`: `version --format json` is `{name, version,
//! lock_protocol}`, and every spelling of version agrees.

use super::Env;
use crate::tests::files;

/// What ctxloom's lock reaper requires of a running binary, at least.
const LOCK_PROTOCOL_FLOOR: u64 = 2;

#[test]
fn test_version_in_json_is_name_version_and_lock_protocol() {
    let env = Env::new();
    let ran = env.run(&["version", "--format", "json"]);
    assert_eq!(ran.code, 0, "{}", ran.said());
    let doc = ran.json();
    let object = doc.as_object().expect("an object");
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, ["lock_protocol", "name", "version"], "{doc}");
    assert_eq!(doc["name"], "ctxloom");
    assert_eq!(doc["version"], env!("CARGO_PKG_VERSION"));
    let protocol = doc["lock_protocol"].as_u64().expect("an integer");
    assert!(protocol >= LOCK_PROTOCOL_FLOOR, "{doc}");
    assert_eq!(ran.stderr, "");
    let written = files::fs().read_dir(env.home()).unwrap();
    assert!(written.is_empty(), "the probe writes nothing: {written:?}");
}

#[test]
fn test_every_spelling_of_version_agrees() {
    let env = Env::new();
    let version = env!("CARGO_PKG_VERSION");

    // The VS Code extension's probe: Go's cobra line.
    let flag = env.run(&["--version"]);
    assert_eq!(flag.code, 0, "{}", flag.said());
    assert_eq!(flag.stdout, format!("ctxloom version {version}\n"));
    assert_eq!(flag.stderr, "");

    let text = env.run(&["version", "--format", "text"]);
    assert_eq!(text.code, 0, "{}", text.said());
    assert_eq!(text.stdout, format!("{version}\n"));

    let json = env.run(&["version", "--format", "json"]);
    assert_eq!(json.code, 0, "{}", json.said());
    assert_eq!(json.json()["version"], version);

    // Off a terminal the format is json: bare `version` is the same document.
    let bare = env.run(&["version"]);
    assert_eq!(bare.code, 0, "{}", bare.said());
    assert_eq!(bare.json(), json.json());
}

#[test]
fn test_v_is_accepted_globally_and_changes_no_output() {
    let env = Env::new();
    let plain = env.run(&["version", "--format", "json"]);
    assert_eq!(plain.code, 0, "{}", plain.said());
    let with_v = env.run(&["-v", "version", "--format", "json"]);
    assert_eq!(with_v.code, 0, "{}", with_v.said());
    assert_eq!(with_v.json()["name"], "ctxloom", "{}", with_v.said());
    assert_eq!(with_v.stdout, plain.stdout);
    assert_eq!(with_v.stderr, "");
}
