use super::*;

const KEY: &str = "ctxloom+git://example.test/repo//bundles/kit";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn entry() -> Entry {
    Entry {
        sha: SHA.to_string(),
        url: "https://example.test/repo".to_string(),
        ..Entry::default()
    }
}

fn lock_of(entries: &[(&str, Entry)]) -> Lock {
    Lock {
        entries: entries
            .iter()
            .map(|(k, e)| (k.to_string(), e.clone()))
            .collect(),
    }
}

/// A lock as Go writes it: sha, url, requested_version, version, kind, held.
const GO_LOCK: &str = "schema_version: 3
bundles:
  ctxloom+git://example.test/repo//bundles/kit:
    sha: 0123456789abcdef0123456789abcdef01234567
    url: https://example.test/repo
    requested_version: ^1.2
    version: v1.3.0
    kind: version
    held: true
  ctxloom+git://example.test/repo//bundles/tools:
    sha: fedcba9876543210fedcba9876543210fedcba98
    url: https://example.test/repo
    kind: branch
";

// lockfile_test.go: SaveAndLoad. Plan 8.6: a Go-written lock is read, and
// rewritten byte for byte.
#[test]
fn test_a_lock_go_wrote_is_read_and_rewritten_byte_for_byte() {
    let lock = Lock::decode(GO_LOCK, "lock.yaml").expect("reads");
    assert_eq!(lock.entries.len(), 2);
    let kit = &lock.entries[KEY];
    assert_eq!(kit.sha, SHA);
    assert_eq!(kit.url, "https://example.test/repo");
    assert_eq!(kit.requested_version, "^1.2");
    assert_eq!(kit.version, "v1.3.0");
    assert_eq!(kit.kind, Some(Kind::Version));
    assert!(kit.held);
    assert_eq!(
        lock.entries["ctxloom+git://example.test/repo//bundles/tools"].kind,
        Some(Kind::Branch)
    );
    assert_eq!(lock.encode(), GO_LOCK);
}

#[test]
fn test_the_lock_is_written_in_a_fixed_order_and_omits_what_is_empty() {
    let lock = lock_of(&[
        ("ctxloom+git://h/r//bundles/b", entry()),
        ("ctxloom+git://h/r//bundles/a", entry()),
    ]);
    assert_eq!(
        lock.encode(),
        format!(
            "schema_version: 3\nbundles:\n  ctxloom+git://h/r//bundles/a:\n    sha: {SHA}\n    url: https://example.test/repo\n  ctxloom+git://h/r//bundles/b:\n    sha: {SHA}\n    url: https://example.test/repo\n"
        )
    );
    assert_eq!(Lock::default().encode(), "schema_version: 3\n");
}

// lockfile_test.go: an unchanged lock reads and rewrites identically (the lock
// records no time), whatever its entries.
#[test]
fn test_a_lock_round_trips_through_its_own_text() {
    let mut held = entry();
    held.held = true;
    held.kind = Some(Kind::Tag);
    held.version = "v1".to_string();
    held.requested_version = "v1".to_string();
    let lock = lock_of(&[(KEY, held)]);
    let text = lock.encode();
    let again = Lock::decode(&text, "lock.yaml").expect("reads");
    assert_eq!(again, lock);
    assert_eq!(again.encode(), text);
}

// lockfile_schemaver_test.go: a generation-2 lock still carries the two times
// generation 3 dropped; migrating removes them and keeps pins and holds.
#[test]
fn test_a_generation_two_lock_loses_its_times_and_keeps_its_pins_and_holds() {
    let v2 = format!(
        "schema_version: 2\nlocked_at: 2026-10-01T12:00:00Z\nbundles:\n  {KEY}:\n    sha: abc123\n    fetched_at: 2026-09-30T08:00:00Z\n    held: true\n"
    );
    let lock = Lock::decode(&v2, "lock.yaml").expect("migrates");
    assert_eq!(
        lock.entries[KEY],
        Entry {
            sha: "abc123".to_string(),
            held: true,
            ..Entry::default()
        }
    );
    // Go's struct marshal writes `sha` and `url` whatever they hold.
    assert_eq!(
        lock.encode(),
        format!("schema_version: 3\nbundles:\n  {KEY}:\n    sha: abc123\n    url: \"\"\n    held: true\n")
    );
}

// lockfile_schemaver_test.go: newer is refused naming both numbers; keyless
// and `version:` are not the generation (unstamped is 0); older than 2 too.
#[test]
fn test_a_lock_of_another_generation_is_refused_naming_it() {
    let newer = Lock::decode(
        &format!("schema_version: 4\nbundles:\n  {KEY}:\n    sha: abc\n"),
        "lock.yaml",
    )
    .expect_err("newer");
    assert_eq!(
        newer,
        DepsError::TooNew {
            path: "lock.yaml".to_string(),
            kind: "lockfile".to_string(),
            found: 4,
            current: 3
        }
    );
    for text in [
        format!("bundles:\n  {KEY}:\n    sha: abc\n"),
        format!("version: 3\nbundles:\n  {KEY}:\n    sha: abc\n"),
        format!("schema_version: 1\nbundles:\n  {KEY}:\n    sha: abc\n"),
    ] {
        let older = Lock::decode(&text, "lock.yaml").expect_err("older");
        assert!(
            matches!(&older, DepsError::TooOld { found, oldest: 2, kind, .. } if *found <= 1 && kind == "lockfile"),
            "{text}: {older:?}"
        );
    }
}

// lockfile_test.go: Load_DocumentlessFileIsRefusedAsEmpty. A file with no
// document is not the same as no lock.
#[test]
fn test_a_lock_with_no_document_is_refused_not_read_as_empty() {
    for text in ["", "  \n\n", "# only a comment\n", "null\n"] {
        assert_eq!(
            Lock::decode(text, "/p/lock.yaml").expect_err(text),
            DepsError::LockEmpty {
                path: "/p/lock.yaml".to_string()
            },
            "{text:?}"
        );
    }
}

// lockfile_test.go: Load_InvalidYAML; lockfile_strict_test.go: a field the
// format does not model is refused, not dropped.
#[test]
fn test_a_lock_that_is_not_yaml_or_models_nothing_it_has_is_refused() {
    for text in [
        "schema_version: 3\nbundles: [unterminated\n",
        &format!("schema_version: 3\nbundles:\n  {KEY}:\n    sha: abc\n    mystery: 1\n"),
        "schema_version: 3\nmystery: 1\n",
        &format!("schema_version: 3\nbundles:\n  {KEY}:\n    sha: abc\n    kind: nonsense\n"),
        &format!("schema_version: 3\nbundles:\n  {KEY}:\n    held: maybe\n"),
    ] {
        let refused = Lock::decode(text, "lock.yaml").expect_err(text);
        assert!(
            matches!(
                refused,
                DepsError::Unreadable { .. } | DepsError::BadShape { .. }
            ),
            "{text}: {refused:?}"
        );
    }
}

// lockfile_strict_test.go: a key that is not its own bundle identity is
// one no lookup reaches.
#[test]
fn test_an_entry_not_keyed_by_its_identity_is_refused() {
    for key in [
        "demo",
        "ctxloom+git://example.test/repo//bundles/kit@v1",
        "ctxloom+git://Example.test/repo//bundles/kit",
        "https://example.test/repo@bundles/kit",
    ] {
        let text = format!("schema_version: 3\nbundles:\n  {key}:\n    sha: abc\n");
        assert_eq!(
            Lock::decode(&text, "lock.yaml").expect_err(key),
            DepsError::LockKey {
                path: "lock.yaml".to_string(),
                key: key.to_string()
            },
            "{key}"
        );
    }
}

// lockfile_held_test.go: a hold is serialized `held: true`; pinned alone is
// no field.
#[test]
fn test_a_hold_is_the_held_field_and_nothing_else_is() {
    let mut held = entry();
    held.held = true;
    assert!(lock_of(&[(KEY, held)])
        .encode()
        .contains("    held: true\n"));
    assert!(!lock_of(&[(KEY, entry())]).encode().contains("held"));
}

// lockfile_guard_test.go
#[test]
fn test_a_write_over_a_newer_lock_is_refused() {
    let current = "schema_version: 9\nbundles: {}\n";
    let refused = guard_write(
        Some(current),
        &lock_of(&[(KEY, entry())]),
        false,
        "lock.yaml",
    )
    .expect_err("newer");
    assert!(
        matches!(
            refused,
            DepsError::TooNew {
                found: 9,
                current: 3,
                ..
            }
        ),
        "{refused:?}"
    );
}

#[test]
fn test_a_write_over_an_unreadable_lock_is_refused_whatever_is_written() {
    for allow_empty in [false, true] {
        let refused = guard_write(
            Some("schema_version: 3\nbundles: [unterminated\n"),
            &lock_of(&[(KEY, entry())]),
            allow_empty,
            "/p/lock.yaml",
        )
        .expect_err("unreadable");
        assert!(
            matches!(&refused, DepsError::LockUnreadable { path, .. } if path == "/p/lock.yaml"),
            "{refused:?}"
        );
    }
}

#[test]
fn test_an_empty_lock_does_not_replace_a_populated_one_unless_meant() {
    let populated = lock_of(&[(KEY, entry()), ("ctxloom+git://h/r//bundles/z", entry())]).encode();
    let empty = Lock::default();
    assert_eq!(
        guard_write(Some(&populated), &empty, false, "lock.yaml"),
        Err(DepsError::LockWouldErase {
            path: "lock.yaml".to_string(),
            count: 2
        })
    );
    assert_eq!(
        guard_write(Some(&populated), &empty, true, "lock.yaml"),
        Ok(())
    );
    assert_eq!(
        guard_write(
            Some(&populated),
            &lock_of(&[(KEY, entry())]),
            false,
            "lock.yaml"
        ),
        Ok(())
    );
}

#[test]
fn test_a_write_where_there_is_nothing_to_protect_is_allowed() {
    let empty = Lock::default();
    assert_eq!(guard_write(None, &empty, false, "lock.yaml"), Ok(()));
    assert_eq!(
        guard_write(Some("schema_version: 3\n"), &empty, false, "lock.yaml"),
        Ok(())
    );
    assert_eq!(
        guard_write(None, &lock_of(&[(KEY, entry())]), false, "lock.yaml"),
        Ok(())
    );
}

#[test]
fn test_every_spelling_of_null_is_a_lock_with_no_document() {
    for text in ["~\n", "Null\n", "NULL\n", "null", " ~ \n"] {
        assert_eq!(
            Lock::decode(text, "/p/lock.yaml").expect_err(text),
            DepsError::LockEmpty {
                path: "/p/lock.yaml".to_string()
            },
            "{text:?}"
        );
    }
}
