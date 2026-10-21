//! Go's flat names, and its records read for the one fact they are read
//! for. The names and records are Go's own: written by Go taskloom's
//! `manage install` (fixtures/go-records, the home as `$HOME`).

use std::path::Path;

use super::{added, flat_name};

/// Go's record `fixture`, for the file at `target`, in `dir` under the
/// name Go gives it.
fn lay(dir: &Path, fixture: &str, target: &str, stamp: &str) {
    let text = std::fs::read_to_string(format!(
        "{}/fixtures/go-records/{fixture}.hew-record.yaml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .replace("$HOME/.claude.json", target);
    let name = format!("{}__{stamp}.hew-record.yaml", flat_name(target));
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn test_a_record_says_whether_go_added_the_servers_object_whole() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(".claude.json").display().to_string();
    assert!(!added(dir.path(), &target, "/mcpServers"), "no record");
    lay(
        dir.path(),
        "indented-no-servers",
        &target,
        "20261021T070716.759601638Z",
    );
    assert!(added(dir.path(), &target, "/mcpServers"));
    assert!(!added(dir.path(), &target, "/mcpServers/taskloom"));
    assert!(!added(dir.path(), "/elsewhere/.claude.json", "/mcpServers"));
}

#[test]
fn test_the_newest_record_is_the_one_read() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(".claude.json").display().to_string();
    lay(
        dir.path(),
        "indented-no-servers",
        &target,
        "20261021T070716.759601638Z",
    );
    // A later application, into servers that were there already.
    let text = std::fs::read_to_string(format!(
        "{}/fixtures/go-records/indented-other-server.hew-record.yaml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .replace("$HOME/.claude.json", &target)
    .replace("2026-10-21T07:07:16.774018621Z", "2026-10-22T00:00:00Z");
    let name = format!(
        "{}__20261022T000000.000000000Z.hew-record.yaml",
        flat_name(&target)
    );
    std::fs::write(dir.path().join(name), text).unwrap();
    assert!(!added(dir.path(), &target, "/mcpServers"));
    assert!(added(dir.path(), &target, "/mcpServers/taskloom"));
}

#[test]
fn test_a_record_that_cannot_be_read_says_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(".claude.json").display().to_string();
    let name = format!("{}__x.hew-record.yaml", flat_name(&target));
    std::fs::write(dir.path().join(name), "{ not: [yaml").unwrap();
    assert!(!added(dir.path(), &target, "/mcpServers"));
    assert!(!added(&dir.path().join("absent"), &target, "/mcpServers"));
}

#[test]
fn test_a_flat_name_keeps_exactly_the_last_ninety_six_bytes() {
    let at = |n: usize| flat_name(&"a".repeat(n));
    assert_eq!(at(96).len(), 96 + 18);
    assert_eq!(at(97).len(), 96 + 18);
    assert!(at(96).starts_with(&"a".repeat(96)));
    let name = flat_name(&format!("b{}", "\u{e9}".repeat(48)));
    assert_eq!(
        &name[..name.len() - 18],
        "\u{e9}".repeat(48),
        "cut forward to a character"
    );
    assert!(flat_name(" ").starts_with(" __"), "a space is kept");
}

#[test]
fn test_only_a_record_of_this_file_and_this_kind_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join(".claude.json").display().to_string();
    lay(
        dir.path(),
        "indented-no-servers",
        &target,
        "20261021T070716.759601638Z",
    );
    let named = std::fs::read_dir(dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let other = named.with_extension("yaml.bak");
    std::fs::rename(&named, &other).unwrap();
    assert!(
        !added(dir.path(), &target, "/mcpServers"),
        "not a record's suffix"
    );
}
