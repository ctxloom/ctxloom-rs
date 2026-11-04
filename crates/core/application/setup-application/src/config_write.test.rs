//! `util config-write` over in-memory files (cli/util.feature).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use surface_ports::{ForeignFiles, SurfaceError};

use crate::fakes::{world, CONFIG};
use crate::{Setup, SetupError};

#[derive(Default)]
struct Files {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
    records: Mutex<BTreeMap<String, String>>,
    /// A disk that loses what it is given: every write leaves `{}`.
    lossy: std::sync::atomic::AtomicBool,
}

impl ForeignFiles for Files {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, SurfaceError> {
        Ok(self.files.lock().unwrap().get(path).cloned())
    }
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), SurfaceError> {
        let bytes = if self.lossy.load(std::sync::atomic::Ordering::SeqCst) {
            b"{}".as_slice()
        } else {
            bytes
        };
        self.files
            .lock()
            .unwrap()
            .insert(path.into(), bytes.to_vec());
        Ok(())
    }
    fn record(&self, path: &str) -> Result<Option<String>, SurfaceError> {
        Ok(self.records.lock().unwrap().get(path).cloned())
    }
    fn write_record(&self, path: &str, text: &str) -> Result<String, SurfaceError> {
        self.records
            .lock()
            .unwrap()
            .insert(path.into(), text.into());
        Ok(format!("records/{path}"))
    }
}

fn setup(files: &Arc<Files>) -> Setup {
    world(CONFIG).setup.with_foreign(files.clone())
}

const VENDOR: &str = r#"{"$schema": "https://example.com/v.json", "servers": {"vendor-owned": {"command": "vendor-server", "args": ["--serve"], "env": {"VENDOR_TOKEN": "keep-me-verbatim"}}}}"#;
const F: &str = "/work/vendor-tool.json";

fn patch(command: &str) -> String {
    format!(r#"{{"servers": {{"ctxloom": {{"command": "{command}", "args": ["mcp", "serve"]}}}}}}"#)
}

fn text(files: &Files) -> String {
    String::from_utf8(files.files.lock().unwrap().get(F).cloned().unwrap()).unwrap()
}

// util.feature: Scenario: A third write still replaces ctxloom's entry after
// one field changed
#[test]
fn test_a_third_write_replaces_ctxloom_entry_and_keeps_the_vendors() {
    let files = Arc::new(Files::default());
    files.write(F, VENDOR.as_bytes()).unwrap();
    let setup = setup(&files);
    for command in ["/first/ctxloom", "/second/ctxloom", "/third/ctxloom"] {
        let done = setup.config_write(F, None, &patch(command)).unwrap();
        assert!(done.verified && !done.created);
        assert!(text(&files).contains(command));
    }
    let held = text(&files);
    assert_eq!(held.matches("/third/ctxloom").count(), 1);
    assert!(!held.contains("/first/ctxloom") && !held.contains("/second/ctxloom"));
    for kept in [
        "https://example.com/v.json",
        "keep-me-verbatim",
        "vendor-server",
        "--serve",
    ] {
        assert!(held.contains(kept), "{held}");
    }
    // The arrays were replaced, not appended to.
    assert_eq!(held.matches("\"serve\"").count(), 1, "{held}");
}

// util.feature (the opening paragraph): the next write reverses the record of
// the last before applying the new set.
#[test]
fn test_a_write_takes_back_a_key_the_last_write_added() {
    let files = Arc::new(Files::default());
    files.write(F, VENDOR.as_bytes()).unwrap();
    let setup = setup(&files);
    setup
        .config_write(
            F,
            None,
            r#"{"servers": {"ctxloom": {"command": "a", "env": {"A": "1"}}}}"#,
        )
        .unwrap();
    setup
        .config_write(F, None, r#"{"servers": {"ctxloom": {"command": "b"}}}"#)
        .unwrap();
    let held = text(&files);
    assert!(held.contains("\"b\"") && !held.contains("\"A\""), "{held}");
    assert!(held.contains("keep-me-verbatim"));
}

// util.feature: a value of the vendor's that the patch replaces comes back
// when the next write no longer touches it.
#[test]
fn test_a_replaced_vendor_value_comes_back_when_the_next_write_leaves_it() {
    let files = Arc::new(Files::default());
    files.write(F, br#"{"a": {"b": 1}, "keep": true}"#).unwrap();
    let setup = setup(&files);
    setup.config_write(F, None, r#"{"a": {"b": 2}}"#).unwrap();
    assert!(text(&files).contains("\"b\": 2"));
    setup.config_write(F, None, r#"{"c": 3}"#).unwrap();
    let held = text(&files);
    assert!(
        held.contains("\"b\": 1") && held.contains("\"c\": 3") && held.contains("keep"),
        "{held}"
    );
}

// util.feature (long help, rules 3 and 4).
#[test]
fn test_a_missing_file_starts_empty_and_an_unparseable_one_is_never_overwritten() {
    let files = Arc::new(Files::default());
    let setup = setup(&files);
    let made = setup.config_write(F, None, r#"{"a": {"b": 1}}"#).unwrap();
    assert!(made.created);
    assert_eq!(made.merged_keys, vec!["a"]);
    assert!(made.record.starts_with("records/"));
    files.write("/work/bad.json", b"{ no").unwrap();
    let err = setup
        .config_write("/work/bad.json", None, r#"{"a": 1}"#)
        .unwrap_err();
    assert!(matches!(err, SetupError::ConfigWrite { .. }));
    assert_eq!(files.read("/work/bad.json").unwrap().unwrap(), b"{ no");
}

// util.feature (long help, rule 1 and the patch's own): the arguments are
// checked before anything is read.
#[test]
fn test_the_path_and_the_patch_are_checked_first() {
    let files = Arc::new(Files::default());
    let setup = setup(&files);
    for (file, kind, patch) in [
        ("", None, r#"{"a":1}"#),
        ("rel.json", None, r#"{"a":1}"#),
        ("/x/~/a.json", None, r#"{"a":1}"#),
        ("/x/$HOME.json", None, r#"{"a":1}"#),
        ("/x/a.toml", None, r#"{"a":1}"#),
        ("/x/a", None, r#"{"a":1}"#),
        ("/x/a.json", Some("toml"), r#"{"a":1}"#),
        ("/x/a.json", None, ""),
        ("/x/a.json", None, "[1]"),
        ("/x/a.json", None, "{}"),
        ("/x/a.json", None, "nope"),
    ] {
        let err = setup.config_write(file, kind, patch).unwrap_err();
        assert!(
            matches!(err, SetupError::ConfigWrite { .. }),
            "{file} {patch}"
        );
    }
    assert!(files.files.lock().unwrap().is_empty());
}

#[test]
fn test_a_record_ctxloom_cannot_read_stops_the_write() {
    let files = Arc::new(Files::default());
    files.write(F, VENDOR.as_bytes()).unwrap();
    files.write_record(F, "garbage").unwrap();
    let err = setup(&files)
        .config_write(F, None, &patch("x"))
        .unwrap_err();
    assert!(matches!(err, SetupError::ConfigWrite { .. }));
    assert_eq!(text(&files), VENDOR);
}

#[test]
fn test_a_build_without_foreign_files_says_so() {
    let err = world(CONFIG).setup.config_write(F, None, "{}").unwrap_err();
    assert!(matches!(err, SetupError::ConfigWrite { .. }));
}

// Payload verification: a write the disk did not keep is refused, not reported.
#[test]
fn test_a_write_the_file_does_not_hold_fails_verification() {
    let files = Arc::new(Files::default());
    files.lossy.store(true, std::sync::atomic::Ordering::SeqCst);
    let err = setup(&files)
        .config_write(F, None, r#"{"a": {"b": 1}}"#)
        .unwrap_err();
    let SetupError::ConfigWrite { .. } = err else {
        panic!("{err:?}")
    };
    assert!(err.to_string().contains("verify"), "{err}");
}

// A file with nothing but whitespace is an empty document, not a corrupt one.
#[test]
fn test_a_blank_file_is_an_empty_document() {
    let files = Arc::new(Files::default());
    files.write(F, b" \n\t\n").unwrap();
    let written = setup(&files).config_write(F, None, r#"{"a": 1}"#).unwrap();
    assert!(!written.created);
    assert!(text(&files).contains("\"a\": 1"));
}
