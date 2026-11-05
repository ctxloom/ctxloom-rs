//! Go's editor round trip (`editInEditor`, `bundle_items.go`) and the fixture
//! editors of `steps_fixture.go`: `sh -c <script> editor <tmpfile>`, so `$1`
//! is the file.

use file_system::{Perms, Rooted};
use os::Mode;

use super::*;

fn sh(script: &str, args: &[&str]) -> EditorCommand {
    let mut all = vec!["-c".to_string(), script.to_string()];
    all.extend(args.iter().map(ToString::to_string));
    // "editor" is $0 unless a test names one.
    EditorCommand {
        program: "sh".to_string(),
        args: all,
    }
}

fn editor() -> (Arc<Rooted>, ProcessEditor) {
    let fs = Arc::new(Rooted::temp().expect("a root"));
    fs.create_dir_all(Path::new("/tmp"), Mode::from_bits(0o755))
        .unwrap();
    let editor = ProcessEditor::new(fs.clone(), Path::new("/tmp"));
    (fs, editor)
}

fn leftovers(fs: &Rooted) -> Vec<String> {
    fs.read_dir(Path::new("/tmp"))
        .unwrap()
        .into_iter()
        .map(|e| e.name.to_string_lossy().into_owned())
        .collect()
}

// steps_fixture.go markerEditorConfig
#[test]
fn test_the_content_goes_in_the_editor_and_what_it_leaves_comes_back() {
    let (fs, editor) = editor();
    let marker = sh("printf \"\\nEDITED-BY-TEST\\n\" >> \"$1\"", &["editor"]);
    let out = editor
        .edit(&marker, "testing.md", "FRAGMENT-BODY-testing")
        .unwrap();
    assert_eq!(out, "FRAGMENT-BODY-testing\nEDITED-BY-TEST\n");
    assert_eq!(
        leftovers(&fs),
        Vec::<String>::new(),
        "the temporary directory is removed"
    );
}

// emptyEditorConfig: a crashed editor is not an error here; the caller judges
#[test]
fn test_an_editor_that_empties_the_file_returns_nothing() {
    let (_fs, editor) = editor();
    let empties = sh("printf \"\" > \"$1\"", &["editor"]);
    assert_eq!(editor.edit(&empties, "x.md", "was something").unwrap(), "");
}

#[test]
fn test_the_editor_is_given_a_file_with_the_name_asked_for_and_mode_0600() {
    let (fs, editor) = editor();
    let seen_at = os::fixture("/witness/seen");
    fs.create_dir_all(seen_at.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    let w = fs.real_path(&seen_at).unwrap();
    let w = w.to_str().unwrap();
    let probe = sh(
        "{ basename \"$1\"; cat \"$1\"; stat -c %a \"$1\"; } > \"$0\"",
        &[w],
    );
    editor.edit(&probe, "review.md", "THE CONTENT").unwrap();
    assert_eq!(
        fs.read_to_string(&seen_at).unwrap(),
        "review.md\nTHE CONTENT600\n"
    );
}

#[test]
fn test_the_editors_arguments_come_before_the_file() {
    let (fs, editor) = editor();
    let seen_at = os::fixture("/witness/seen");
    fs.create_dir_all(seen_at.parent().unwrap(), Mode::from_bits(0o755))
        .unwrap();
    let w = fs.real_path(&seen_at).unwrap();
    let w = w.to_str().unwrap();
    let command = EditorCommand {
        program: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            "echo \"$2\" > \"$0\"".to_string(),
            w.to_string(),
            "first".to_string(),
        ],
    };
    // $0 = w, $1 = first, $2 = the file: its last component names the item.
    editor.edit(&command, "item.md", "x").unwrap();
    let seen = fs.read_to_string(&seen_at).unwrap();
    assert!(seen.trim_end().ends_with("/item.md"), "{seen}");
}

#[test]
fn test_an_editor_that_fails_is_an_error_and_leaves_nothing_behind() {
    let (fs, editor) = editor();
    let err = editor
        .edit(&sh("exit 3", &["editor"]), "x.md", "c")
        .unwrap_err();
    assert_eq!(err.code(), "editor-process-failed");
    assert!(err.to_string().contains("editor-process-failed"));
    assert_eq!(err.args()[0].0, "program");
    assert!(leftovers(&fs).is_empty());
}

#[test]
fn test_an_editor_that_cannot_be_run_is_an_error() {
    let (fs, editor) = editor();
    let command = EditorCommand {
        program: "/no/such/editor".to_string(),
        args: Vec::new(),
    };
    let err = editor.edit(&command, "x.md", "c").unwrap_err();
    assert_eq!(err.code(), "editor-process-spawn");
    assert!(leftovers(&fs).is_empty());
}

#[test]
fn test_what_is_not_text_is_refused() {
    let (fs, editor) = editor();
    let bad = sh("printf '\\377\\376' > \"$1\"", &["editor"]);
    let err = editor.edit(&bad, "x.md", "c").unwrap_err();
    assert_eq!(err.code(), "editor-process-read");
    assert!(leftovers(&fs).is_empty());
}

#[test]
fn test_a_file_name_is_one_name_not_a_path() {
    let (_fs, editor) = editor();
    let noop = sh("true", &["editor"]);
    for bad in ["", ".", "..", "a/b", "../x", "/abs"] {
        let err = editor.edit(&noop, bad, "c").unwrap_err();
        assert_eq!(err.code(), "editor-process-bad-name", "{bad:?}");
    }
}

// config.go openInEditor: the editor edits the file where it is
#[test]
fn test_open_runs_the_editor_on_the_file_in_place() {
    let root = Rooted::temp().unwrap();
    let dir = root.root().to_path_buf();
    let fs = Arc::new(file_system::Os::new(os::host(), &dir).expect("the real filesystem"));
    let editor = ProcessEditor::new(fs.clone(), &dir);
    let file = dir.join("config.yaml");
    fs.write_atomic(&file, b"a: 1\n", Perms::Exact(Mode::FILE))
        .unwrap();
    let append = sh("printf '# edited\\n' >> \"$1\"", &["editor"]);
    editor.open(&append, file.to_str().unwrap()).unwrap();
    assert_eq!(fs.read_to_string(&file).unwrap(), "a: 1\n# edited\n");
    let fails = sh("exit 1", &["editor"]);
    assert_eq!(
        editor
            .open(&fails, file.to_str().unwrap())
            .unwrap_err()
            .code(),
        "editor-process-failed"
    );
    let missing = EditorCommand {
        program: "/no/such/editor".to_string(),
        args: Vec::new(),
    };
    assert_eq!(
        editor
            .open(&missing, file.to_str().unwrap())
            .unwrap_err()
            .code(),
        "editor-process-spawn"
    );
}

#[test]
fn test_every_error_is_localized() {
    for sample in diagnostic_samples() {
        assert!(
            sample.code().starts_with("editor-process-"),
            "{}",
            sample.code()
        );
        assert!(sample.to_string().contains(sample.code()));
    }
    assert_eq!(diagnostic_samples().len(), 5);
}
