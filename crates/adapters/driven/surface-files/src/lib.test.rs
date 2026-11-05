use std::sync::Arc;

use file_system::{Perms, Rooted};
use os::Mode;

use surface_ports::{Json, JsonOp};

use super::*;

/// The store over a real temporary directory the seam re-roots every path
/// under (`Rooted`), so a test can observe modes; the target is `/project`.
struct Fixture {
    fs: Arc<Rooted>,
    target: String,
    store: SurfaceFiles,
}

impl Fixture {
    fn new() -> Fixture {
        let fs = Arc::new(Rooted::temp().unwrap());
        fs.create_dir_all(Path::new("/project"), Mode::DIR).unwrap();
        let store = SurfaceFiles::new(fs.clone(), "/home/records");
        Fixture {
            fs,
            target: "/project".to_string(),
            store,
        }
    }

    fn at(&self, rel: &str) -> PathBuf {
        Path::new(&self.target).join(rel)
    }

    fn put(&self, rel: &str, text: &str) {
        let path = self.at(rel);
        self.fs
            .create_dir_all(path.parent().unwrap(), Mode::DIR)
            .unwrap();
        self.fs
            .write_atomic(&path, text.as_bytes(), Perms::Exact(Mode::FILE))
            .unwrap();
    }

    fn get(&self, rel: &str) -> Option<String> {
        self.fs.read_to_string(&self.at(rel)).ok()
    }

    fn mode_of(&self, path: &Path) -> u32 {
        self.fs.metadata(path).unwrap().mode.unwrap().bits() & 0o7777
    }

    fn deliver(&self, engine: &str, kind: Kind, claims: &[Claim]) -> Delivered {
        self.store
            .deliver(&self.target, engine, kind, claims, false)
            .unwrap()
    }

    fn release(&self, engine: &str, kind: Kind) -> Released {
        self.store
            .release(&self.target, engine, kind, false)
            .unwrap()
    }
}

fn section(path: &str, text: &str) -> Claim {
    Claim::Section {
        path: path.into(),
        text: text.into(),
    }
}

fn set(path: &str, key: &str, value: &str) -> Claim {
    Claim::Json {
        path: path.into(),
        ops: vec![JsonOp::Set {
            path: vec![key.into()],
            value: Json::text(value),
        }],
    }
}

#[test]
fn test_a_file_ctxloom_made_is_removed_with_its_directories_on_release() {
    let f = Fixture::new();
    f.deliver(
        "e",
        Kind::Settings,
        &[set(".claude/settings.json", "a", "1")],
    );
    assert!(f.get(".claude/settings.json").is_some());
    f.release("e", Kind::Settings);
    assert!(f.get(".claude/settings.json").is_none());
    assert!(!f.fs.exists(&f.at(".claude")).unwrap());
    assert!(f.fs.exists(Path::new(&f.target)).unwrap());
}

#[test]
fn test_a_file_that_was_there_comes_back_byte_for_byte() {
    let f = Fixture::new();
    let theirs = "{\n\t\"x\":   1234567890123456789,\n  \"y\": [1,2]}\n";
    f.put("settings.json", theirs);
    f.deliver("e", Kind::Settings, &[set("settings.json", "a", "1")]);
    assert_ne!(f.get("settings.json").as_deref(), Some(theirs));
    assert!(f
        .get("settings.json")
        .unwrap()
        .contains("1234567890123456789"));
    f.release("e", Kind::Settings);
    assert_eq!(f.get("settings.json").as_deref(), Some(theirs));
}

#[test]
fn test_delivering_again_replaces_the_claim_instead_of_doubling_it() {
    let f = Fixture::new();
    f.put("CLAUDE.md", "# Mine");
    f.deliver("e", Kind::Context, &[section("CLAUDE.md", "ctxloom text")]);
    f.deliver("e", Kind::Context, &[section("CLAUDE.md", "ctxloom text")]);
    assert_eq!(
        f.get("CLAUDE.md").unwrap().matches("ctxloom text").count(),
        1
    );
    f.deliver("e", Kind::Context, &[section("CLAUDE.md", "newer text")]);
    let held = f.get("CLAUDE.md").unwrap();
    assert!(
        held.contains("newer text") && !held.contains("ctxloom text"),
        "{held}"
    );
    f.release("e", Kind::Context);
    assert_eq!(f.get("CLAUDE.md").as_deref(), Some("# Mine"));
}

#[test]
fn test_releasing_one_kind_leaves_the_other_kinds_claims_in_the_same_file() {
    let f = Fixture::new();
    f.put("s.json", "{\"mine\": true}\n");
    f.deliver("e", Kind::Settings, &[set("s.json", "a", "1")]);
    f.deliver("e", Kind::Hooks, &[set("s.json", "b", "2")]);
    f.release("e", Kind::Hooks);
    let held = f.get("s.json").unwrap();
    assert!(
        held.contains("\"a\"") && !held.contains("\"b\"") && held.contains("mine"),
        "{held}"
    );
    f.release("e", Kind::Settings);
    assert_eq!(f.get("s.json").as_deref(), Some("{\"mine\": true}\n"));
}

#[test]
fn test_two_engines_in_one_file_release_independently() {
    let f = Fixture::new();
    f.deliver("one", Kind::Context, &[section("AGENTS.md", "first")]);
    f.deliver("two", Kind::Context, &[section("AGENTS.md", "second")]);
    f.release("one", Kind::Context);
    let held = f.get("AGENTS.md").unwrap();
    assert!(held.contains("second") && !held.contains("first"), "{held}");
    f.release("two", Kind::Context);
    assert!(f.get("AGENTS.md").is_none());
}

#[test]
fn test_a_file_somebody_changed_since_has_only_ctxloom_entries_cut_out() {
    let f = Fixture::new();
    f.deliver("e", Kind::Settings, &[set("s.json", "a", "1")]);
    let edited = f
        .get("s.json")
        .unwrap()
        .replace("{\n", "{\n  \"theirs\": 1,\n");
    f.put("s.json", &edited);
    f.release("e", Kind::Settings);
    let held = f.get("s.json").unwrap();
    assert!(held.contains("theirs") && !held.contains("\"a\""), "{held}");
}

#[test]
fn test_a_whole_file_keeps_its_execute_bit_and_a_dry_run_writes_nothing() {
    let f = Fixture::new();
    let claim = Claim::Whole {
        path: "skills/x/run.sh".into(),
        bytes: b"#!/bin/sh\n".to_vec(),
        executable: true,
    };
    let dry = f
        .store
        .deliver(
            &f.target,
            "e",
            Kind::Skills,
            std::slice::from_ref(&claim),
            true,
        )
        .unwrap();
    assert!(dry.changed);
    assert!(f.get("skills/x/run.sh").is_none());
    f.deliver("e", Kind::Skills, &[claim]);
    if f.fs.os().flavor() == os::Flavor::Unix {
        assert_eq!(f.mode_of(&f.at("skills/x/run.sh")) & 0o111, 0o111);
    }
    let again = f.store.release(&f.target, "e", Kind::Skills, true).unwrap();
    assert!(again.changed);
    assert!(f.get("skills/x/run.sh").is_some());
}

#[test]
fn test_a_delivery_that_changes_nothing_says_so() {
    let f = Fixture::new();
    let claim = section("CLAUDE.md", "text");
    assert!(
        f.deliver("e", Kind::Context, std::slice::from_ref(&claim))
            .changed
    );
    assert!(!f.deliver("e", Kind::Context, &[claim]).changed);
    assert!(!f.release("e", Kind::Skills).changed);
}

#[test]
fn test_what_an_engine_claims_is_listed_by_kind() {
    let f = Fixture::new();
    f.deliver("e", Kind::Hooks, &[set("s.json", "a", "1")]);
    f.deliver("e", Kind::Context, &[section("C.md", "t")]);
    f.deliver("other", Kind::Mcp, &[set("m.json", "a", "1")]);
    assert_eq!(
        f.store.claimed(&f.target, "e").unwrap(),
        vec![Kind::Context, Kind::Hooks]
    );
    f.release("e", Kind::Hooks);
    assert_eq!(
        f.store.claimed(&f.target, "e").unwrap(),
        vec![Kind::Context]
    );
    assert!(f.store.claimed(&f.target, "none").unwrap().is_empty());
}

#[test]
fn test_a_refused_write_leaves_the_file_and_the_record_alone() {
    let f = Fixture::new();
    f.put("s.json", "{ nope");
    let err = f
        .store
        .deliver(
            &f.target,
            "e",
            Kind::Settings,
            &[set("s.json", "a", "1")],
            false,
        )
        .unwrap_err();
    assert!(matches!(err, SurfaceError::Refused { .. }));
    assert_eq!(f.get("s.json").as_deref(), Some("{ nope"));
    assert!(f.store.claimed(&f.target, "e").unwrap().is_empty());
}

#[test]
fn test_a_claim_outside_the_target_is_refused() {
    let f = Fixture::new();
    for bad in ["../x", "/etc/x", "", "a/../../x"] {
        let err = f
            .store
            .deliver(&f.target, "e", Kind::Context, &[section(bad, "t")], false)
            .unwrap_err();
        assert!(matches!(err, SurfaceError::Refused { .. }), "{bad}");
    }
}

#[test]
fn test_files_are_read_below_the_target_only() {
    let f = Fixture::new();
    f.put("a/b.txt", "x");
    assert_eq!(
        SurfaceStore::read(&f.store, &f.target, "a/b.txt").unwrap(),
        Some(b"x".to_vec())
    );
    assert_eq!(
        SurfaceStore::read(&f.store, &f.target, "missing").unwrap(),
        None
    );
    assert!(SurfaceStore::read(&f.store, &f.target, "../a").is_err());
}

#[test]
fn test_a_corrupt_record_is_an_error_not_a_blank_slate() {
    let f = Fixture::new();
    f.deliver("e", Kind::Context, &[section("C.md", "t")]);
    let path = f.store.record_path(&f.target);
    f.fs.write_atomic(&path, b"garbage", Perms::Exact(Mode::FILE))
        .unwrap();
    assert!(matches!(
        f.store.claimed(&f.target, "e"),
        Err(SurfaceError::Record { .. })
    ));
}

#[test]
fn test_a_target_is_inspected_and_a_file_is_read_by_its_absolute_path() {
    let f = Fixture::new();
    assert_eq!(f.store.inspect(&f.target).unwrap(), Inspected::Directory);
    assert_eq!(
        f.store.inspect(&format!("{}/nope", f.target)).unwrap(),
        Inspected::Missing
    );
    f.put("a.txt", "x");
    assert_eq!(
        f.store.inspect(&format!("{}/a.txt", f.target)).unwrap(),
        Inspected::Other
    );
    assert_eq!(
        f.store
            .read_absolute(&format!("{}/a.txt", f.target))
            .unwrap(),
        Some(b"x".to_vec())
    );
    assert_eq!(f.store.read_absolute("/no/such/file").unwrap(), None);
}

#[test]
fn test_a_foreign_file_and_the_record_of_its_last_write_are_kept_apart() {
    let f = Fixture::new();
    let path = format!("{}/vendor.json", f.target);
    assert_eq!(ForeignFiles::read(&f.store, &path).unwrap(), None);
    assert_eq!(f.store.record(&path).unwrap(), None);
    ForeignFiles::write(&f.store, &path, b"{}").unwrap();
    assert_eq!(f.get("vendor.json").as_deref(), Some("{}"));
    let kept = f.store.write_record(&path, "record").unwrap();
    assert!(kept.ends_with(".hew-record.yaml") && !kept.starts_with(&f.target));
    assert_eq!(f.store.record(&path).unwrap().as_deref(), Some("record"));
}

// A kind delivered again keeps its place among the claims of a shared file, so
// a delivery that changes nothing changes nothing (the project refresh's
// preview of a project already current names no kind).
#[test]
fn test_delivering_an_earlier_kind_again_leaves_a_shared_file_unchanged() {
    let f = Fixture::new();
    let first = set("s.json", "first", "1");
    let second = set("s.json", "second", "2");
    f.deliver("e", Kind::Settings, std::slice::from_ref(&first));
    f.deliver("e", Kind::Hooks, std::slice::from_ref(&second));
    let before = f.get("s.json").unwrap();
    let again = f
        .store
        .deliver(&f.target, "e", Kind::Settings, &[first], true)
        .unwrap();
    assert!(!again.changed);
    assert_eq!(f.get("s.json").unwrap(), before);
}

#[test]
fn test_a_record_is_named_for_its_target_with_only_safe_characters_kept() {
    let f = Fixture::new();
    let records = f.store.records.clone();
    assert_eq!(
        f.store.record_path("/a-b.c/d e9_Z"),
        records.join("_a-b.c_d_e9_Z.claims.yaml")
    );
    assert_eq!(
        f.store.foreign_record("/a-b.c/d e9_Z"),
        records.join("_a-b.c_d_e9_Z.hew-record.yaml")
    );
}

#[test]
fn test_a_record_that_cannot_be_read_is_an_error_not_a_blank_slate() {
    let f = Fixture::new();
    f.fs.create_dir_all(&f.store.record_path(&f.target), Mode::DIR)
        .unwrap();
    assert!(matches!(
        f.store.claimed(&f.target, "e"),
        Err(SurfaceError::Io { .. })
    ));
}

#[test]
fn test_saving_no_claims_removes_the_record_and_reports_other_failures() {
    let f = Fixture::new();
    // Nothing to remove is fine.
    f.store.save(&f.target, &Record::default()).unwrap();
    // A record that cannot be removed is not.
    let path = f.store.record_path(&f.target);
    f.fs.create_dir_all(&path.join("inner"), Mode::DIR).unwrap();
    assert!(matches!(
        f.store.save(&f.target, &Record::default()),
        Err(SurfaceError::Io { .. })
    ));
}

#[test]
fn test_a_file_that_cannot_be_read_is_an_error_and_a_missing_one_is_none() {
    let f = Fixture::new();
    f.put("dir/x", "x");
    assert!(matches!(
        SurfaceStore::read(&f.store, &f.target, "dir"),
        Err(SurfaceError::Io { .. })
    ));
    assert!(matches!(
        ForeignFiles::read(&f.store, &format!("{}/dir", f.target)),
        Err(SurfaceError::Io { .. })
    ));
}

#[test]
fn test_a_foreign_file_that_was_written_is_read_back() {
    let f = Fixture::new();
    let path = format!("{}/v.json", f.target);
    ForeignFiles::write(&f.store, &path, b"{}").unwrap();
    assert_eq!(
        ForeignFiles::read(&f.store, &path).unwrap(),
        Some(b"{}".to_vec())
    );
}

#[test]
fn test_a_foreign_record_that_cannot_be_read_is_an_error() {
    let f = Fixture::new();
    let path = format!("{}/vendor.json", f.target);
    f.fs.create_dir_all(&f.store.foreign_record(&path), Mode::DIR)
        .unwrap();
    assert!(matches!(
        f.store.record(&path),
        Err(SurfaceError::Io { .. })
    ));
}

#[test]
fn test_inspecting_a_path_through_a_file_is_an_error_not_missing() {
    let f = Fixture::new();
    f.put("a.txt", "x");
    assert!(matches!(
        f.store.inspect(&format!("{}/a.txt/below", f.target)),
        Err(SurfaceError::Io { .. })
    ));
}

#[test]
fn test_a_written_file_keeps_its_mode_or_takes_the_mode_asked_for() {
    let f = Fixture::new();
    let fs = &*f.fs;
    // New: 0644, or 0755 when executable.
    write(fs, &f.at("new"), b"x", false).unwrap();
    assert_eq!(f.mode_of(&f.at("new")), 0o644);
    write(fs, &f.at("new-exec"), b"x", true).unwrap();
    assert_eq!(f.mode_of(&f.at("new-exec")), 0o755);
    // Existing: kept, unless executable is asked for.
    let held = f.at("held");
    f.put("held", "old");
    fs.set_permissions(&held, Mode::from_bits(0o640)).unwrap();
    write(fs, &held, b"new", false).unwrap();
    assert_eq!(f.mode_of(&held), 0o640);
    assert_eq!(f.get("held").as_deref(), Some("new"));
    write(fs, &held, b"newer", true).unwrap();
    assert_eq!(f.mode_of(&held), 0o755);
}

#[test]
fn test_removing_a_file_that_is_already_gone_succeeds_and_other_failures_do_not() {
    let f = Fixture::new();
    let fs = &*f.fs;
    remove(fs, &f.target, &f.at("gone")).unwrap();
    fs.create_dir_all(&f.at("d/inner"), Mode::DIR).unwrap();
    assert!(matches!(
        remove(fs, &f.target, &f.at("d")),
        Err(SurfaceError::Io { .. })
    ));
}

#[test]
fn test_removing_a_file_leaves_the_directories_that_still_hold_something() {
    let f = Fixture::new();
    f.put("a/b/x", "x");
    f.put("a/keep", "k");
    remove(&*f.fs, &f.target, &f.at("a/b/x")).unwrap();
    assert!(!f.fs.exists(&f.at("a/b")).unwrap());
    assert!(f.get("a/keep").is_some());
}

#[test]
fn test_delivering_again_keeps_the_place_among_another_engines_claims() {
    let f = Fixture::new();
    f.deliver("one", Kind::Context, &[section("AGENTS.md", "first")]);
    f.deliver("two", Kind::Context, &[section("AGENTS.md", "second")]);
    let before = f.get("AGENTS.md").unwrap();
    f.deliver("two", Kind::Context, &[section("AGENTS.md", "second")]);
    assert_eq!(f.get("AGENTS.md").unwrap(), before);
    assert!(before.find("first") < before.find("second"), "{before}");
}

#[test]
fn test_claims_to_one_file_are_delivered_in_the_order_given_each_time() {
    let f = Fixture::new();
    let claims = [set("s.json", "a", "1"), set("s.json", "b", "2")];
    f.deliver("e", Kind::Settings, &claims);
    f.deliver("e", Kind::Settings, &claims);
    let held = f.get("s.json").unwrap();
    assert!(held.find("\"a\"") < held.find("\"b\""), "{held}");
}

#[test]
fn test_a_delivery_names_only_the_files_it_claims_not_those_it_let_go() {
    let f = Fixture::new();
    f.deliver(
        "e",
        Kind::Settings,
        &[set("s.json", "a", "1"), set("t.json", "b", "2")],
    );
    let again = f.deliver("e", Kind::Settings, &[set("s.json", "a", "1")]);
    assert_eq!(again.files, vec!["s.json".to_string()]);
    assert!(f.get("t.json").is_none());
}
