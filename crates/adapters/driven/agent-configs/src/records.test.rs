//! Go's flat names, and its records read for the one fact they are read
//! for. The names and records are Go's own: written by Go taskloom's
//! `manage install` (fixtures/go-records, the home as `$HOME`).

use std::path::Path;

use super::{added, flat_name};

const SCRATCH: &str =
    "/tmp/claude-1000/-home-babbitt-workspace-ctxloom-ctxloom-rs/74fb18c8-0651-4a53-83c8-19f8a2e256cc/scratchpad";

#[test]
fn test_a_flat_name_is_gos() {
    assert_eq!(
        flat_name(&format!("{SCRATCH}/tmp.L0iKUu4t5Y/home/.claude.json")),
        "ctxloom-rs__74fb18c8-0651-4a53-83c8-19f8a2e256cc__scratchpad__tmp.L0iKUu4t5Y__home__.claude.json__d07667d34aae78ed"
    );
    assert_eq!(
        flat_name(&format!("{SCRATCH}/tmp.qBAVn0XNgO/home/.claude.json")),
        "ctxloom-rs__74fb18c8-0651-4a53-83c8-19f8a2e256cc__scratchpad__tmp.qBAVn0XNgO__home__.claude.json__1865b30e2a6a57b4"
    );
}

#[test]
fn test_a_flat_name_escapes_what_ntfs_refuses_and_cuts_on_a_character() {
    let name = flat_name("C:/a|b\u{1}");
    assert!(name.starts_with("C%3A__a%7Cb%01__"), "{name}");
    assert_eq!(name.len(), "C%3A__a%7Cb%01__".len() + 16);
    let deep = format!("/{}", "\u{e9}".repeat(60));
    let name = flat_name(&deep);
    let tail = &name[..name.len() - 18];
    assert!(tail.len() <= 96, "{}", tail.len());
    assert!(tail.chars().all(|c| c == '\u{e9}'), "{tail}");
}

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
